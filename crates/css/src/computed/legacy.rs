use crate::{
    PropertyId, PropertyInheritance, model, property_registry,
    specified::parse_specified_value,
    syntax::ParseOptions,
    values::{Display, Length},
};

use html::Node;

use super::{
    builder::ComputedStyleBuilder,
    style::ComputedStyle,
    style_tree::StyledNode,
    value::{ComputedValue, normalize_specified_value},
};

/// Compute the final, inherited style for an element, given:
/// - its specified declarations (currently the legacy `Node.style` bridge)
/// - an optional parent computed style.
///
/// Assumptions:
/// - `specified` already reflects cascade (author + inline etc.)
///
/// This remains a compatibility step for callers that still provide the
/// DOM-attached string vector. Supported declarations are still resolved through
/// the property registry, specified-value parser, computed-value normalizer, and
/// `ComputedStyle` invariant gate. New document-level style work should use
/// `compute_document_styles(...)` or `build_style_tree_with_stylesheets(...)`.
/// Bridge-phase HTML/UA default display behavior is still applied later in
/// `build_style_tree()` when no authored `display` declaration exists.
pub fn compute_style(
    tag_name: Option<&str>,
    specified: &[(String, String)],
    parent: Option<&ComputedStyle>,
) -> ComputedStyle {
    let mut result = legacy_base_computed_style(parent);
    let mut has_valid_color_decl = false;

    for (name, value) in specified {
        let Some((property, value)) = legacy_computed_declaration(name, value) else {
            continue;
        };

        if property == PropertyId::Color {
            has_valid_color_decl = true;
        }
        result = replace_computed_property(result, property, value);
    }

    apply_legacy_ua_defaults(tag_name, result, has_valid_color_decl)
}

fn legacy_base_computed_style(parent: Option<&ComputedStyle>) -> ComputedStyle {
    let mut builder = ComputedStyleBuilder::new();

    for property in property_registry().ids() {
        let value = match (property.metadata().inheritance, parent) {
            (PropertyInheritance::Inherited, Some(parent)) => parent.get(property).value(),
            _ => ComputedValue::from_initial(property),
        };
        if builder.record(property, value).is_err() {
            #[cfg(debug_assertions)]
            eprintln!(
                "legacy base computed-style assembly degraded while recording '{}'",
                property.name()
            );
            return parent
                .copied()
                .unwrap_or_else(ComputedStyle::fallback_initial);
        }
    }

    builder.build().unwrap_or_else(|error| {
        #[cfg(debug_assertions)]
        eprintln!("legacy base computed-style assembly degraded: {error}");
        parent
            .copied()
            .unwrap_or_else(ComputedStyle::fallback_initial)
    })
}

fn legacy_computed_declaration(name: &str, value: &str) -> Option<(PropertyId, ComputedValue)> {
    let name = name.trim().to_ascii_lowercase();
    let property = property_registry().lookup_id(&name)?;
    let declaration_value = legacy_declaration_value(property, value)?;
    let specified = parse_specified_value(property, &declaration_value).ok()?;
    let computed = normalize_specified_value(&specified).ok()?;
    Some((property, computed))
}

fn legacy_declaration_value(property: PropertyId, value: &str) -> Option<model::DeclarationValue> {
    // Bridge-only parser adapter: this routes DOM-attached `(property, value)`
    // pairs through the real model parser so the legacy path does not grow a
    // second CSS value parser. It is intentionally heavier than the final
    // resolved-style pipeline and should disappear with the compatibility
    // bridge, not become the long-term declaration-value entrypoint.
    let source = format!("div {{ {}: {}; }}", property.name(), value);
    let parse = model::parse_stylesheet_with_options(&source, &ParseOptions::stylesheet());
    let model::Rule::Style(rule) = parse.stylesheet.rules.into_iter().next()? else {
        return None;
    };
    let declaration = rule.declarations.declarations.into_iter().next()?;
    if declaration.name.text.as_deref()? != property.name() {
        return None;
    }
    Some(declaration.value)
}

fn replace_computed_property(
    style: ComputedStyle,
    property: PropertyId,
    value: ComputedValue,
) -> ComputedStyle {
    style
        .with_property(property, value)
        .unwrap_or_else(|error| {
            #[cfg(debug_assertions)]
            eprintln!(
                "legacy computed-style replacement degraded for '{}': {error}",
                property.name()
            );
            style
        })
}

