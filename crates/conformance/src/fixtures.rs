use crate::model::{CanvasColor, Expectation, Fixture, FixtureKind, TestId};

// Order is intentional and is also the report order. Expectations are authored
// from CSS canvas propagation and cascade specificity, never blessed output.
pub(crate) const FIXTURES: &[Fixture] = &[
    Fixture {
        id: TestId("canvas/root"),
        source: "fixtures/root-canvas-color.html",
        kind: FixtureKind::Runnable {
            html: include_bytes!("../fixtures/root-canvas-color.html"),
            expected: CanvasColor([0x12, 0x34, 0x56]),
            expectation: Expectation::Match,
        },
    },
    Fixture {
        id: TestId("canvas/cascade"),
        source: "fixtures/cascade-canvas-color.html",
        kind: FixtureKind::Runnable {
            html: include_bytes!("../fixtures/cascade-canvas-color.html"),
            expected: CanvasColor([0x34, 0x56, 0x78]),
            expectation: Expectation::Match,
        },
    },
];
