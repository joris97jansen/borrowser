#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CssFuzzRegressionTool {
    Tokenizer,
    Parser,
    SelectorParser,
    SelectorMatching,
    Cascade,
    Values,
}

impl CssFuzzRegressionTool {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Tokenizer => "css_tokenizer",
            Self::Parser => "css_parser",
            Self::SelectorParser => "css_selector_parser",
            Self::SelectorMatching => "css_selector_matching",
            Self::Cascade => "css_cascade",
            Self::Values => "css_values",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "css_tokenizer" => Some(Self::Tokenizer),
            "css_parser" => Some(Self::Parser),
            "css_selector_parser" => Some(Self::SelectorParser),
            "css_selector_matching" => Some(Self::SelectorMatching),
            "css_cascade" => Some(Self::Cascade),
            "css_values" => Some(Self::Values),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CssFuzzRegressionProfile {
    Default,
    SelectorLimitZero,
}

impl CssFuzzRegressionProfile {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::SelectorLimitZero => "selector-limit-zero",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "default" => Some(Self::Default),
            "selector-limit-zero" => Some(Self::SelectorLimitZero),
            _ => None,
        }
    }
}