fn apply_legacy_ua_defaults(
    tag_name: Option<&str>,
    style: ComputedStyle,
    has_valid_color_decl: bool,
) -> ComputedStyle {
    let Some(tag) = tag_name else {
        return style;
    };

    let mut style = style;
    if tag.eq_ignore_ascii_case("a") && !has_valid_color_decl {
        style = replace_computed_property(
            style,
            PropertyId::Color,
            ComputedValue::Color((0, 0, 238, 255)),
        );
    }

    if tag.eq_ignore_ascii_case("button") {
        if style.background_color().3 == 0 {
            style = replace_computed_property(
                style,
                PropertyId::BackgroundColor,
                ComputedValue::Color((233, 233, 233, 255)),
            );
        }

        let box_metrics = style.box_metrics();
        for (property, px) in [
            (PropertyId::PaddingLeft, box_metrics.padding_left.max(8.0)),
            (PropertyId::PaddingRight, box_metrics.padding_right.max(8.0)),
            (PropertyId::PaddingTop, box_metrics.padding_top.max(4.0)),
            (
                PropertyId::PaddingBottom,
                box_metrics.padding_bottom.max(4.0),
            ),
        ] {
            style =
                replace_computed_property(style, property, ComputedValue::Length(Length::Px(px)));
        }
    }

    style
}

fn default_display_for(tag: &str) -> Display {
    // Bridge-phase HTML/UA-ish element defaults applied only when no authored
    // `display` declaration exists. This is not the CSS initial value of
    // `display`; the cascade contract keeps that at `inline`.
    if tag.eq_ignore_ascii_case("span")
        || tag.eq_ignore_ascii_case("a")
        || tag.eq_ignore_ascii_case("em")
        || tag.eq_ignore_ascii_case("strong")
        || tag.eq_ignore_ascii_case("b")
        || tag.eq_ignore_ascii_case("i")
        || tag.eq_ignore_ascii_case("u")
        || tag.eq_ignore_ascii_case("small")
        || tag.eq_ignore_ascii_case("big")
        || tag.eq_ignore_ascii_case("code")
        || tag.eq_ignore_ascii_case("img")
        || tag.eq_ignore_ascii_case("input")
    {
        return Display::Inline;
    }

    // List items are special: they default to list-item
    if tag.eq_ignore_ascii_case("li") {
        return Display::ListItem;
    }

    if tag.eq_ignore_ascii_case("button") {
        return Display::InlineBlock;
    }
    if tag.eq_ignore_ascii_case("textarea") {
        return Display::InlineBlock;
    }
    // Everything else we treat as block for now
    Display::Block
}

/// Build a style tree from a DOM root.
/// - `root` is the DOM node (usually the document root)
/// - `parent_style` is the inherited style, if any
///
/// We:
/// - Create a `StyledNode` for Document + Element nodes
/// - Skip Text/Comment nodes for now (can be added later for inline layout)
pub fn build_style_tree<'a>(
    root: &'a html::Node,
    parent_style: Option<&ComputedStyle>,
) -> StyledNode<'a> {
    match root {
        Node::Document { children, .. } => {
            let base = parent_style.copied().unwrap_or_else(ComputedStyle::initial);

            let mut styled_children = Vec::new();
            for child in children {
                // Include *all* node types so we see Text nodes here too
                styled_children.push(build_style_tree(child, Some(&base)));
            }

            StyledNode {
                node: root,
                node_id: root.id(),
                style: base,
                children: styled_children,
            }
        }

        Node::Element {
            name,
            style,
            children,
            ..
        } => {
            // 1) Check if there is a valid explicit `display:` declaration.
            // Invalid declarations are ignored, so they must not suppress the
            // temporary HTML/UA default-display bridge.
            let has_display_decl = style.iter().any(|(name, value)| {
                matches!(
                    legacy_computed_declaration(name, value),
                    Some((PropertyId::Display, _))
                )
            });

            // 2) Compute the base style (inherits, applies declarations, etc.)
            let mut computed = compute_style(Some(name), style, parent_style);

            // 3) If no explicit `display:` was specified, apply the temporary
            //    HTML/UA default-display bridge for this element type.
            if !has_display_decl {
                computed = replace_computed_property(
                    computed,
                    PropertyId::Display,
                    ComputedValue::Display(default_display_for(name)),
                );
            }

            // 4) Recurse into children with this as the parent computed style
            let mut styled_children = Vec::new();
            for child in children {
                styled_children.push(build_style_tree(child, Some(&computed)));
            }

            StyledNode {
                node: root,
                node_id: root.id(),
                style: computed,
                children: styled_children,
            }
        }

        Node::Text { .. } | Node::Comment { .. } => {
            // Inherit everything from parent
            let inherited = parent_style.copied().unwrap_or_else(ComputedStyle::initial);

            StyledNode {
                node: root,
                node_id: root.id(),
                style: inherited,
                children: Vec::new(),
            }
        }
    }
}
