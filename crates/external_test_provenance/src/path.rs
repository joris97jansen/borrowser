use std::fmt;
use std::path::{Component, Path};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UpstreamPath(String);

impl UpstreamPath {
    pub fn parse(value: &str) -> Result<Self, UpstreamPathParseError> {
        let path = Path::new(value);
        if value.is_empty()
            || value.contains('\\')
            || value.contains(':')
            || path.is_absolute()
            || path.components().any(|component| {
                matches!(
                    component,
                    Component::ParentDir
                        | Component::CurDir
                        | Component::RootDir
                        | Component::Prefix(_)
                )
            })
            || value.split('/').any(|component| component.is_empty())
        {
            return Err(UpstreamPathParseError);
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for UpstreamPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UpstreamPathParseError;

impl fmt::Display for UpstreamPathParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("upstream path must be a portable repository-relative path")
    }
}

impl std::error::Error for UpstreamPathParseError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_portable_relative_paths_are_accepted() {
        assert!(UpstreamPath::parse("css/reference/blank.html").is_ok());
        for value in [
            "",
            "/absolute",
            "../escape",
            "a/../b",
            "a\\b",
            "c:drive",
            "a//b",
        ] {
            assert!(UpstreamPath::parse(value).is_err(), "{value:?}");
        }
    }
}
