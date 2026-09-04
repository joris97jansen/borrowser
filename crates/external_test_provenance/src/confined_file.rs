use std::fs;
use std::path::{Component, Path, PathBuf};

#[cfg(unix)]
use std::fs::File;
#[cfg(unix)]
use std::io::Read;

#[cfg(unix)]
use crate::allocation::{
    ProductionReservation, ReservationPolicy, ReservationSite, try_reserve_vec,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfinedFileError {
    InvalidRelativePath,
    Missing,
    Symlink,
    NonDirectoryParent,
    NonRegularFile,
    TooLarge,
    Io,
}

/// Failures from the same-opened-object confined reader.
///
/// Unlike [`read_confined_regular_file`], this API guarantees that component
/// traversal, final-object validation, and bounded reading all apply to the
/// same opened filesystem objects. It fails closed when the host cannot
/// provide that guarantee.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SameObjectConfinedReadError {
    InvalidRelativePath,
    Missing,
    Symlink,
    NonDirectoryParent,
    NonRegularFile,
    TooLarge,
    Allocation,
    LengthOverflow,
    UnsupportedPlatform,
    Io,
}

pub fn read_confined_regular_file(
    root: &Path,
    relative: &Path,
    maximum_bytes: u64,
) -> Result<Vec<u8>, ConfinedFileError> {
    let path = validate_confined_regular_file(root, relative, maximum_bytes)?;
    let bytes = fs::read(path).map_err(map_io)?;
    if bytes.len() as u64 > maximum_bytes {
        return Err(ConfinedFileError::TooLarge);
    }
    Ok(bytes)
}

pub fn read_confined_regular_file_same_object(
    root: &Path,
    relative: &Path,
    maximum_bytes: u64,
) -> Result<Vec<u8>, SameObjectConfinedReadError> {
    validate_relative_path(relative)
        .map_err(|_| SameObjectConfinedReadError::InvalidRelativePath)?;
    read_confined_regular_file_same_object_impl(root, relative, maximum_bytes)
}

#[cfg(unix)]
fn read_confined_regular_file_same_object_impl(
    root: &Path,
    relative: &Path,
    maximum_bytes: u64,
) -> Result<Vec<u8>, SameObjectConfinedReadError> {
    let opened = open_confined_regular_file(root, relative)?;
    read_opened_regular_file_bounded(opened, maximum_bytes)
}

#[cfg(not(unix))]
fn read_confined_regular_file_same_object_impl(
    _root: &Path,
    _relative: &Path,
    _maximum_bytes: u64,
) -> Result<Vec<u8>, SameObjectConfinedReadError> {
    Err(SameObjectConfinedReadError::UnsupportedPlatform)
}

#[cfg(unix)]
struct OpenedConfinedRegularFile(File);

#[cfg(unix)]
fn open_confined_regular_file(
    root: &Path,
    relative: &Path,
) -> Result<OpenedConfinedRegularFile, SameObjectConfinedReadError> {
    use rustix::fs::{Mode, OFlags, open, openat};

    let directory_flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let mut directory = open(root, directory_flags, Mode::empty())
        .map_err(|error| map_strong_root_open(root, error))?;
    let mut components = relative.components().peekable();
    while let Some(component) = components.next() {
        let Component::Normal(part) = component else {
            return Err(SameObjectConfinedReadError::InvalidRelativePath);
        };
        if components.peek().is_some() {
            directory = openat(&directory, part, directory_flags, Mode::empty())
                .map_err(|error| map_strong_open_at(&directory, part, error))?;
            continue;
        }
        let file_flags = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK;
        let file = File::from(
            openat(&directory, part, file_flags, Mode::empty())
                .map_err(|error| map_strong_open_at(&directory, part, error))?,
        );
        let metadata = file
            .metadata()
            .map_err(|_| SameObjectConfinedReadError::Io)?;
        if !metadata.is_file() {
            return Err(SameObjectConfinedReadError::NonRegularFile);
        }
        return Ok(OpenedConfinedRegularFile(file));
    }
    Err(SameObjectConfinedReadError::InvalidRelativePath)
}

#[cfg(unix)]
fn read_opened_regular_file_bounded(
    opened: OpenedConfinedRegularFile,
    maximum_bytes: u64,
) -> Result<Vec<u8>, SameObjectConfinedReadError> {
    read_opened_regular_file_bounded_with_policy(opened, maximum_bytes, &mut ProductionReservation)
}

