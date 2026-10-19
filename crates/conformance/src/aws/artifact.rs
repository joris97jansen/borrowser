use super::{config, template};
use serde::{Deserialize, Serialize};
use std::path::Path;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct FixtureIdentity {
    id: String,
    source: String,
    sha256: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct Identity {
    pub source_tree: Option<String>,
    pub platform: String,
    fixtures: Vec<FixtureIdentity>,
    chromium_pin_sha256: String,
}
pub(super) fn identity() -> Identity {
    Identity {
        source_tree: option_env!("BORROWSER_AG4_SOURCE_TREE")
            .filter(|tree| !tree.is_empty())
            .map(Into::into),
        platform: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        fixtures: crate::fixtures::FIXTURES
            .iter()
            .filter_map(|f| match f.kind {
                crate::model::FixtureKind::Runnable { html, .. } => Some(FixtureIdentity {
                    id: f.id.0.into(),
                    source: f.source.into(),
                    sha256: template::sha256(html),
                }),
                _ => None,
            })
            .collect(),
        chromium_pin_sha256: template::sha256(include_bytes!("../../chromium-reference.json")),
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema: u32,
    sha256: String,
    identity: Identity,
}
pub(super) struct Prepared {
    pub bytes: Vec<u8>,
    pub sha256: String,
    pub identity: Identity,
}
impl Prepared {
    pub fn load(directory: &Path) -> Result<Self, String> {
        let own = identity();
        if own.source_tree.is_none() {
            return Err("AG4 requires a snapshot-stamped controller; local commands do not".into());
        }
        let manifest: Manifest = serde_json::from_value(template::json(&config::read(
            &directory.join("artifact.json"),
            16384,
        )?)?)
        .map_err(|e| e.to_string())?;
        let mut expected = own;
        expected.platform = "linux-x86_64".into();
        if manifest.schema != 1 || manifest.identity != expected {
            return Err("artifact/source/fixture/browser identity mismatch".into());
        }
        let executable = directory.join("borrowser-conformance-linux");
        let bytes = config::read(&executable, 64 * 1024 * 1024)?;
        if bytes.len() < 20
            || &bytes[..4] != b"\x7fELF"
            || bytes[4] != 2
            || bytes[5] != 1
            || bytes[18..20] != [62, 0]
        {
            return Err("expected Linux x86-64 ELF harness".into());
        }
        let sha256 = template::sha256(&bytes);
        if manifest.sha256 != sha256 {
            return Err("artifact digest mismatch".into());
        }
        Ok(Self {
            bytes,
            sha256,
            identity: expected,
        })
    }
    pub fn verify_remote(&self, bytes: &[u8]) -> Result<(), String> {
        let actual: Identity =
            serde_json::from_value(template::json(bytes)?).map_err(|e| e.to_string())?;
        if actual != self.identity {
            return Err("remote harness identity mismatch".into());
        }
        Ok(())
    }
}
