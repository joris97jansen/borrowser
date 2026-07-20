//! Immutable WHATWG-derived foreign-content tables.

pub(in crate::html5::tree_builder) const SVG_TAG_NAME_ADJUSTMENTS: [(&str, &str); 37] = [
    ("altglyph", "altGlyph"),
    ("altglyphdef", "altGlyphDef"),
    ("altglyphitem", "altGlyphItem"),
    ("animatecolor", "animateColor"),
    ("animatemotion", "animateMotion"),
    ("animatetransform", "animateTransform"),
    ("clippath", "clipPath"),
    ("feblend", "feBlend"),
    ("fecolormatrix", "feColorMatrix"),
    ("fecomponenttransfer", "feComponentTransfer"),
    ("fecomposite", "feComposite"),
    ("feconvolvematrix", "feConvolveMatrix"),
    ("fediffuselighting", "feDiffuseLighting"),
    ("fedisplacementmap", "feDisplacementMap"),
    ("fedistantlight", "feDistantLight"),
    ("fedropshadow", "feDropShadow"),
    ("feflood", "feFlood"),
    ("fefunca", "feFuncA"),
    ("fefuncb", "feFuncB"),
    ("fefuncg", "feFuncG"),
    ("fefuncr", "feFuncR"),
    ("fegaussianblur", "feGaussianBlur"),
    ("feimage", "feImage"),
    ("femerge", "feMerge"),
    ("femergenode", "feMergeNode"),
    ("femorphology", "feMorphology"),
    ("feoffset", "feOffset"),
    ("fepointlight", "fePointLight"),
    ("fespecularlighting", "feSpecularLighting"),
    ("fespotlight", "feSpotLight"),
    ("fetile", "feTile"),
    ("feturbulence", "feTurbulence"),
    ("foreignobject", "foreignObject"),
    ("glyphref", "glyphRef"),
    ("lineargradient", "linearGradient"),
    ("radialgradient", "radialGradient"),
    ("textpath", "textPath"),
];

pub(in crate::html5::tree_builder) const SVG_ATTRIBUTE_ADJUSTMENTS: [(&str, &str); 58] = [
    ("attributename", "attributeName"),
    ("attributetype", "attributeType"),
    ("basefrequency", "baseFrequency"),
    ("baseprofile", "baseProfile"),
    ("calcmode", "calcMode"),
    ("clippathunits", "clipPathUnits"),
    ("diffuseconstant", "diffuseConstant"),
    ("edgemode", "edgeMode"),
    ("filterunits", "filterUnits"),
    ("glyphref", "glyphRef"),
    ("gradienttransform", "gradientTransform"),
    ("gradientunits", "gradientUnits"),
    ("kernelmatrix", "kernelMatrix"),
    ("kernelunitlength", "kernelUnitLength"),
    ("keypoints", "keyPoints"),
    ("keysplines", "keySplines"),
    ("keytimes", "keyTimes"),
    ("lengthadjust", "lengthAdjust"),
    ("limitingconeangle", "limitingConeAngle"),
    ("markerheight", "markerHeight"),
    ("markerunits", "markerUnits"),
    ("markerwidth", "markerWidth"),
    ("maskcontentunits", "maskContentUnits"),
    ("maskunits", "maskUnits"),
    ("numoctaves", "numOctaves"),
    ("pathlength", "pathLength"),
    ("patterncontentunits", "patternContentUnits"),
    ("patterntransform", "patternTransform"),
    ("patternunits", "patternUnits"),
    ("pointsatx", "pointsAtX"),
    ("pointsaty", "pointsAtY"),
    ("pointsatz", "pointsAtZ"),
    ("preservealpha", "preserveAlpha"),
    ("preserveaspectratio", "preserveAspectRatio"),
    ("primitiveunits", "primitiveUnits"),
    ("refx", "refX"),
    ("refy", "refY"),
    ("repeatcount", "repeatCount"),
    ("repeatdur", "repeatDur"),
    ("requiredextensions", "requiredExtensions"),
    ("requiredfeatures", "requiredFeatures"),
    ("specularconstant", "specularConstant"),
    ("specularexponent", "specularExponent"),
    ("spreadmethod", "spreadMethod"),
    ("startoffset", "startOffset"),
    ("stddeviation", "stdDeviation"),
    ("stitchtiles", "stitchTiles"),
    ("surfacescale", "surfaceScale"),
    ("systemlanguage", "systemLanguage"),
    ("tablevalues", "tableValues"),
    ("targetx", "targetX"),
    ("targety", "targetY"),
    ("textlength", "textLength"),
    ("viewbox", "viewBox"),
    ("viewtarget", "viewTarget"),
    ("xchannelselector", "xChannelSelector"),
    ("ychannelselector", "yChannelSelector"),
    ("zoomandpan", "zoomAndPan"),
];

