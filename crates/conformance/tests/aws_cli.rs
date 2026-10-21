use std::process::Command;
#[test]
fn local_build_identity_is_optional_and_aws_arguments_fail_before_credentials() {
    let exe = env!("CARGO_BIN_EXE_borrowser-conformance");
    let info = Command::new(exe)
        .arg("--ag4-artifact-info")
        .output()
        .unwrap();
    assert!(info.status.success());
    let v: serde_json::Value = serde_json::from_slice(&info.stdout).unwrap();
    fn declarations(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                declarations(&entry.path(), out);
            } else if entry
                .file_name()
                .to_str()
                .unwrap()
                .ends_with(".fixture.json")
            {
                out.push(entry.path());
            }
        }
    }
    fn selection(value: &serde_json::Value, compared: serde_json::Value) -> serde_json::Value {
        use serde_json::json;
        if value.is_null() {
            return json!({"kind":"not_requested"});
        }
        match value["kind"].as_str().unwrap() {
            "compare" => json!({"kind":"compare","value":compared}),
            "unsupported" => json!({"kind":"unsupported","value":value["reason"]}),
            _ => panic!("fixture request"),
        }
    }
    let mut paths = vec![];
    declarations(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures"),
        &mut paths,
    );
    let mut expected = vec![];
    for path in paths {
        use serde_json::{Value, json};
        use sha2::Digest;
        let m: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        if m["classification"]["kind"] != "runnable" {
            continue;
        }
        let o = &m["observations"];
        let styles=o["styles"].as_array().map(|s|s.iter().map(|f|json!({"target":f["target"],"property":f["property"],"selection":selection(&f["selection"],Value::Null)})).collect::<Vec<_>>()).unwrap_or_default();
        let request = json!({"dom":selection(&o["dom"],Value::Null),"styles":styles,"layout":selection(&o["layout"],o["layout"]["targets"].clone()),"canvas":selection(&o["canvas"],Value::Null)});
        for (role, input) in [
            ("test", &m["input"]),
            ("reference", &m["relationship"]["input"]),
        ] {
            if input.is_null() {
                continue;
            }
            let bytes =
                std::fs::read(path.parent().unwrap().join(input.as_str().unwrap())).unwrap();
            expected.push(json!({"case_id":m["id"],"role":role,"source_sha256":format!("{:x}",sha2::Sha256::digest(bytes)),"request":request}));
        }
    }
    expected.sort_by_key(|d| {
        (
            d["case_id"].as_str().unwrap().to_owned(),
            d["role"] == "reference",
        )
    });
    assert!(!expected.is_empty());
    assert_eq!(v["fixtures"], serde_json::json!(expected));
    for args in [
        vec!["--compare-aws"],
        vec!["--aws-cleanup"],
        vec!["--compare-aws", "unknown"],
    ] {
        let o = Command::new(exe)
            .args(args)
            .env("AWS_ACCESS_KEY_ID", "invalid-test-value")
            .output()
            .unwrap();
        assert_eq!(o.status.code(), Some(2));
        assert!(o.stdout.is_empty());
        assert!(!o.stderr.is_empty());
    }
}
