use log::warn;

pub(crate) const HTML_PARSER_ENV: &str = "BORROWSER_HTML_PARSER";
pub(crate) const PARSER_MODE_LEGACY: &str = "legacy";
pub(crate) const PARSER_MODE_HTML5: &str = "html5";

/// Runtime parser mode selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ParserMode {
    Legacy,
    Html5,
}

pub(crate) fn parse_runtime_parser_mode(value: Option<&str>) -> Option<ParserMode> {
    let value = value?.trim().to_ascii_lowercase();
    match value.as_str() {
        PARSER_MODE_LEGACY => Some(ParserMode::Legacy),
        PARSER_MODE_HTML5 => Some(ParserMode::Html5),
        _ => None,
    }
}

pub(crate) fn parser_mode_from_env_with<F>(get_env: F) -> ParserMode
where
    F: Fn(&str) -> Option<String>,
{
    match get_env(HTML_PARSER_ENV) {
        Some(value) => match parse_runtime_parser_mode(Some(&value)) {
            Some(mode) => mode,
            None => {
                warn!(
                    target: "runtime_parse",
                    "unsupported parser mode '{value}' (env {HTML_PARSER_ENV}); defaulting to legacy"
                );
                ParserMode::Legacy
            }
        },
        None => ParserMode::Legacy,
    }
}

pub(crate) fn parser_mode_from_env() -> ParserMode {
    parser_mode_from_env_with(|key| std::env::var(key).ok())
}

pub(crate) fn resolve_parser_mode(requested: ParserMode) -> ParserMode {
    #[cfg(feature = "html5")]
    {
        requested
    }
    #[cfg(not(feature = "html5"))]
    {
        match requested {
            ParserMode::Legacy => ParserMode::Legacy,
            ParserMode::Html5 => {
                warn!(
                    target: "runtime_parse",
                    "html5 parser requested (env {HTML_PARSER_ENV}) but feature not enabled; defaulting to legacy"
                );
                ParserMode::Legacy
            }
        }
    }
}
