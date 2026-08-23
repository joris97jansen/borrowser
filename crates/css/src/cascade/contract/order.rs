const SOURCE_KIND_BITS: u32 = 2;
const SOURCE_PAYLOAD_MAX: u64 = u64::MAX >> SOURCE_KIND_BITS;
const SOURCE_KIND_UA: u64 = 0;
const SOURCE_KIND_BROWSER: u64 = 1;
const SOURCE_KIND_COMPATIBILITY: u64 = 2;
const SOURCE_KIND_IN_MEMORY: u64 = 3;

/// Opaque identity for one stylesheet source in cascade provenance.
///
/// Identity is independent from URL, parse storage, content, cascade origin,
/// and source order. The private encoding keeps current source domains
/// collision-free without making those domains part of cascade semantics.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StylesheetSourceId(u64);

impl StylesheetSourceId {
    pub const fn built_in_user_agent() -> Self {
        Self(SOURCE_KIND_UA)
    }

    pub fn from_browser_slot(slot: u64) -> Result<Self, StylesheetSourceIdError> {
        if slot > SOURCE_PAYLOAD_MAX {
            return Err(StylesheetSourceIdError::BrowserSlotPayloadOutOfRange {
                payload: slot,
                maximum: SOURCE_PAYLOAD_MAX,
            });
        }
        Ok(Self((slot << SOURCE_KIND_BITS) | SOURCE_KIND_BROWSER))
    }

    pub fn compatibility_generation_index(index: u32) -> Self {
        Self((u64::from(index) << SOURCE_KIND_BITS) | SOURCE_KIND_COMPATIBILITY)
    }

    pub fn in_memory_generation_index(index: u32) -> Self {
        Self((u64::from(index) << SOURCE_KIND_BITS) | SOURCE_KIND_IN_MEMORY)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StylesheetSourceIdError {
    BrowserSlotPayloadOutOfRange { payload: u64, maximum: u64 },
}

impl StylesheetSourceIdError {
    pub const fn stable_label(self) -> &'static str {
        match self {
            Self::BrowserSlotPayloadOutOfRange { .. } => "browser-slot-payload-out-of-range",
        }
    }
}

impl std::fmt::Display for StylesheetSourceIdError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BrowserSlotPayloadOutOfRange { payload, maximum } => write!(
                formatter,
                "stylesheet source id browser slot payload {payload} exceeds {maximum}"
            ),
        }
    }
}

impl std::error::Error for StylesheetSourceIdError {}

macro_rules! checked_u32_coordinate {
    ($name:ident, $label:literal) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(u32);

        impl $name {
            pub const fn new(value: u32) -> Self {
                Self(value)
            }

            pub fn from_usize(value: usize) -> Result<Self, SourceCoordinateError> {
                u32::try_from(value)
                    .map(Self)
                    .map_err(|_| SourceCoordinateError::Unrepresentable {
                        coordinate: $label,
                        value,
                        maximum: u32::MAX as usize,
                    })
            }

            pub const fn get(self) -> u32 {
                self.0
            }
        }

        impl From<u32> for $name {
            fn from(value: u32) -> Self {
                Self::new(value)
            }
        }

        impl PartialEq<u32> for $name {
            fn eq(&self, other: &u32) -> bool {
                self.0 == *other
            }
        }
    };
}

checked_u32_coordinate!(StylesheetOrder, "stylesheet-order");
checked_u32_coordinate!(RawRuleIndex, "raw-rule-index");
checked_u32_coordinate!(StyleRulePosition, "style-rule-position");
checked_u32_coordinate!(DeclarationSourceIndex, "declaration-source-index");
checked_u32_coordinate!(DeclarationOrder, "declaration-order");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceCoordinateError {
    Unrepresentable {
        coordinate: &'static str,
        value: usize,
        maximum: usize,
    },
    CounterExhausted {
        coordinate: &'static str,
    },
}

impl SourceCoordinateError {
    pub const fn stable_label(self) -> &'static str {
        match self {
            Self::Unrepresentable { .. } => "unrepresentable",
            Self::CounterExhausted { .. } => "counter-exhausted",
        }
    }
}

impl std::fmt::Display for SourceCoordinateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unrepresentable {
                coordinate,
                value,
                maximum,
            } => write!(
                formatter,
                "{coordinate} value {value} exceeds representable maximum {maximum}"
            ),
            Self::CounterExhausted { coordinate } => {
                write!(formatter, "{coordinate} counter exhausted")
            }
        }
    }
}

impl std::error::Error for SourceCoordinateError {}

/// Current stylesheet-rule source-order key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StylesheetRuleOrder {
    stylesheet: StylesheetOrder,
    rule: StyleRulePosition,
}

impl StylesheetRuleOrder {
    pub const fn new(stylesheet: StylesheetOrder, rule: StyleRulePosition) -> Self {
        Self { stylesheet, rule }
    }

    pub const fn stylesheet(self) -> StylesheetOrder {
        self.stylesheet
    }

    pub const fn rule(self) -> StyleRulePosition {
        self.rule
    }

    pub fn semantic_cmp(self, other: Self) -> std::cmp::Ordering {
        self.stylesheet
            .cmp(&other.stylesheet)
            .then_with(|| self.rule.cmp(&other.rule))
    }
}

impl Ord for StylesheetRuleOrder {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.semantic_cmp(*other)
    }
}

impl PartialOrd for StylesheetRuleOrder {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
impl From<u32> for StylesheetRuleOrder {
    fn from(style_rule_position: u32) -> Self {
        Self::new(
            StylesheetOrder::new(0),
            StyleRulePosition::new(style_rule_position),
        )
    }
}
