use conformance_test_support::{LanePolicyScope, TestId};
use external_test_provenance::Sha256Digest;
use std::{ffi::OsString, path::PathBuf};

#[derive(Debug)]
pub(super) enum Command {
    Legacy,
    Aggregate(AggregateCommand),
}

#[derive(Debug)]
pub(super) enum AggregateCommand {
    Summary(LaneRequest),
    Detail(LaneRequest),
    BaselineWithoutEvaluation(LaneRequest),
    BaselineSelectedDom {
        request: LaneRequest,
        test_id: TestId,
    },
    Trend {
        from: BaselineInput,
        to: BaselineInput,
    },
}

#[derive(Clone, Copy, Debug)]
pub(super) struct LaneRequest {
    pub lane: LanePolicyScope,
    pub check: CheckPolicy,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum CheckPolicy {
    ReportOnly,
    RequireExpected,
}

#[derive(Debug)]
pub(super) struct BaselineInput {
    pub root: PathBuf,
    pub relative_path: PathBuf,
    pub expected_sha256: Sha256Digest,
}

#[derive(Debug)]
pub(super) struct Usage;
impl std::fmt::Display for Usage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("usage: conformance-runner aggregate summary|detail --lane L [--check]\n       conformance-runner aggregate baseline --lane L --external-evidence repository [--compare-dom-test TEST_ID] [--check]\n       conformance-runner aggregate trend --from-root ROOT --from RELATIVE_PATH --from-sha256 SHA256 --to-root ROOT --to RELATIVE_PATH --to-sha256 SHA256")
    }
}

pub(super) fn parse(mut args: impl Iterator<Item = OsString>) -> Result<Command, Usage> {
    if args.next().as_deref() != Some(std::ffi::OsStr::new("aggregate")) {
        return Ok(Command::Legacy);
    }
    let mode = args.next().ok_or(Usage)?;
    let allowed: &[&str] = match mode.to_str() {
        Some("summary" | "detail") => &["--lane", "--check"],
        Some("baseline") => &[
            "--lane",
            "--check",
            "--external-evidence",
            "--compare-dom-test",
        ],
        Some("trend") => &[
            "--from-root",
            "--from",
            "--from-sha256",
            "--to-root",
            "--to",
            "--to-sha256",
        ],
        _ => return Err(Usage),
    };
    // Slots are parser-only state. No incomplete request crosses this boundary.
    let mut values = std::collections::BTreeMap::new();
    while let Some(flag) = args.next() {
        let flag = flag.to_str().ok_or(Usage)?;
        let key = allowed
            .iter()
            .copied()
            .find(|key| *key == flag)
            .ok_or(Usage)?;
        let value = if key == "--check" {
            OsString::new()
        } else {
            let value = args.next().ok_or(Usage)?;
            if value.is_empty() {
                return Err(Usage);
            }
            value
        };
        if values.insert(key, value).is_some() {
            return Err(Usage);
        }
    }
    let text = |key| values.get(key).and_then(|v| v.to_str()).ok_or(Usage);
    let command = if mode == "trend" {
        let input = |root, path, digest| -> Result<BaselineInput, Usage> {
            Ok(BaselineInput {
                root: values.get(root).ok_or(Usage)?.into(),
                relative_path: values.get(path).ok_or(Usage)?.into(),
                expected_sha256: Sha256Digest::parse(text(digest)?).map_err(|_| Usage)?,
            })
        };
        AggregateCommand::Trend {
            from: input("--from-root", "--from", "--from-sha256")?,
            to: input("--to-root", "--to", "--to-sha256")?,
        }
    } else {
        let request = LaneRequest {
            lane: LanePolicyScope::parse(text("--lane")?).ok_or(Usage)?,
            check: if values.contains_key("--check") {
                CheckPolicy::RequireExpected
            } else {
                CheckPolicy::ReportOnly
            },
        };
        match mode.to_str() {
            Some("summary") => AggregateCommand::Summary(request),
            Some("detail") => AggregateCommand::Detail(request),
            Some("baseline") => {
                if text("--external-evidence")? != "repository" {
                    return Err(Usage);
                }
                match values.get("--compare-dom-test") {
                    None => AggregateCommand::BaselineWithoutEvaluation(request),
                    Some(id) => AggregateCommand::BaselineSelectedDom {
                        request,
                        test_id: TestId::parse(id.to_str().ok_or(Usage)?).map_err(|_| Usage)?,
                    },
                }
            }
            _ => return Err(Usage),
        }
    };
    Ok(Command::Aggregate(command))
}

#[cfg(all(test, unix))]
mod native_path_tests {
    use super::*;
    use std::os::unix::ffi::OsStringExt;

    #[test]
    fn trend_paths_retain_non_utf8_bytes_without_filesystem_assumptions() {
        // Some Unix filesystems (including this project's macOS hosts) cannot
        // create arbitrary byte names. Parsing must still preserve OsString.
        let root = OsString::from_vec(b"/root-\xff".to_vec());
        let path = OsString::from_vec(b"baseline-\xfe".to_vec());
        let digest = OsString::from("0".repeat(64));
        let args = [
            "aggregate".into(),
            "trend".into(),
            "--from-root".into(),
            root.clone(),
            "--from".into(),
            path.clone(),
            "--from-sha256".into(),
            digest.clone(),
            "--to-root".into(),
            root.clone(),
            "--to".into(),
            path.clone(),
            "--to-sha256".into(),
            digest,
        ];
        let Command::Aggregate(AggregateCommand::Trend { from, to }) =
            parse(args.into_iter()).unwrap()
        else {
            panic!("typed trend command")
        };
        for input in [from, to] {
            assert_eq!(input.root.as_os_str(), root);
            assert_eq!(input.relative_path.as_os_str(), path);
        }
    }
}
