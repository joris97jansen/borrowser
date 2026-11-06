use addressing::{BrowserUrl, FilePathConversionError};

fn url(input: &str) -> BrowserUrl {
    BrowserUrl::parse(input).unwrap().url
}

#[cfg(unix)]
#[test]
fn file_paths_use_native_decoding_without_query_or_fragment() {
    for (input, expected) in [
        ("file:///tmp/My Page.html", "/tmp/My Page.html"),
        ("file:///tmp/My%20Page.html", "/tmp/My Page.html"),
        ("file:///tmp/My%2520Page.html", "/tmp/My%20Page.html"),
        ("file:///tmp/caf%C3%A9.html", "/tmp/café.html"),
        ("file://localhost/tmp/a?query#fragment", "/tmp/a"),
        ("file:///tmp/a%3Fb%23c?query#fragment", "/tmp/a?b#c"),
        ("file:///tmp/a%2Fb", "/tmp/a/b"),
        ("file:///tmp/unchanged%ZZ", "/tmp/unchanged%ZZ"),
    ] {
        let parsed = url(input);
        let before = parsed.as_str().to_owned();
        assert_eq!(
            parsed.to_file_path().unwrap(),
            std::path::Path::new(expected)
        );
        assert_eq!(
            parsed.as_str(),
            before,
            "conversion must not mutate provenance"
        );
    }
}

#[cfg(unix)]
#[test]
fn unix_file_paths_preserve_native_non_utf8_bytes_and_reject_remote_authorities() {
    use std::os::unix::ffi::OsStrExt;
    assert_eq!(
        url("file:///tmp/%FF")
            .to_file_path()
            .unwrap()
            .as_os_str()
            .as_bytes(),
        b"/tmp/\xff"
    );
    for input in ["file://server.test/share/file", "file://127.0.0.1/tmp/file"] {
        assert_eq!(url(input).to_file_path(), Err(FilePathConversionError));
    }
}

#[test]
fn file_path_failures_are_fixed_and_reject_wrong_schemes_and_decoded_nul() {
    for input in [
        "https://PRIVATE.test/SECRET",
        "custom:/PRIVATE/SECRET",
        "file:///C:/PRIVATE/SECRET%00.html",
    ] {
        let error = url(input).to_file_path().unwrap_err();
        assert_eq!(error, FilePathConversionError);
        assert_eq!(format!("{error:?}"), "FilePathConversionError");
        assert_eq!(
            format!("{error}"),
            "URL cannot be converted to a native file path"
        );
    }
}

#[cfg(windows)]
#[test]
fn windows_file_paths_use_drive_and_unc_rules() {
    for (input, expected) in [
        (
            "file:///C:/My%20Page.html?query#fragment",
            r"C:\My Page.html",
        ),
        (
            "file://server/share/caf%C3%A9.html",
            r"\\server\share\café.html",
        ),
    ] {
        assert_eq!(
            url(input).to_file_path().unwrap(),
            std::path::Path::new(expected)
        );
    }
    for input in ["file:///missing-drive/file", "file:///C:/%FF"] {
        assert_eq!(url(input).to_file_path(), Err(FilePathConversionError));
    }
}
