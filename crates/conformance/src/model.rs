#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CanvasColor(pub(crate) [u8; 3]);

#[derive(Debug)]
pub(crate) struct HarnessError {
    pub(crate) code: &'static str,
    pub(crate) detail: String,
}

impl HarnessError {
    pub(crate) fn new(code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TestId(pub(crate) &'static str);

#[derive(Clone, Copy, Debug)]
pub(crate) struct Fixture {
    pub(crate) id: TestId,
    pub(crate) source: &'static str,
    pub(crate) kind: FixtureKind,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum FixtureKind {
    Runnable {
        html: &'static [u8],
        expected: CanvasColor,
        expectation: Expectation,
    },
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "Registry supports explicit classifications; current fixtures are runnable; tested below."
        )
    )]
    Unsupported { reason: &'static str },
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "Registry supports intentional skips; current fixtures are runnable; tested below."
        )
    )]
    Skipped { reason: &'static str },
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Expectation {
    Match,
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "No registered fixture currently has a known failure; XFAIL/XPASS are regression tested."
        )
    )]
    KnownFailure {
        reason: &'static str,
    },
}

#[derive(Debug)]
pub(crate) enum Execution {
    Observed(CanvasColor),
    Unsupported { reason: &'static str },
    Skipped { reason: &'static str },
    Error(HarnessError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Comparison {
    Match,
    Mismatch,
}

pub(crate) fn compare(actual: CanvasColor, expected: CanvasColor) -> Comparison {
    if actual == expected {
        Comparison::Match
    } else {
        Comparison::Mismatch
    }
}
