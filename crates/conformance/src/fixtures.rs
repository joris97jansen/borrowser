use crate::model::{CanvasColor, Expectation, Fixture, FixtureKind, TestId};
use crate::projection::{ObservationRequest, Request, StyleField, StyleProperty};

// Order is intentional and is also the report order. Expectations are authored
// from CSS canvas propagation and cascade specificity, never blessed output.
pub(crate) const FIXTURES: &[Fixture] = &[
    Fixture {
        request: crate::projection::ObservationRequest::CANVAS,
        id: TestId("canvas/root"),
        source: "fixtures/root-canvas-color.html",
        kind: FixtureKind::Runnable {
            html: include_bytes!("../fixtures/root-canvas-color.html"),
            expected: crate::model::Expected::Canvas(CanvasColor([0x12, 0x34, 0x56])),
            expectation: Expectation::Match,
        },
    },
    Fixture {
        request: crate::projection::ObservationRequest::CANVAS,
        id: TestId("canvas/cascade"),
        source: "fixtures/cascade-canvas-color.html",
        kind: FixtureKind::Runnable {
            html: include_bytes!("../fixtures/cascade-canvas-color.html"),
            expected: crate::model::Expected::Canvas(CanvasColor([0x34, 0x56, 0x78])),
            expectation: Expectation::Match,
        },
    },
    Fixture {
        id: TestId("dom/structure"),
        source: "fixtures/dom-structure.html",
        request: ObservationRequest {
            dom: Request::Compare(()),
            ..ObservationRequest::NONE
        },
        kind: FixtureKind::Runnable {
            html: include_bytes!("../fixtures/dom-structure.html"),
            expected: crate::model::Expected::Projection(include_str!(
                "../fixtures/dom-structure.expected.json"
            )),
            expectation: Expectation::Match,
        },
    },
    Fixture {
        id: TestId("dom/template-namespaces"),
        source: "fixtures/dom-template-namespaces.html",
        request: ObservationRequest {
            dom: Request::Compare(()),
            ..ObservationRequest::NONE
        },
        kind: FixtureKind::Runnable {
            html: include_bytes!("../fixtures/dom-template-namespaces.html"),
            expected: crate::model::Expected::Projection(include_str!(
                "../fixtures/dom-template-namespaces.expected.json"
            )),
            expectation: Expectation::Match,
        },
    },
    Fixture {
        id: TestId("style/cascade-inheritance"),
        source: "fixtures/style-cascade-inheritance.html",
        request: ObservationRequest {
            styles: &STYLE_FIELDS,
            ..ObservationRequest::NONE
        },
        kind: FixtureKind::Runnable {
            html: include_bytes!("../fixtures/style-cascade-inheritance.html"),
            expected: crate::model::Expected::Projection(include_str!(
                "../fixtures/style-cascade-inheritance.expected.json"
            )),
            expectation: Expectation::Match,
        },
    },
    Fixture {
        id: TestId("layout/block-boxes"),
        source: "fixtures/layout-block-boxes.html",
        request: ObservationRequest {
            layout: Request::Compare(&[
                "container",
                "child",
                "sibling",
                "hidden",
                "zero",
                "missing",
            ]),
            ..ObservationRequest::NONE
        },
        kind: FixtureKind::Runnable {
            html: include_bytes!("../fixtures/layout-block-boxes.html"),
            expected: crate::model::Expected::Projection(include_str!(
                "../fixtures/layout-block-boxes.expected.json"
            )),
            expectation: Expectation::Match,
        },
    },
];

const STYLE_FIELDS: [StyleField; 12] = [
    StyleField {
        target: "parent",
        property: Request::Compare(StyleProperty::Color),
        name: StyleProperty::Color.name(),
    },
    StyleField {
        target: "parent",
        property: Request::Compare(StyleProperty::BackgroundColor),
        name: StyleProperty::BackgroundColor.name(),
    },
    StyleField {
        target: "parent",
        property: Request::Compare(StyleProperty::FontSize),
        name: StyleProperty::FontSize.name(),
    },
    StyleField {
        target: "parent",
        property: Request::Compare(StyleProperty::Display),
        name: StyleProperty::Display.name(),
    },
    StyleField {
        target: "child",
        property: Request::Compare(StyleProperty::Color),
        name: StyleProperty::Color.name(),
    },
    StyleField {
        target: "child",
        property: Request::Compare(StyleProperty::BackgroundColor),
        name: StyleProperty::BackgroundColor.name(),
    },
    StyleField {
        target: "child",
        property: Request::Compare(StyleProperty::FontSize),
        name: StyleProperty::FontSize.name(),
    },
    StyleField {
        target: "child",
        property: Request::Compare(StyleProperty::Display),
        name: StyleProperty::Display.name(),
    },
    StyleField {
        target: "hidden",
        property: Request::Compare(StyleProperty::Color),
        name: StyleProperty::Color.name(),
    },
    StyleField {
        target: "hidden",
        property: Request::Compare(StyleProperty::BackgroundColor),
        name: StyleProperty::BackgroundColor.name(),
    },
    StyleField {
        target: "hidden",
        property: Request::Compare(StyleProperty::FontSize),
        name: StyleProperty::FontSize.name(),
    },
    StyleField {
        target: "hidden",
        property: Request::Compare(StyleProperty::Display),
        name: StyleProperty::Display.name(),
    },
];
