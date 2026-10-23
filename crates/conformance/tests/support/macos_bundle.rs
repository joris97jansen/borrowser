//! Test-owned minimal pinned bundle. No browser discovery, download or launch.
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::{Path, PathBuf},
};

pub const APP: &str = "Google Chrome for Testing.app";
pub const EXE: &str = "Google Chrome for Testing";
pub const FRAMEWORK: &str = "Google Chrome for Testing Framework";
pub const HELPERS: [(&str, &str); 6] = [
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
pub fn version() -> String {
    serde_json::from_str::<serde_json::Value>(include_str!("../../chromium-reference.json"))
        .unwrap()["version"]
        .as_str()
        .unwrap()
        .into()
}
pub struct Bundle {
    pub directory: tempfile::TempDir,
    pub app: PathBuf,
    pub executable: PathBuf,
    pub framework: PathBuf,
    pub versioned: PathBuf,
}
fn file(path: &Path, bytes: &[u8], executable: bool) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
    fs::set_permissions(
        path,
        fs::Permissions::from_mode(if executable { 0o755 } else { 0o644 }),
    )
    .unwrap();
}
fn plist(path: &Path, id: &str, exe: &str, helper: bool) {
    let background = if helper {
        "<key>LSUIElement</key><string>1</string>"
    } else {
        ""
    };
    file(path, format!("<?xml version=\"1.0\"?><plist version=\"1.0\"><dict><key>CFBundleIdentifier</key><string>{id}</string><key>CFBundleExecutable</key><string>{exe}</string><key>CFBundleShortVersionString</key><string>{}</string>{background}</dict></plist>", version()).as_bytes(), false);
}
impl Bundle {
    pub fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let app = directory.path().join(APP);
        let executable = app.join("Contents/MacOS").join(EXE);
        file(&executable, b"test-owned executable placeholder", true);
        fs::create_dir_all(app.join("Contents/Resources")).unwrap();
        plist(
            &app.join("Contents/Info.plist"),
            "com.google.chrome.for.testing",
            EXE,
            false,
        );
        let framework = app
            .join("Contents/Frameworks")
            .join(format!("{FRAMEWORK}.framework"));
        let versioned = framework.join("Versions").join(version());
        for dir in ["Helpers", "Libraries", "Resources"] {
            fs::create_dir_all(versioned.join(dir)).unwrap();
        }
        file(&versioned.join(FRAMEWORK), b"test-owned framework", true);
        file(
            &versioned.join("Helpers/chrome_crashpad_handler"),
            b"test-owned handler",
            true,
        );
        plist(
            &versioned.join("Resources/Info.plist"),
            "com.google.chrome.for.testing.framework",
            FRAMEWORK,
            false,
        );
        for (suffix, id) in HELPERS {
            let name = format!("Google Chrome for Testing Helper{suffix}");
            let contents = versioned
                .join("Helpers")
                .join(format!("{name}.app/Contents"));
            file(
                &contents.join("MacOS").join(&name),
                b"test-owned helper",
                true,
            );
            plist(&contents.join("Info.plist"), id, &name, true);
        }
        symlink(version(), framework.join("Versions/Current")).unwrap();
        for name in [FRAMEWORK, "Helpers", "Libraries", "Resources"] {
            symlink(format!("Versions/Current/{name}"), framework.join(name)).unwrap();
        }
        Self {
            directory,
            app,
            executable,
            framework,
            versioned,
        }
    }
    pub fn helper(&self, suffix: &str) -> PathBuf {
        self.versioned.join("Helpers").join(format!(
            "Google Chrome for Testing Helper{suffix}.app/Contents"
        ))
    }
    pub fn use_test_executable(&self) {
        fs::remove_file(&self.executable).unwrap();
        fs::copy(std::env::current_exe().unwrap(), &self.executable).unwrap();
    }
}

