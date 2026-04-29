//! Deterministic CSS performance fixtures shared by benches and guards.
//!
//! These fixtures intentionally avoid external files and randomness. They are
//! representative enough to exercise parsing, selector matching, cascade, and
//! computed-style reuse without becoming a benchmark suite for a single web
//! page.

use html::{Node, internal::Id};
use std::{fmt::Write, sync::Arc};

const DECLARATIONS_PER_GENERATED_RULE: usize = 5;

pub fn declarations_per_generated_rule() -> usize {
    DECLARATIONS_PER_GENERATED_RULE
}

pub fn representative_stylesheet(rule_count: usize) -> String {
    let mut css = String::with_capacity(rule_count.saturating_mul(128));

    for index in 0..rule_count {
        let bucket = index % 8;
        let label = index % 16;
        let red = (index * 37) % 256;
        let green = (index * 67) % 256;
        let blue = (index * 97) % 256;
        let background = 255usize.saturating_sub(red);
        let font_size = 12 + (index % 9);
        let margin_left = index % 11;
        let padding_left = index % 7;

        writeln!(
            css,
            ".bucket-{bucket} .label-{label} {{ color: #{red:02x}{green:02x}{blue:02x}; background-color: #{background:02x}{blue:02x}{green:02x}; font-size: {font_size}px; margin-left: {margin_left}px; padding-left: {padding_left}px; }}"
        )
        .expect("writing to String should not fail");
    }

    css
}

pub fn representative_dom(block_count: usize) -> Node {
    let children = (0..block_count).map(representative_section).collect();
    element("div", &[("class", "perf-root")], children)
}

pub fn representative_selector() -> &'static str {
    ".bucket-7 > article.card[data-kind~=\"secondary\"] p.label-7"
}

pub fn representative_selector_rule() -> String {
    format!("{} {{ color: red; }}", representative_selector())
}

pub fn expected_representative_selector_matches(block_count: usize) -> usize {
    (0..block_count).filter(|index| index % 16 == 7).count()
}

pub fn representative_element_count(block_count: usize) -> usize {
    1 + (block_count * 4)
}

fn representative_section(index: usize) -> Node {
    let bucket_class = format!("bucket bucket-{}", index % 8);
    let card_class = format!("card card-{}", index % 16);
    let label_class = format!("label label-{}", index % 16);
    let data_kind = if index.is_multiple_of(2) {
        "primary feature"
    } else {
        "secondary feature"
    };

    element(
        "section",
        &[("class", bucket_class.as_str())],
        vec![element(
            "article",
            &[("class", card_class.as_str()), ("data-kind", data_kind)],
            vec![
                element("p", &[("class", label_class.as_str())], Vec::new()),
                element("span", &[("class", "meta")], Vec::new()),
            ],
        )],
    )
}

fn element(name: &str, attributes: &[(&str, &str)], children: Vec<Node>) -> Node {
    Node::Element {
        id: Id::INVALID,
        name: Arc::from(name),
        attributes: attributes
            .iter()
            .map(|(name, value)| (Arc::from(*name), Some((*value).to_string())))
            .collect(),
        style: Vec::new(),
        children,
    }
}