pub(in crate::html5::tree_builder) const FOREIGN_BREAKOUT_START_TAGS: [&str; 44] = [
    "b",
    "big",
    "blockquote",
    "body",
    "br",
    "center",
    "code",
    "dd",
    "div",
    "dl",
    "dt",
    "em",
    "embed",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "head",
    "hr",
    "i",
    "img",
    "li",
    "listing",
    "menu",
    "meta",
    "nobr",
    "ol",
    "p",
    "pre",
    "ruby",
    "s",
    "small",
    "span",
    "strike",
    "strong",
    "sub",
    "sup",
    "table",
    "tt",
    "u",
    "ul",
    "var",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::html5::tree_builder) enum QualifiedForeignAttributeAdjustment {
    Xml(&'static str),
    XLink(&'static str),
    XmlnsDefault,
    XmlnsPrefixed(&'static str),
}

pub(in crate::html5::tree_builder) const QUALIFIED_FOREIGN_ATTRIBUTE_ADJUSTMENTS: [(
    &str,
    QualifiedForeignAttributeAdjustment,
); 12] = [
    (
        "xlink:actuate",
        QualifiedForeignAttributeAdjustment::XLink("actuate"),
    ),
    (
        "xlink:arcrole",
        QualifiedForeignAttributeAdjustment::XLink("arcrole"),
    ),
    (
        "xlink:href",
        QualifiedForeignAttributeAdjustment::XLink("href"),
    ),
    (
        "xlink:role",
        QualifiedForeignAttributeAdjustment::XLink("role"),
    ),
    (
        "xlink:show",
        QualifiedForeignAttributeAdjustment::XLink("show"),
    ),
    (
        "xlink:title",
        QualifiedForeignAttributeAdjustment::XLink("title"),
    ),
    (
        "xlink:type",
        QualifiedForeignAttributeAdjustment::XLink("type"),
    ),
    ("xml:base", QualifiedForeignAttributeAdjustment::Xml("base")),
    ("xml:lang", QualifiedForeignAttributeAdjustment::Xml("lang")),
    (
        "xml:space",
        QualifiedForeignAttributeAdjustment::Xml("space"),
    ),
    ("xmlns", QualifiedForeignAttributeAdjustment::XmlnsDefault),
    (
        "xmlns:xlink",
        QualifiedForeignAttributeAdjustment::XmlnsPrefixed("xlink"),
    ),
];

pub(in crate::html5::tree_builder) fn svg_adjusted_tag_name(name: &str) -> &str {
    SVG_TAG_NAME_ADJUSTMENTS
        .binary_search_by_key(&name, |(source, _)| *source)
        .map_or(name, |index| SVG_TAG_NAME_ADJUSTMENTS[index].1)
}

pub(in crate::html5::tree_builder) fn svg_adjusted_attribute_name(name: &str) -> &str {
    SVG_ATTRIBUTE_ADJUSTMENTS
        .binary_search_by_key(&name, |(source, _)| *source)
        .map_or(name, |index| SVG_ATTRIBUTE_ADJUSTMENTS[index].1)
}

pub(in crate::html5::tree_builder) fn is_foreign_breakout_start(name: &str) -> bool {
    FOREIGN_BREAKOUT_START_TAGS.binary_search(&name).is_ok()
}

pub(in crate::html5::tree_builder) fn qualified_foreign_attribute_adjustment(
    name: &str,
) -> Option<QualifiedForeignAttributeAdjustment> {
    QUALIFIED_FOREIGN_ATTRIBUTE_ADJUSTMENTS
        .binary_search_by_key(&name, |(source, _)| *source)
        .ok()
        .map(|index| QUALIFIED_FOREIGN_ATTRIBUTE_ADJUSTMENTS[index].1)
}