pub const INVALID: &[&str] = &[
    "missing-outer-plist",
    "malformed-plist",
    "wrong-identifier",
    "wrong-executable-field",
    "wrong-version",
    "missing-executable",
    "executable-directory",
    "missing-renderer",
    "missing-gpu",
    "missing-helper-plist",
    "missing-helper-executable",
    "wrong-helper-identity",
    "wrong-helper-version",
    "wrong-helper-executable",
    "wrong-helper-background",
    "plist-directory",
    "plist-oversized",
    "plist-non-dictionary",
    "plist-wrong-field-type",
    "plist-fifo",
    "outer-directory-type",
    "framework-escape",
    "plist-escape",
    "executable-escape",
    "helper-directory-escape",
    "wrong-framework-identity",
    "missing-framework-binary",
    "framework-link-directory",
    "non-executable",
    "missing-aperitif-renderer",
];
pub fn invalidate(bundle: &Bundle, case: &str) {
    let outer = bundle.app.join("Contents/Info.plist");
    let renderer = bundle.helper(" (Renderer)");
    let replace = |path: &Path, from: &str, to: &str| {
        let before = fs::read_to_string(path).unwrap();
        assert!(before.contains(from));
        fs::write(path, before.replace(from, to)).unwrap();
    };
    match case {
        "missing-outer-plist" => fs::remove_file(outer).unwrap(),
        "malformed-plist" => fs::write(outer, b"<plist><dict>").unwrap(),
        "wrong-identifier" => replace(&outer, "com.google.chrome.for.testing", "wrong.identifier"),
        "wrong-executable-field" => replace(&outer, EXE, "../outside"),
        "wrong-version" => replace(&outer, &version(), "0.0.0.0"),
        "missing-executable" => fs::remove_file(&bundle.executable).unwrap(),
        "executable-directory" => {
            fs::remove_file(&bundle.executable).unwrap();
            fs::create_dir(&bundle.executable).unwrap();
        }
        "missing-renderer" => fs::remove_dir_all(renderer.parent().unwrap()).unwrap(),
        "missing-gpu" => fs::remove_dir_all(bundle.helper(" (GPU)").parent().unwrap()).unwrap(),
        "missing-aperitif-renderer" => {
            fs::remove_dir_all(bundle.helper(" (Aperitif Renderer)").parent().unwrap()).unwrap()
        }
        "missing-helper-plist" => fs::remove_file(renderer.join("Info.plist")).unwrap(),
        "missing-helper-executable" => {
            fs::remove_file(renderer.join("MacOS/Google Chrome for Testing Helper (Renderer)"))
                .unwrap()
        }
        "wrong-helper-identity" => replace(
            &renderer.join("Info.plist"),
            "com.google.chrome.for.testing.helper.renderer",
            "wrong.helper",
        ),
        "wrong-helper-version" => replace(&renderer.join("Info.plist"), &version(), "0.0.0.0"),
        "wrong-helper-executable" => replace(
            &renderer.join("Info.plist"),
            "Google Chrome for Testing Helper (Renderer)",
            "../outside",
        ),
        "wrong-helper-background" => replace(
            &renderer.join("Info.plist"),
            "<string>1</string>",
            "<string>0</string>",
        ),
        "plist-directory" => {
            fs::remove_file(&outer).unwrap();
            fs::create_dir(outer).unwrap();
        }
        "plist-oversized" => fs::write(outer, vec![b' '; 65537]).unwrap(),
        "plist-non-dictionary" => {
            fs::write(outer, b"<plist version=\"1.0\"><array/></plist>").unwrap()
        }
        "plist-wrong-field-type" => replace(
            &outer,
            "<string>com.google.chrome.for.testing</string>",
            "<integer>1</integer>",
        ),
        "plist-fifo" => {
            fs::remove_file(&outer).unwrap();
            let path = std::ffi::CString::new(outer.as_os_str().as_encoded_bytes()).unwrap();
            assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
        }
        "outer-directory-type" => {
            let path = bundle.app.join("Contents/Resources");
            fs::remove_dir(&path).unwrap();
            fs::write(path, b"not directory").unwrap();
        }
        "framework-escape" => {
            let link = bundle.framework.join("Versions/Current");
            fs::remove_file(&link).unwrap();
            symlink(bundle.directory.path(), link).unwrap();
        }
        "plist-escape" => {
            let outside = bundle.directory.path().join("outside.plist");
            fs::rename(&outer, &outside).unwrap();
            symlink(outside, outer).unwrap();
        }
        "executable-escape" => {
            let outside = bundle.directory.path().join("outside-executable");
            fs::rename(&bundle.executable, &outside).unwrap();
            symlink(outside, &bundle.executable).unwrap();
        }
        "helper-directory-escape" => {
            let outside = bundle.directory.path().join("outside-helper");
            let helper = renderer.parent().unwrap();
            fs::rename(helper, &outside).unwrap();
            symlink(outside, helper).unwrap();
        }
        "wrong-framework-identity" => replace(
            &bundle.versioned.join("Resources/Info.plist"),
            "com.google.chrome.for.testing.framework",
            "wrong.framework",
        ),
        "missing-framework-binary" => fs::remove_file(bundle.versioned.join(FRAMEWORK)).unwrap(),
        "framework-link-directory" => {
            let link = bundle.framework.join("Helpers");
            fs::remove_file(&link).unwrap();
            fs::create_dir(link).unwrap();
        }
        "non-executable" => {
            fs::set_permissions(&bundle.executable, fs::Permissions::from_mode(0o644)).unwrap()
        }
        _ => panic!("unknown invalid bundle case: {case}"),
    }
}