#[cfg(unix)]
fn read_opened_regular_file_bounded_with_policy(
    mut opened: OpenedConfinedRegularFile,
    maximum_bytes: u64,
    reservation: &mut impl ReservationPolicy,
) -> Result<Vec<u8>, SameObjectConfinedReadError> {
    let metadata_length = opened
        .0
        .metadata()
        .map_err(|_| SameObjectConfinedReadError::Io)?
        .len();
    if metadata_length > maximum_bytes {
        return Err(SameObjectConfinedReadError::TooLarge);
    }

    let initial = usize::try_from(metadata_length)
        .map_err(|_| SameObjectConfinedReadError::LengthOverflow)?;
    let mut bytes = Vec::new();
    try_reserve_vec(
        &mut bytes,
        initial,
        ReservationSite::ConfinedReadInitial,
        reservation,
    )
    .map_err(|_| SameObjectConfinedReadError::Allocation)?;
    let mut total = 0_u64;
    let mut chunk = [0_u8; 8 * 1024];
    loop {
        let remaining = maximum_bytes
            .checked_sub(total)
            .ok_or(SameObjectConfinedReadError::LengthOverflow)?;
        if remaining == 0 {
            let mut sentinel = [0_u8; 1];
            match opened.0.read(&mut sentinel) {
                Ok(0) => return Ok(bytes),
                Ok(_) => return Err(SameObjectConfinedReadError::TooLarge),
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => return Err(SameObjectConfinedReadError::Io),
            }
        }
        let requested = usize::try_from(remaining.min(chunk.len() as u64))
            .map_err(|_| SameObjectConfinedReadError::LengthOverflow)?;
        let read = match opened.0.read(&mut chunk[..requested]) {
            Ok(0) => return Ok(bytes),
            Ok(read) => read,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(SameObjectConfinedReadError::Io),
        };
        total = total
            .checked_add(read as u64)
            .ok_or(SameObjectConfinedReadError::LengthOverflow)?;
        try_reserve_vec(
            &mut bytes,
            read,
            ReservationSite::ConfinedReadGrowth,
            reservation,
        )
        .map_err(|_| SameObjectConfinedReadError::Allocation)?;
        bytes.extend_from_slice(&chunk[..read]);
    }
}

#[cfg(unix)]
fn map_strong_open(error: rustix::io::Errno) -> SameObjectConfinedReadError {
    if error == rustix::io::Errno::NOENT {
        SameObjectConfinedReadError::Missing
    } else if error == rustix::io::Errno::LOOP {
        SameObjectConfinedReadError::Symlink
    } else if error == rustix::io::Errno::NOTDIR {
        SameObjectConfinedReadError::NonDirectoryParent
    } else {
        SameObjectConfinedReadError::Io
    }
}

#[cfg(unix)]
fn map_strong_root_open(root: &Path, error: rustix::io::Errno) -> SameObjectConfinedReadError {
    if matches!(error, rustix::io::Errno::NOTDIR | rustix::io::Errno::LOOP)
        && fs::symlink_metadata(root)
            .map(|metadata| metadata.file_type().is_symlink())
            .unwrap_or(false)
    {
        SameObjectConfinedReadError::Symlink
    } else {
        map_strong_open(error)
    }
}

#[cfg(unix)]
fn map_strong_open_at(
    directory: &std::os::fd::OwnedFd,
    component: &std::ffi::OsStr,
    error: rustix::io::Errno,
) -> SameObjectConfinedReadError {
    use rustix::fs::{AtFlags, FileType, statat};

    if matches!(error, rustix::io::Errno::NOTDIR | rustix::io::Errno::LOOP)
        && statat(directory, component, AtFlags::SYMLINK_NOFOLLOW)
            .map(|stat| FileType::from_raw_mode(stat.st_mode).is_symlink())
            .unwrap_or(false)
    {
        SameObjectConfinedReadError::Symlink
    } else {
        map_strong_open(error)
    }
}

pub fn validate_confined_regular_file(
    root: &Path,
    relative: &Path,
    maximum_bytes: u64,
) -> Result<PathBuf, ConfinedFileError> {
    let path = validate_confined_path(root, relative, true)?;
    let metadata = fs::metadata(&path).map_err(map_io)?;
    if metadata.len() > maximum_bytes {
        return Err(ConfinedFileError::TooLarge);
    }
    Ok(path)
}

pub fn validate_confined_output_file(
    root: &Path,
    relative: &Path,
) -> Result<PathBuf, ConfinedFileError> {
    validate_confined_path(root, relative, false)
}

