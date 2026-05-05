use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpTimeoutPolicy {
    pub connect: Duration,
    pub read: Duration,
    pub write: Duration,
}

impl Default for HttpTimeoutPolicy {
    fn default() -> Self {
        Self {
            connect: Duration::from_secs(15),
            read: Duration::from_secs(30),
            write: Duration::from_secs(30),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum TlsTrustStore {
    #[default]
    NativeRoots,
    NativeRootsWithAdditional(Vec<Vec<u8>>),
    CustomRoots(Vec<Vec<u8>>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpClientPolicy {
    pub user_agent: String,
    pub redirects: u32,
    pub timeouts: HttpTimeoutPolicy,
    pub tls: TlsTrustStore,
}

impl Default for HttpClientPolicy {
    fn default() -> Self {
        Self {
            user_agent: default_user_agent(),
            redirects: 10,
            timeouts: HttpTimeoutPolicy::default(),
            tls: TlsTrustStore::NativeRoots,
        }
    }
}

fn default_user_agent() -> String {
    format!("Borrowser/{}", env!("CARGO_PKG_VERSION"))
}
