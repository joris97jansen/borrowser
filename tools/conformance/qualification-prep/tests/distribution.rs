use qualification_prep::{
    canonical,
    distribution::{
        self, CandidateDistributionManifest as Manifest, DirectoryRecord, EntryRecord as Entry,
    },
};
fn regular(path: &str) -> Entry {
    Entry::Regular {
        path: path.into(),
        mode: 493,
        executable: true,
        byte_length: 1,
        sha256: canonical::hash(b"x"),
        file_capabilities: "absent".into(),
    }
}
fn specimen() -> Manifest {
    Manifest {
        format: distribution::FORMAT.into(),
        root_mode: 493,
        directories: vec![],
        entries: vec![regular("chrome")],
    }
}
fn link(path: &str, target: &str, resolved: &str) -> Entry {
    Entry::Symlink {
        path: path.into(),
        mode: 511,
        executable: false,
        target: target.into(),
        target_sha256: canonical::hash(target.as_bytes()),
        resolved_path: resolved.into(),
    }
}
#[test]
fn canonical_complete_golden() {
    let bytes = specimen().canonical_bytes().unwrap();
    assert_eq!(
        String::from_utf8(bytes).unwrap(),
        "format = \"borrowser-chromium-distribution-manifest-v1\"\nroot_mode = 493\ndirectories = []\n[[entries]]\npath = \"chrome\"\nkind = \"regular\"\nmode = 493\nexecutable = true\nbyte_length = 1\nsha256 = \"2d711642b726b04401627ca9fbac32f5c8530fb1903cc4db02258717921a4881\"\nfile_capabilities = \"absent\"\n"
    );
}
#[test]
fn paths_modes_population_and_size_bounds() {
    for path in [
        "/a".into(),
        "a//b".into(),
        "a/../b".into(),
        "a\\b".into(),
        "a".repeat(65),
        vec!["a".repeat(64); 4].join("/") + "/x",
    ] {
        let mut m = specimen();
        m.entries = vec![regular(&path)];
        assert!(m.validate().is_err(), "{path}");
    }
    for mode in [0o4777, 0o2775, 0o1777, 0o777, 0o775, 0o757] {
        let mut m = specimen();
        m.root_mode = mode;
        assert!(m.validate().is_err());
    }
    let mut m = specimen();
    m.entries.push(regular("chrome"));
    assert!(m.validate().is_err());
    m = specimen();
    m.entries = vec![regular("nested/chrome")];
    assert!(m.validate().is_err());
    m.directories.push(DirectoryRecord {
        path: "nested".into(),
        mode: 493,
    });
    assert!(m.validate().is_ok());
    m.directories.push(DirectoryRecord {
        path: "nested".into(),
        mode: 493,
    });
    assert!(m.validate().is_err());
    m = specimen();
    if let Entry::Regular { byte_length, .. } = &mut m.entries[0] {
        *byte_length = distribution::FILE_BYTES + 1;
    }
    assert!(m.validate().is_err());
    m.entries = (0..5)
        .map(|i| {
            let mut e = regular(&format!("c{i}"));
            if let Entry::Regular { byte_length, .. } = &mut e {
                *byte_length = distribution::FILE_BYTES;
            }
            e
        })
        .collect();
    assert!(m.validate().is_err());
    m.entries = (0..4097).map(|i| regular(&format!("c{i:04}"))).collect();
    assert!(m.validate().is_err());
    m = specimen();
    m.directories = (0..1025)
        .map(|i| DirectoryRecord {
            path: format!("d{i:04}"),
            mode: 493,
        })
        .collect();
    assert!(m.validate().is_err());
}
#[test]
fn links_exact_lexical_contract() {
    for target in [
        "/chrome",
        "../chrome",
        "chrome\\x",
        "chrome//x",
        "",
        "./",
        "dir",
    ] {
        let mut m = specimen();
        m.directories.push(DirectoryRecord {
            path: "dir".into(),
            mode: 493,
        });
        m.entries.push(link("z", target, "chrome"));
        assert!(m.validate().is_err(), "{target}");
    }
    let mut m = specimen();
    m.entries.push(link("z", "./chrome", "chrome"));
    assert!(m.validate().is_ok());
    if let Entry::Symlink { target_sha256, .. } = &mut m.entries[1] {
        *target_sha256 = "0".repeat(64);
    }
    assert!(m.validate().is_err());
    m = specimen();
    m.entries.push(link("y", "z", "chrome"));
    m.entries.push(link("z", "y", "chrome"));
    assert!(m.validate().is_err());
    for count in [16, 17] {
        let mut m = specimen();
        for i in 0..count {
            let target = if i + 1 == count {
                "chrome".into()
            } else {
                format!("l{:02}", i + 1)
            };
            m.entries.push(link(&format!("l{i:02}"), &target, "chrome"));
        }
        assert_eq!(m.validate().is_ok(), count == 16);
    }
}
#[test]
fn strict_input_and_exclusive_complete_publication() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("manifest");
    let bytes = specimen().canonical_bytes().unwrap();
    canonical::publish_new(&path, &bytes).unwrap();
    assert!(canonical::publish_new(&path, b"replacement").is_err());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert!(Manifest::load(&path).is_ok());
    std::fs::write(&path, [b"# comment\n".as_slice(), &bytes].concat()).unwrap();
    assert!(Manifest::load(&path).is_err());
    assert_eq!(std::fs::read_dir(d.path()).unwrap().count(), 1);
}
#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use std::os::unix::{
        fs::{PermissionsExt, symlink},
        net::UnixListener,
    };
    fn tree() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("chrome"), b"x").unwrap();
        std::fs::set_permissions(
            d.path().join("chrome"),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        d
    }
    #[test]
    fn complete_object_population_and_order() {
        let d = tree();
        std::fs::create_dir(d.path().join("resources")).unwrap();
        std::fs::write(d.path().join("resources/a"), b"a").unwrap();
        symlink("chrome", d.path().join("z")).unwrap();
        let a = distribution::inventory(d.path()).unwrap();
        let b = distribution::inventory(d.path()).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.entries.len(), 3);
        assert_eq!(a.directories.len(), 1);
        assert_eq!(a.entries[0].path(), "chrome");
    }
    #[test]
    fn rejects_root_symlink_socket_fifo_modes_and_oversize() {
        let d = tree();
        let parent = tempfile::tempdir().unwrap();
        symlink(d.path(), parent.path().join("root")).unwrap();
        assert!(distribution::inventory(&parent.path().join("root")).is_err());
        let sock = UnixListener::bind(d.path().join("socket")).unwrap();
        assert!(distribution::inventory(d.path()).is_err());
        drop(sock);
        std::fs::remove_file(d.path().join("socket")).unwrap();
        let name = std::ffi::CString::new(d.path().join("fifo").to_str().unwrap()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
        assert!(distribution::inventory(d.path()).is_err());
        std::fs::remove_file(d.path().join("fifo")).unwrap();
        for mode in [0o777, 0o4755, 0o2755, 0o1755] {
            std::fs::set_permissions(
                d.path().join("chrome"),
                std::fs::Permissions::from_mode(mode),
            )
            .unwrap();
            assert!(distribution::inventory(d.path()).is_err());
        }
        std::fs::set_permissions(
            d.path().join("chrome"),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        std::fs::File::options()
            .write(true)
            .open(d.path().join("chrome"))
            .unwrap()
            .set_len(distribution::FILE_BYTES + 1)
            .unwrap();
        assert!(distribution::inventory(d.path()).is_err());
    }
    #[test]
    fn no_candidate_after_invalid_input() {
        let d = tree();
        symlink("/etc/passwd", d.path().join("bad")).unwrap();
        let out = tempfile::tempdir().unwrap();
        let status = std::process::Command::new(env!("CARGO_BIN_EXE_qualification-prep"))
            .args(["manifest", "--distribution-root"])
            .arg(d.path())
            .arg("--output")
            .arg(out.path().join("candidate"))
            .status()
            .unwrap();
        assert!(!status.success());
        assert_eq!(std::fs::read_dir(out.path()).unwrap().count(), 0);
    }
}
#[cfg(target_os = "linux")]
#[test]
fn global_enumerated_population_is_bounded_before_file_hashing() {
    let d = tempfile::tempdir().unwrap();
    for directory in ["a", "b"] {
        let dir = d.path().join(directory);
        std::fs::create_dir(&dir).unwrap();
        for i in 0..2560 {
            std::fs::write(dir.join(format!("f{i:04}")), b"").unwrap();
        }
    }
    assert_eq!(
        distribution::inventory(d.path()).unwrap_err(),
        qualification_prep::Error::Invalid("total enumerated population")
    );
}