fn validate_confined_path(
    root: &Path,
    relative: &Path,
    require_file: bool,
) -> Result<PathBuf, ConfinedFileError> {
    if relative.as_os_str().is_empty() || relative.is_absolute() {
        return Err(ConfinedFileError::InvalidRelativePath);
    }
    let root_metadata = fs::symlink_metadata(root).map_err(map_io)?;
    if root_metadata.file_type().is_symlink() {
        return Err(ConfinedFileError::Symlink);
    }
    if !root_metadata.is_dir() {
        return Err(ConfinedFileError::NonDirectoryParent);
    }

    let components = relative.components().collect::<Vec<_>>();
    if components
        .iter()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(ConfinedFileError::InvalidRelativePath);
    }

    let mut current = root.to_path_buf();
    for (index, component) in components.iter().enumerate() {
        let Component::Normal(part) = component else {
            unreachable!("components were validated")
        };
        current.push(part);
        let final_component = index + 1 == components.len();
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(ConfinedFileError::Symlink);
            }
            Ok(metadata) if final_component && !metadata.is_file() => {
                return Err(ConfinedFileError::NonRegularFile);
            }
            Ok(metadata) if !final_component && !metadata.is_dir() => {
                return Err(ConfinedFileError::NonDirectoryParent);
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if require_file || !final_component {
                    return Err(ConfinedFileError::Missing);
                }
            }
            Err(_) => return Err(ConfinedFileError::Io),
        }
    }
    Ok(current)
}

fn validate_relative_path(relative: &Path) -> Result<(), ConfinedFileError> {
    if relative.as_os_str().is_empty()
        || relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        Err(ConfinedFileError::InvalidRelativePath)
    } else {
        Ok(())
    }
}

