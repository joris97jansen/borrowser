//! Test-only record of the pinned macOS CfT structure for future re-enablement.
//! This validator does not authorize macOS Chromium execution.
//! Archive provenance belongs to qualification setup. This is not attestation,
//! recursive resource validation, or protection against concurrent bundle edits.
use super::error::Error;
use core_foundation::{
    base::{CFType, TCFType},
    data::CFData,
    dictionary::CFDictionary,
    propertylist::{create_with_data, kCFPropertyListImmutable},
    string::CFString,
};
use std::{
    fs::{self, OpenOptions},
    io::Read,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Component, Path, PathBuf},
};

const APP: &str = "Google Chrome for Testing.app";
const EXE: &str = "Google Chrome for Testing";
const FRAMEWORK: &str = "Google Chrome for Testing Framework";
const MAX_PLIST_BYTES: u64 = 65_536;
// ChildProcessHost::GetChildPath at the pinned revision selects these three
// roles and their Aperitif variants. Alerts helpers serve out-of-scope native
// notification UI. Full archive materialization still includes every helper.
const HELPERS: [(&str, &str); 6] = [
    ("", "com.google.chrome.for.testing.helper"),
    (" (GPU)", "com.google.chrome.for.testing.helper"),
    (
        " (Renderer)",
        "com.google.chrome.for.testing.helper.renderer",
    ),
    (" (Aperitif)", "com.google.chrome.for.testing.helper"),
    (" (Aperitif GPU)", "com.google.chrome.for.testing.helper"),
    (
        " (Aperitif Renderer)",
        "com.google.chrome.for.testing.helper.renderer",
    ),
];
fn invalid(detail: impl std::fmt::Display) -> Error {
    Error::Configuration(format!("macOS CfT bundle: {detail}"))
}
#[derive(Clone, Copy)]
enum Entry {
    Directory,
    File,
    Executable,
}
// All traversed ordinary components must be real entries. The five official
// framework aliases are checked separately against their exact relative targets.
fn entry(app: &Path, relative: &Path, kind: Entry) -> Result<PathBuf, Error> {
    let mut path = app.to_owned();
    let mut components = relative.components().peekable();
    while let Some(component) = components.next() {
        let Component::Normal(name) = component else {
            return Err(invalid("non-relative required path"));
        };
        path.push(name);
        let metadata = fs::symlink_metadata(&path)
            .map_err(|e| invalid(format!("{}: {e}", relative.display())))?;
        let expected = if components.peek().is_some() {
            Entry::Directory
        } else {
            kind
        };
        let valid = match expected {
            Entry::Directory => metadata.is_dir(),
            Entry::File => metadata.is_file(),
            Entry::Executable => metadata.is_file() && metadata.permissions().mode() & 0o111 != 0,
        };
        if !valid {
            return Err(invalid(format!(
                "wrong entry type/permissions: {}",
                relative.display()
            )));
        }
    }
    Ok(path)
}
fn metadata(
    app: &Path,
    relative: &Path,
    identifier: &str,
    executable: &str,
    version: &str,
    helper: bool,
) -> Result<(), Error> {
    let path = entry(app, relative, Entry::File)?;
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(&path)
        .map_err(|e| invalid(format!("{}: {e}", relative.display())))?;
    let stat = file.metadata().map_err(invalid)?;
    if !stat.is_file() || stat.len() > MAX_PLIST_BYTES {
        return Err(invalid("plist is not a bounded regular file"));
    }
    let mut bytes = Vec::new();
    file.take(MAX_PLIST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(invalid)?;
    if bytes.len() as u64 > MAX_PLIST_BYTES {
        return Err(invalid("plist byte bound exceeded"));
    }
    // CoreFoundation parses XML and binary plists. No handwritten XML matching
    // or unchecked conversion of property types; no arbitrary plist values in errors.
    let (raw, _) = create_with_data(CFData::from_buffer(&bytes), kCFPropertyListImmutable)
        .map_err(|_| invalid(format!("malformed plist: {}", relative.display())))?;
    // SAFETY: create_with_data returned a retained, non-null property-list object.
    let value = unsafe { CFType::wrap_under_create_rule(raw) };
    let dictionary = value
        .downcast::<CFDictionary>()
        .ok_or_else(|| invalid("plist root must be a dictionary"))?;
    for (key, expected) in [
        ("CFBundleIdentifier", identifier),
        ("CFBundleExecutable", executable),
        ("CFBundleShortVersionString", version),
    ]
    .into_iter()
    .chain(helper.then_some(("LSUIElement", "1")))
    {
        let key_value = CFString::new(key);
        let actual = dictionary.find(key_value.as_CFTypeRef()).and_then(|raw| {
            // SAFETY: plist dictionary values are CF objects, retained by the
            // dictionary. Take a reference before checking the dynamic type.
            unsafe { CFType::wrap_under_get_rule(*raw) }.downcast::<CFString>()
        });
        if actual.as_ref() != Some(&CFString::new(expected)) {
            return Err(invalid(format!(
                "{}: missing, mistyped or contradictory {key}",
                relative.display()
            )));
        }
    }
    Ok(())
}

pub(super) fn validate(executable: &Path) -> Result<PathBuf, Error> {
    let suffix = Path::new("Contents/MacOS").join(EXE);
    if !executable.ends_with(&suffix) {
        return Err(invalid("expected Contents/MacOS/Google Chrome for Testing"));
    }
    let app = executable
        .ancestors()
        .nth(3)
        .ok_or_else(|| invalid("missing containing app"))?
        .canonicalize()
        .map_err(invalid)?;
    if app.file_name().and_then(|s| s.to_str()) != Some(APP) || !app.is_dir() {
        return Err(invalid("expected containing Google Chrome for Testing.app"));
    }
    let pin: serde_json::Value =
        serde_json::from_str(include_str!("../../chromium-reference.json"))
            .map_err(|_| invalid("invalid repository browser pin"))?;
    let version = pin["version"]
        .as_str()
        .ok_or_else(|| invalid("missing pinned version"))?;
    let executable = entry(&app, &suffix, Entry::Executable)?;
    entry(&app, Path::new("Contents/Resources"), Entry::Directory)?;
    metadata(
        &app,
        Path::new("Contents/Info.plist"),
        "com.google.chrome.for.testing",
        EXE,
        version,
        false,
    )?;
    let framework = Path::new("Contents/Frameworks").join(format!("{FRAMEWORK}.framework"));
    let versioned = framework.join("Versions").join(version);
    for directory in ["Helpers", "Libraries", "Resources"] {
        entry(&app, &versioned.join(directory), Entry::Directory)?;
    }
    entry(&app, &versioned.join(FRAMEWORK), Entry::Executable)?;
    entry(
        &app,
        &versioned.join("Helpers/chrome_crashpad_handler"),
        Entry::Executable,
    )?;
    metadata(
        &app,
        &versioned.join("Resources/Info.plist"),
        "com.google.chrome.for.testing.framework",
        FRAMEWORK,
        version,
        false,
    )?;
    for (suffix, identifier) in HELPERS {
        let name = format!("Google Chrome for Testing Helper{suffix}");
        let contents = versioned
            .join("Helpers")
            .join(format!("{name}.app/Contents"));
        entry(&app, &contents.join("MacOS").join(&name), Entry::Executable)?;
        metadata(
            &app,
            &contents.join("Info.plist"),
            identifier,
            &name,
            version,
            true,
        )?;
    }
    for (alias, target) in std::iter::once(("Versions/Current".to_owned(), version.to_owned()))
        .chain(
            [FRAMEWORK, "Helpers", "Libraries", "Resources"]
                .into_iter()
                .map(|name| (name.to_owned(), format!("Versions/Current/{name}"))),
        )
    {
        let path = app.join(&framework).join(alias);
        if fs::read_link(&path).map_err(invalid)? != Path::new(&target) {
            return Err(invalid("unexpected framework symlink target"));
        }
        if !path.canonicalize().map_err(invalid)?.starts_with(&app) {
            return Err(invalid("framework symlink escapes application"));
        }
    }
    Ok(executable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chromium::bundle_fixture::{Bundle, INVALID, invalidate};
    #[test]
    fn minimal_pinned_bundle_and_official_framework_links() {
        let bundle = Bundle::new();
        assert_eq!(
            validate(&bundle.executable).unwrap(),
            bundle.executable.canonicalize().unwrap()
        );
        // Required structure only: there are no locale/resource payloads here.
        assert!(
            fs::read_dir(bundle.app.join("Contents/Resources"))
                .unwrap()
                .next()
                .is_none()
        );
    }
    #[test]
    fn rejects_incomplete_contradictory_and_escaping_bundles() {
        for case in INVALID {
            let bundle = Bundle::new();
            invalidate(&bundle, case);
            assert!(
                matches!(validate(&bundle.executable), Err(Error::Configuration(_))),
                "{case}"
            );
        }
    }
    #[test]
    fn binary_plist_uses_the_same_typed_metadata_contract() {
        let bundle = Bundle::new();
        let path = bundle.app.join("Contents/Info.plist");
        let (raw, _) = create_with_data(
            CFData::from_buffer(&fs::read(&path).unwrap()),
            kCFPropertyListImmutable,
        )
        .unwrap();
        // SAFETY: transfer ownership of the created property list exactly once.
        let value = unsafe { CFType::wrap_under_create_rule(raw) };
        let binary = core_foundation::propertylist::create_data(
            value.as_CFTypeRef(),
            core_foundation::propertylist::kCFPropertyListBinaryFormat_v1_0,
        )
        .unwrap();
        fs::write(path, binary.bytes()).unwrap();
        assert!(validate(&bundle.executable).is_ok());
    }
}
