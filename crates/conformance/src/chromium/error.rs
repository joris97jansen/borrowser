use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Phase {
    Startup,
    Navigation,
    Capture,
    Shutdown,
}

#[derive(Debug)]
pub(super) enum Error {
    Configuration(String),
    UnsupportedPlatform(String),
    IncompatibleBrowser(String),
    Launch {
        stage: &'static str,
        errno: i32,
    },
    Io {
        operation: &'static str,
        source: std::io::Error,
    },
    Protocol(ProtocolError),
    Navigation(String),
    Capture(String),
    Timeout(Phase),
    Cancelled,
    BrowserExited,
}

#[derive(Debug)]
pub(super) enum ProtocolError {
    Eof,
    Truncated,
    Oversized,
    Malformed(String),
    UnexpectedResponse,
    Remote {
        method: String,
        code: i64,
        message: String,
    },
}

#[derive(Debug)]
pub(super) enum CleanupError {
    Identity(String),
    Signal(String),
    Reap(String),
    Timeout,
    Artifacts(std::io::Error),
}

#[derive(Debug)]
pub(super) struct Failure {
    pub primary: Option<Error>,
    pub cleanup: Vec<CleanupError>,
}

impl Error {
    pub fn io(operation: &'static str, source: std::io::Error) -> Self {
        Self::Io { operation, source }
    }
    pub fn code(&self) -> &'static str {
        match self {
            Self::Configuration(_) => "configuration",
            Self::UnsupportedPlatform(_) => "platform",
            Self::IncompatibleBrowser(_) => "identity",
            Self::Launch { .. } => "launch",
            Self::Io { .. } => "transport",
            Self::Protocol(_) => "protocol",
            Self::Navigation(_) => "navigation",
            Self::Capture(_) => "capture",
            Self::Timeout(_) => "timeout",
            Self::Cancelled => "cancelled",
            Self::BrowserExited => "browser-exited",
        }
    }
}
impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Eof => write!(f, "premature pipe EOF"),
            Self::Truncated => write!(f, "EOF inside a protocol frame"),
            Self::Oversized => write!(f, "protocol frame exceeds limit"),
            Self::Malformed(s) => write!(f, "malformed protocol: {s}"),
            Self::UnexpectedResponse => write!(f, "unexpected response identity"),
            Self::Remote {
                method,
                code,
                message,
            } => write!(f, "{method}: {code}: {message}"),
        }
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Configuration(s)
            | Self::UnsupportedPlatform(s)
            | Self::IncompatibleBrowser(s)
            | Self::Navigation(s)
            | Self::Capture(s) => f.write_str(s),
            Self::Launch { stage, errno } => {
                write!(f, "{stage}: {}", std::io::Error::from_raw_os_error(*errno))
            }
            Self::Io { operation, source } => write!(f, "{operation}: {source}"),
            Self::Protocol(e) => e.fmt(f),
            Self::Timeout(p) => write!(f, "{p:?} deadline expired"),
            Self::Cancelled => write!(f, "capture cancelled"),
            Self::BrowserExited => write!(f, "browser exited before capture completed"),
        }
    }
}
impl fmt::Display for CleanupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Identity(s) => write!(f, "identity verification: {s}"),
            Self::Signal(s) => write!(f, "termination: {s}"),
            Self::Reap(s) => write!(f, "reaping: {s}"),
            Self::Timeout => write!(
                f,
                "owned process termination could not be verified before deadline"
            ),
            Self::Artifacts(e) => write!(f, "artifact removal: {e}"),
        }
    }
}