fn map_io(error: std::io::Error) -> ConfinedFileError {
    if error.kind() == std::io::ErrorKind::NotFound {
        ConfinedFileError::Missing
    } else {
        ConfinedFileError::Io
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    use crate::allocation::{RejectReservationAt, ReservationSite};

    #[test]
    fn regular_confined_file_is_bounded_and_parent_symlinks_are_rejected() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("a/b")).unwrap();
        fs::write(root.path().join("a/b/input.toml"), b"abc").unwrap();
        assert_eq!(
            read_confined_regular_file(root.path(), Path::new("a/b/input.toml"), 3).unwrap(),
            b"abc"
        );
        assert_eq!(
            read_confined_regular_file(root.path(), Path::new("a/b/input.toml"), 2),
            Err(ConfinedFileError::TooLarge)
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            fs::rename(root.path().join("a"), root.path().join("real-a")).unwrap();
            symlink(root.path().join("real-a"), root.path().join("a")).unwrap();
            assert_eq!(
                read_confined_regular_file(root.path(), Path::new("a/b/input.toml"), 3),
                Err(ConfinedFileError::Symlink)
            );
        }
    }

    #[test]
    fn output_validation_allows_only_a_missing_final_file() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("generated")).unwrap();
        assert_eq!(
            validate_confined_output_file(root.path(), Path::new("generated/summary.toml"))
                .unwrap(),
            root.path().join("generated/summary.toml")
        );
        assert_eq!(
            validate_confined_output_file(root.path(), Path::new("missing/summary.toml")),
            Err(ConfinedFileError::Missing)
        );
    }

    #[cfg(unix)]
    #[test]
    fn same_object_reader_retains_the_opened_file_across_path_replacement() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("capture.txt"), b"opened-object").unwrap();
        let opened = open_confined_regular_file(root.path(), Path::new("capture.txt")).unwrap();

        fs::rename(
            root.path().join("capture.txt"),
            root.path().join("original.txt"),
        )
        .unwrap();
        fs::write(root.path().join("capture.txt"), b"replacement").unwrap();

        assert_eq!(
            read_opened_regular_file_bounded(opened, 32).unwrap(),
            b"opened-object"
        );
    }

    #[cfg(unix)]
    #[test]
    fn same_object_reader_maps_initial_and_growth_reservation_failures() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("capture.txt"), b"bounded bytes").unwrap();

        for site in [
            ReservationSite::ConfinedReadInitial,
            ReservationSite::ConfinedReadGrowth,
        ] {
            let opened = open_confined_regular_file(root.path(), Path::new("capture.txt")).unwrap();
            assert_eq!(
                read_opened_regular_file_bounded_with_policy(
                    opened,
                    64,
                    &mut RejectReservationAt::new(site),
                ),
                Err(SameObjectConfinedReadError::Allocation)
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn same_object_reader_rejects_intermediate_and_final_symlinks() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("real")).unwrap();
        fs::write(root.path().join("real/capture.txt"), b"capture").unwrap();
        symlink(root.path().join("real"), root.path().join("linked")).unwrap();
        symlink(
            root.path().join("real/capture.txt"),
            root.path().join("capture-link.txt"),
        )
        .unwrap();

        assert_eq!(
            read_confined_regular_file_same_object(
                root.path(),
                Path::new("linked/capture.txt"),
                32,
            ),
            Err(SameObjectConfinedReadError::Symlink)
        );
        assert_eq!(
            read_confined_regular_file_same_object(root.path(), Path::new("capture-link.txt"), 32,),
            Err(SameObjectConfinedReadError::Symlink)
        );
    }

    #[cfg(unix)]
    #[test]
    fn same_object_reader_rejects_a_symlinked_repository_root() {
        use std::os::unix::fs::symlink;

        let parent = tempfile::tempdir().unwrap();
        let real = parent.path().join("real");
        fs::create_dir(&real).unwrap();
        fs::write(real.join("capture.txt"), b"capture").unwrap();
        let linked = parent.path().join("linked");
        symlink(&real, &linked).unwrap();

        assert_eq!(
            read_confined_regular_file_same_object(&linked, Path::new("capture.txt"), 32),
            Err(SameObjectConfinedReadError::Symlink)
        );
    }

    #[cfg(unix)]
    #[test]
    fn same_object_reader_rejects_a_fifo_without_blocking() {
        use std::sync::mpsc;
        use std::time::Duration;

        let root = tempfile::tempdir().unwrap();
        #[cfg(target_vendor = "apple")]
        let fifo = root.path().join("capture.fifo");

        #[cfg(target_vendor = "apple")]
        assert!(
            std::process::Command::new("/usr/bin/mkfifo")
                .arg(&fifo)
                .status()
                .unwrap()
                .success()
        );

        #[cfg(not(target_vendor = "apple"))]
        {
            use rustix::fs::{Mode, OFlags, mkfifoat, open};
            let directory = open(
                root.path(),
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .unwrap();
            mkfifoat(&directory, "capture.fifo", Mode::RUSR | Mode::WUSR).unwrap();
        }

        let root_path = root.path().to_owned();
        let (sender, receiver) = mpsc::channel();
        let reader = std::thread::spawn(move || {
            sender
                .send(read_confined_regular_file_same_object(
                    &root_path,
                    Path::new("capture.fifo"),
                    32,
                ))
                .unwrap();
        });
        assert_eq!(
            receiver.recv_timeout(Duration::from_secs(2)).unwrap(),
            Err(SameObjectConfinedReadError::NonRegularFile)
        );
        reader.join().unwrap();
    }

    #[test]
    fn historical_reader_keeps_its_existing_contract() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("input.toml"), b"abc").unwrap();
        assert_eq!(
            read_confined_regular_file(root.path(), Path::new("input.toml"), 3),
            Ok(b"abc".to_vec())
        );
        assert_eq!(
            read_confined_regular_file(root.path(), Path::new("input.toml"), 2),
            Err(ConfinedFileError::TooLarge)
        );
    }

    #[cfg(unix)]
    #[test]
    fn same_object_reader_rejects_unsafe_non_regular_and_oversized_inputs() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("bounded.txt"), b"abcd").unwrap();
        fs::create_dir(root.path().join("directory")).unwrap();

        assert_eq!(
            read_confined_regular_file_same_object(root.path(), Path::new("../bounded.txt"), 4),
            Err(SameObjectConfinedReadError::InvalidRelativePath)
        );
        assert_eq!(
            read_confined_regular_file_same_object(root.path(), Path::new("directory"), 4),
            Err(SameObjectConfinedReadError::NonRegularFile)
        );
        assert_eq!(
            read_confined_regular_file_same_object(root.path(), Path::new("bounded.txt"), 4),
            Ok(b"abcd".to_vec())
        );
        assert_eq!(
            read_confined_regular_file_same_object(root.path(), Path::new("bounded.txt"), 3),
            Err(SameObjectConfinedReadError::TooLarge)
        );
    }

    #[cfg(not(unix))]
    #[test]
    fn same_object_reader_fails_closed_on_unsupported_hosts() {
        assert_eq!(
            read_confined_regular_file_same_object(
                Path::new("repository"),
                Path::new("capture.txt"),
                32,
            ),
            Err(SameObjectConfinedReadError::UnsupportedPlatform),
        );
    }
}
