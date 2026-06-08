//! Layout-owned flex algorithms for the Milestone Z core subset.
//!
//! This module is deliberately independent of CSS parsing. CSS owns authored
//! values; layout consumes typed factors, sizes, alignment defaults, and
//! generated flex item participation metadata.

use std::fmt::Write;

use crate::{CssPx, SignedCssPx};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlexMainAxis {
    Row,
}

impl FlexMainAxis {
    pub fn as_debug_label(self) -> &'static str {
        match self {
            Self::Row => "row",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlexCrossAxis {
    Block,
}

impl FlexCrossAxis {
    pub fn as_debug_label(self) -> &'static str {
        match self {
            Self::Block => "block",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlexCrossAxisAlignment {
    Start,
    Center,
    End,
    Stretch,
}

impl FlexCrossAxisAlignment {
    pub fn as_debug_label(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Center => "center",
            Self::End => "end",
            Self::Stretch => "stretch",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlexFreeSpaceDistribution {
    NoItems,
    ExactFit,
    PositiveNoGrow,
    PositiveGrow,
    NegativeShrink,
    NegativeNoShrink,
}

impl FlexFreeSpaceDistribution {
    pub fn as_debug_label(self) -> &'static str {
        match self {
            Self::NoItems => "no-items",
            Self::ExactFit => "exact-fit",
            Self::PositiveNoGrow => "positive-no-grow",
            Self::PositiveGrow => "positive-grow",
            Self::NegativeShrink => "negative-shrink",
            Self::NegativeNoShrink => "negative-no-shrink",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlexContainerMainAxisLayout {
    axis: FlexMainAxis,
    available_main_size: CssPx,
    total_outer_base_size: SignedCssPx,
    free_space: SignedCssPx,
    distribution: FlexFreeSpaceDistribution,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlexContainerCrossAxisLayout {
    axis: FlexCrossAxis,
    available_cross_size: Option<CssPx>,
    line_cross_size: CssPx,
    auto_cross_size: CssPx,
    used_cross_size: CssPx,
}

impl FlexContainerCrossAxisLayout {
    fn new(
        axis: FlexCrossAxis,
        available_cross_size: Option<CssPx>,
        line_cross_size: CssPx,
        auto_cross_size: CssPx,
        used_cross_size: CssPx,
    ) -> Self {
        Self {
            axis,
            available_cross_size,
            line_cross_size,
            auto_cross_size,
            used_cross_size,
        }
    }

    pub fn axis(self) -> FlexCrossAxis {
        self.axis
    }

    pub fn available_cross_size(self) -> Option<CssPx> {
        self.available_cross_size
    }

    pub fn line_cross_size(self) -> CssPx {
        self.line_cross_size
    }

    pub fn auto_cross_size(self) -> CssPx {
        self.auto_cross_size
    }

    pub fn used_cross_size(self) -> CssPx {
        self.used_cross_size
    }

    pub fn as_debug_label(self) -> String {
        format!(
            "axis={} available={} line={} auto={} used={}",
            self.axis.as_debug_label(),
            optional_css_px_debug_label(self.available_cross_size),
            css_px_debug_label(self.line_cross_size),
            css_px_debug_label(self.auto_cross_size),
            css_px_debug_label(self.used_cross_size),
        )
    }
}

impl FlexContainerMainAxisLayout {
    fn new(
        axis: FlexMainAxis,
        available_main_size: CssPx,
        total_outer_base_size: SignedCssPx,
        free_space: SignedCssPx,
        distribution: FlexFreeSpaceDistribution,
    ) -> Self {
        Self {
            axis,
            available_main_size,
            total_outer_base_size,
            free_space,
            distribution,
        }
    }

    pub fn axis(self) -> FlexMainAxis {
        self.axis
    }

    pub fn available_main_size(self) -> CssPx {
        self.available_main_size
    }

    pub fn total_outer_base_size(self) -> SignedCssPx {
        self.total_outer_base_size
    }

    pub fn free_space(self) -> SignedCssPx {
        self.free_space
    }

    pub fn distribution(self) -> FlexFreeSpaceDistribution {
        self.distribution
    }

    pub fn as_debug_label(self) -> String {
        format!(
            "axis={} available={} outer-base={} free-space={} distribution={}",
            self.axis.as_debug_label(),
            css_px_debug_label(self.available_main_size),
            signed_css_px_debug_label(self.total_outer_base_size),
            signed_css_px_debug_label(self.free_space),
            self.distribution.as_debug_label(),
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlexItemMainAxisInput {
    base_main_size: CssPx,
    margin_start: SignedCssPx,
    margin_end: SignedCssPx,
    flex_grow: f32,
    flex_shrink: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlexItemCrossAxisInput {
    hypothetical_cross_size: CssPx,
    margin_before: SignedCssPx,
    margin_after: SignedCssPx,
    alignment: FlexCrossAxisAlignment,
    can_stretch: bool,
}

impl FlexItemCrossAxisInput {
    pub fn new(
        hypothetical_cross_size: CssPx,
        margin_before: SignedCssPx,
        margin_after: SignedCssPx,
        alignment: FlexCrossAxisAlignment,
        can_stretch: bool,
    ) -> Self {
        Self {
            hypothetical_cross_size,
            margin_before,
            margin_after,
            alignment,
            can_stretch,
        }
    }

    pub fn default_row_stretch(
        hypothetical_cross_size: CssPx,
        margin_before: SignedCssPx,
        margin_after: SignedCssPx,
        can_stretch: bool,
    ) -> Self {
        Self::new(
            hypothetical_cross_size,
            margin_before,
            margin_after,
            FlexCrossAxisAlignment::Stretch,
            can_stretch,
        )
    }

    pub fn hypothetical_cross_size(self) -> CssPx {
        self.hypothetical_cross_size
    }

    pub fn margin_before(self) -> SignedCssPx {
        self.margin_before
    }

    pub fn margin_after(self) -> SignedCssPx {
        self.margin_after
    }

    pub fn alignment(self) -> FlexCrossAxisAlignment {
        self.alignment
    }

    pub fn can_stretch(self) -> bool {
        self.can_stretch
    }

    fn outer_hypothetical_size(self) -> f32 {
        self.margin_before.get() + self.hypothetical_cross_size.get() + self.margin_after.get()
    }
}

impl FlexItemMainAxisInput {
    pub fn new(
        base_main_size: CssPx,
        margin_start: SignedCssPx,
        margin_end: SignedCssPx,
        flex_grow: f32,
        flex_shrink: f32,
    ) -> Option<Self> {
        if !is_valid_factor(flex_grow) || !is_valid_factor(flex_shrink) {
            return None;
        }

        Some(Self {
            base_main_size,
            margin_start,
            margin_end,
            flex_grow,
            flex_shrink,
        })
    }

    pub fn default_row_auto_basis(
        base_main_size: CssPx,
        margin_start: SignedCssPx,
        margin_end: SignedCssPx,
    ) -> Self {
        Self::new(base_main_size, margin_start, margin_end, 0.0, 1.0)
            .expect("default flex factors are valid")
    }

    pub fn base_main_size(self) -> CssPx {
        self.base_main_size
    }

    pub fn margin_start(self) -> SignedCssPx {
        self.margin_start
    }

    pub fn margin_end(self) -> SignedCssPx {
        self.margin_end
    }

    pub fn flex_grow(self) -> f32 {
        self.flex_grow
    }

    pub fn flex_shrink(self) -> f32 {
        self.flex_shrink
    }

    fn outer_base_size(self) -> f32 {
        self.margin_start.get() + self.base_main_size.get() + self.margin_end.get()
    }

    fn scaled_shrink_factor(self) -> f32 {
        self.flex_shrink * self.base_main_size.get()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlexItemMainAxisLayout {
    base_main_size: CssPx,
    target_main_size: CssPx,
    main_offset: SignedCssPx,
    margin_start: SignedCssPx,
    margin_end: SignedCssPx,
    flex_grow: f32,
    flex_shrink: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlexItemCrossAxisLayout {
    hypothetical_cross_size: CssPx,
    target_cross_size: CssPx,
    cross_offset: SignedCssPx,
    margin_before: SignedCssPx,
    margin_after: SignedCssPx,
    alignment: FlexCrossAxisAlignment,
    can_stretch: bool,
}

impl FlexItemCrossAxisLayout {
    pub fn new(
        input: FlexItemCrossAxisInput,
        target_cross_size: CssPx,
        cross_offset: SignedCssPx,
    ) -> Self {
        Self {
            hypothetical_cross_size: input.hypothetical_cross_size,
            target_cross_size,
            cross_offset,
            margin_before: input.margin_before,
            margin_after: input.margin_after,
            alignment: input.alignment,
            can_stretch: input.can_stretch,
        }
    }

    pub fn hypothetical_cross_size(self) -> CssPx {
        self.hypothetical_cross_size
    }

    pub fn target_cross_size(self) -> CssPx {
        self.target_cross_size
    }

    pub fn cross_offset(self) -> SignedCssPx {
        self.cross_offset
    }

    pub fn margin_before(self) -> SignedCssPx {
        self.margin_before
    }

    pub fn margin_after(self) -> SignedCssPx {
        self.margin_after
    }

    pub fn alignment(self) -> FlexCrossAxisAlignment {
        self.alignment
    }

    pub fn can_stretch(self) -> bool {
        self.can_stretch
    }

    pub fn stretches(self) -> bool {
        self.alignment == FlexCrossAxisAlignment::Stretch && self.can_stretch
    }

    pub fn as_debug_label(self) -> String {
        format!(
            "hypothetical={} target={} offset={} margin-before={} margin-after={} alignment={} can-stretch={}",
            css_px_debug_label(self.hypothetical_cross_size),
            css_px_debug_label(self.target_cross_size),
            signed_css_px_debug_label(self.cross_offset),
            signed_css_px_debug_label(self.margin_before),
            signed_css_px_debug_label(self.margin_after),
            self.alignment.as_debug_label(),
            bool_debug_label(self.can_stretch),
        )
    }
}

impl FlexItemMainAxisLayout {
    pub fn new(
        input: FlexItemMainAxisInput,
        target_main_size: CssPx,
        main_offset: SignedCssPx,
    ) -> Self {
        Self {
            base_main_size: input.base_main_size,
            target_main_size,
            main_offset,
            margin_start: input.margin_start,
            margin_end: input.margin_end,
            flex_grow: input.flex_grow,
            flex_shrink: input.flex_shrink,
        }
    }

    pub fn base_main_size(self) -> CssPx {
        self.base_main_size
    }

    pub fn target_main_size(self) -> CssPx {
        self.target_main_size
    }

    pub fn main_offset(self) -> SignedCssPx {
        self.main_offset
    }

    pub fn margin_start(self) -> SignedCssPx {
        self.margin_start
    }

    pub fn margin_end(self) -> SignedCssPx {
        self.margin_end
    }

    pub fn flex_grow(self) -> f32 {
        self.flex_grow
    }

    pub fn flex_shrink(self) -> f32 {
        self.flex_shrink
    }

    pub fn as_debug_label(self) -> String {
        format!(
            "base={} target={} offset={} margin-start={} margin-end={} grow={:.2} shrink={:.2}",
            css_px_debug_label(self.base_main_size),
            css_px_debug_label(self.target_main_size),
            signed_css_px_debug_label(self.main_offset),
            signed_css_px_debug_label(self.margin_start),
            signed_css_px_debug_label(self.margin_end),
            self.flex_grow,
            self.flex_shrink,
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FlexMainAxisLayout {
    container: FlexContainerMainAxisLayout,
    items: Vec<FlexItemMainAxisLayout>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FlexCrossAxisLayout {
    container: FlexContainerCrossAxisLayout,
    items: Vec<FlexItemCrossAxisLayout>,
}

impl FlexCrossAxisLayout {
    fn new(container: FlexContainerCrossAxisLayout, items: Vec<FlexItemCrossAxisLayout>) -> Self {
        Self { container, items }
    }

    pub fn container(&self) -> FlexContainerCrossAxisLayout {
        self.container
    }

    pub fn items(&self) -> &[FlexItemCrossAxisLayout] {
        &self.items
    }

    pub fn into_items(self) -> Vec<FlexItemCrossAxisLayout> {
        self.items
    }

    pub fn to_debug_snapshot(&self) -> String {
        let mut out = String::new();
        writeln!(&mut out, "version: 1").expect("write snapshot");
        writeln!(&mut out, "flex-cross-axis").expect("write snapshot");
        writeln!(&mut out, "container: {}", self.container.as_debug_label())
            .expect("write snapshot");
        for (index, item) in self.items.iter().enumerate() {
            writeln!(&mut out, "item[{index}]: {}", item.as_debug_label()).expect("write snapshot");
        }
        out
    }
}

impl FlexMainAxisLayout {
    fn new(container: FlexContainerMainAxisLayout, items: Vec<FlexItemMainAxisLayout>) -> Self {
        Self { container, items }
    }

    pub fn container(&self) -> FlexContainerMainAxisLayout {
        self.container
    }

    pub fn items(&self) -> &[FlexItemMainAxisLayout] {
        &self.items
    }

    pub fn into_items(self) -> Vec<FlexItemMainAxisLayout> {
        self.items
    }

    pub fn to_debug_snapshot(&self) -> String {
        let mut out = String::new();
        writeln!(&mut out, "version: 1").expect("write snapshot");
        writeln!(&mut out, "flex-main-axis").expect("write snapshot");
        writeln!(&mut out, "container: {}", self.container.as_debug_label())
            .expect("write snapshot");
        for (index, item) in self.items.iter().enumerate() {
            writeln!(&mut out, "item[{index}]: {}", item.as_debug_label()).expect("write snapshot");
        }
        out
    }
}

pub fn resolve_flex_main_axis_layout(
    available_main_size: CssPx,
    items: &[FlexItemMainAxisInput],
) -> FlexMainAxisLayout {
    let total_outer_base = items.iter().map(|item| item.outer_base_size()).sum::<f32>();
    let free_space_value = available_main_size.get() - total_outer_base;

    let distribution = distribution_for(items, free_space_value);
    let target_sizes = target_sizes_for_distribution(items, free_space_value, distribution);

    let mut cursor = 0.0;
    let mut layout_items = Vec::with_capacity(items.len());
    for (item, target_size) in items.iter().copied().zip(target_sizes) {
        cursor += item.margin_start().get();
        let offset = signed_px_from_finite(cursor, "flex item main offset");
        layout_items.push(FlexItemMainAxisLayout::new(item, target_size, offset));
        cursor += target_size.get() + item.margin_end().get();
    }

    let container = FlexContainerMainAxisLayout::new(
        FlexMainAxis::Row,
        available_main_size,
        signed_px_from_finite(total_outer_base, "flex total outer base size"),
        signed_px_from_finite(free_space_value, "flex free space"),
        distribution,
    );

    FlexMainAxisLayout::new(container, layout_items)
}

pub fn resolve_flex_cross_axis_layout(
    available_cross_size: Option<CssPx>,
    items: &[FlexItemCrossAxisInput],
) -> FlexCrossAxisLayout {
    let auto_cross_size = items
        .iter()
        .map(|item| item.outer_hypothetical_size())
        .fold(0.0, f32::max);
    let auto_cross_size = css_px_from_nonnegative(auto_cross_size, "flex auto cross size");
    let used_cross_size = available_cross_size.unwrap_or(auto_cross_size);
    let line_cross_size = used_cross_size;

    let layout_items = items
        .iter()
        .copied()
        .map(|item| {
            let target_cross_size = target_cross_size_for_item(item, line_cross_size);
            let cross_offset = cross_offset_for_item(item, target_cross_size, line_cross_size);
            FlexItemCrossAxisLayout::new(item, target_cross_size, cross_offset)
        })
        .collect();

    let container = FlexContainerCrossAxisLayout::new(
        FlexCrossAxis::Block,
        available_cross_size,
        line_cross_size,
        auto_cross_size,
        used_cross_size,
    );

    FlexCrossAxisLayout::new(container, layout_items)
}

fn distribution_for(items: &[FlexItemMainAxisInput], free_space: f32) -> FlexFreeSpaceDistribution {
    if items.is_empty() {
        return FlexFreeSpaceDistribution::NoItems;
    }

    if free_space == 0.0 {
        return FlexFreeSpaceDistribution::ExactFit;
    }

    if free_space > 0.0 {
        let grow_sum = items.iter().map(|item| item.flex_grow()).sum::<f32>();
        if grow_sum > 0.0 {
            FlexFreeSpaceDistribution::PositiveGrow
        } else {
            FlexFreeSpaceDistribution::PositiveNoGrow
        }
    } else {
        let shrink_sum = items
            .iter()
            .map(|item| item.scaled_shrink_factor())
            .sum::<f32>();
        if shrink_sum > 0.0 {
            FlexFreeSpaceDistribution::NegativeShrink
        } else {
            FlexFreeSpaceDistribution::NegativeNoShrink
        }
    }
}

fn target_sizes_for_distribution(
    items: &[FlexItemMainAxisInput],
    free_space: f32,
    distribution: FlexFreeSpaceDistribution,
) -> Vec<CssPx> {
    match distribution {
        FlexFreeSpaceDistribution::PositiveGrow => {
            let grow_sum = items.iter().map(|item| item.flex_grow()).sum::<f32>();
            items
                .iter()
                .map(|item| {
                    css_px_from_nonnegative(
                        item.base_main_size().get() + free_space * (item.flex_grow() / grow_sum),
                        "flex grown target size",
                    )
                })
                .collect()
        }
        FlexFreeSpaceDistribution::NegativeShrink => {
            let shrink_sum = items
                .iter()
                .map(|item| item.scaled_shrink_factor())
                .sum::<f32>();
            items
                .iter()
                .map(|item| {
                    let shrink = free_space.abs() * (item.scaled_shrink_factor() / shrink_sum);
                    css_px_from_nonnegative(
                        (item.base_main_size().get() - shrink).max(0.0),
                        "flex shrunken target size",
                    )
                })
                .collect()
        }
        FlexFreeSpaceDistribution::NoItems
        | FlexFreeSpaceDistribution::ExactFit
        | FlexFreeSpaceDistribution::PositiveNoGrow
        | FlexFreeSpaceDistribution::NegativeNoShrink => {
            items.iter().map(|item| item.base_main_size()).collect()
        }
    }
}

fn target_cross_size_for_item(item: FlexItemCrossAxisInput, line_cross_size: CssPx) -> CssPx {
    if item.alignment() == FlexCrossAxisAlignment::Stretch && item.can_stretch() {
        let stretched =
            line_cross_size.get() - item.margin_before().get() - item.margin_after().get();
        css_px_from_nonnegative(stretched, "flex stretched cross size")
    } else {
        item.hypothetical_cross_size()
    }
}

fn cross_offset_for_item(
    item: FlexItemCrossAxisInput,
    target_cross_size: CssPx,
    line_cross_size: CssPx,
) -> SignedCssPx {
    let outer_target =
        item.margin_before().get() + target_cross_size.get() + item.margin_after().get();
    let free_space = line_cross_size.get() - outer_target;
    let offset = match item.alignment() {
        FlexCrossAxisAlignment::Start | FlexCrossAxisAlignment::Stretch => {
            item.margin_before().get()
        }
        FlexCrossAxisAlignment::Center => item.margin_before().get() + free_space / 2.0,
        FlexCrossAxisAlignment::End => item.margin_before().get() + free_space,
    };
    signed_px_from_finite(offset, "flex item cross offset")
}

fn bool_debug_label(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

fn optional_css_px_debug_label(value: Option<CssPx>) -> String {
    value
        .map(css_px_debug_label)
        .unwrap_or_else(|| "auto".to_string())
}

fn is_valid_factor(value: f32) -> bool {
    value.is_finite() && value >= 0.0
}

fn css_px_from_nonnegative(value: f32, label: &str) -> CssPx {
    CssPx::new(value.max(0.0)).unwrap_or_else(|| panic!("{label} must be finite: {value}"))
}

fn signed_px_from_finite(value: f32, label: &str) -> SignedCssPx {
    SignedCssPx::new(value).unwrap_or_else(|| panic!("{label} must be finite: {value}"))
}

fn css_px_debug_label(value: CssPx) -> String {
    format!("{:.2}px", value.get())
}

fn signed_css_px_debug_label(value: SignedCssPx) -> String {
    format!("{:.2}px", value.get())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_items_records_deterministic_noop_distribution() {
        let layout = resolve_flex_main_axis_layout(px(200.0), &[]);

        assert_eq!(layout.container().axis(), FlexMainAxis::Row);
        assert_eq!(
            layout.container().distribution(),
            FlexFreeSpaceDistribution::NoItems
        );
        assert_eq!(layout.container().free_space(), spx(200.0));
        assert!(layout.items().is_empty());
    }

    #[test]
    fn fixed_items_are_placed_in_generated_order_with_start_packing() {
        let layout = resolve_flex_main_axis_layout(
            px(300.0),
            &[
                item(80.0, 0.0, 10.0, 0.0, 1.0),
                item(60.0, 5.0, 0.0, 0.0, 1.0),
            ],
        );

        assert_eq!(
            layout.container().distribution(),
            FlexFreeSpaceDistribution::PositiveNoGrow
        );
        assert_eq!(layout.items()[0].main_offset(), spx(0.0));
        assert_eq!(layout.items()[0].target_main_size(), px(80.0));
        assert_eq!(layout.items()[1].main_offset(), spx(95.0));
        assert_eq!(layout.items()[1].target_main_size(), px(60.0));
    }

    #[test]
    fn positive_free_space_can_grow_with_typed_internal_factors() {
        let layout = resolve_flex_main_axis_layout(
            px(220.0),
            &[
                item(50.0, 0.0, 0.0, 1.0, 1.0),
                item(50.0, 0.0, 0.0, 3.0, 1.0),
            ],
        );

        assert_eq!(
            layout.container().distribution(),
            FlexFreeSpaceDistribution::PositiveGrow
        );
        assert_eq!(layout.items()[0].target_main_size(), px(80.0));
        assert_eq!(layout.items()[1].target_main_size(), px(140.0));
        assert_eq!(layout.items()[1].main_offset(), spx(80.0));
    }

    #[test]
    fn negative_free_space_shrinks_by_scaled_shrink_factor() {
        let layout = resolve_flex_main_axis_layout(
            px(100.0),
            &[
                item(100.0, 0.0, 0.0, 0.0, 1.0),
                item(50.0, 0.0, 0.0, 0.0, 2.0),
            ],
        );

        assert_eq!(
            layout.container().distribution(),
            FlexFreeSpaceDistribution::NegativeShrink
        );
        assert_eq!(layout.items()[0].target_main_size(), px(75.0));
        assert_eq!(layout.items()[1].target_main_size(), px(25.0));
        assert_eq!(layout.items()[1].main_offset(), spx(75.0));
    }

    #[test]
    fn zero_shrink_denominator_is_deterministic() {
        let layout = resolve_flex_main_axis_layout(
            px(50.0),
            &[
                item(40.0, 0.0, 0.0, 0.0, 0.0),
                item(40.0, 0.0, 0.0, 0.0, 0.0),
            ],
        );

        assert_eq!(
            layout.container().distribution(),
            FlexFreeSpaceDistribution::NegativeNoShrink
        );
        assert_eq!(layout.items()[0].target_main_size(), px(40.0));
        assert_eq!(layout.items()[1].target_main_size(), px(40.0));
    }

    #[test]
    fn flex_cross_no_items_records_deterministic_container_sizes() {
        let auto = resolve_flex_cross_axis_layout(None, &[]);
        assert_eq!(auto.container().axis(), FlexCrossAxis::Block);
        assert_eq!(auto.container().available_cross_size(), None);
        assert_eq!(auto.container().line_cross_size(), px(0.0));
        assert_eq!(auto.container().auto_cross_size(), px(0.0));
        assert_eq!(auto.container().used_cross_size(), px(0.0));
        assert!(auto.items().is_empty());

        let explicit = resolve_flex_cross_axis_layout(Some(px(80.0)), &[]);
        assert_eq!(explicit.container().available_cross_size(), Some(px(80.0)));
        assert_eq!(explicit.container().line_cross_size(), px(80.0));
        assert_eq!(explicit.container().auto_cross_size(), px(0.0));
        assert_eq!(explicit.container().used_cross_size(), px(80.0));
    }

    #[test]
    fn flex_cross_auto_container_uses_max_outer_hypothetical_size() {
        let layout = resolve_flex_cross_axis_layout(
            None,
            &[
                cross_item(10.0, 2.0, 3.0, FlexCrossAxisAlignment::Stretch, true),
                cross_item(20.0, 1.0, 4.0, FlexCrossAxisAlignment::Stretch, true),
            ],
        );

        assert_eq!(layout.container().auto_cross_size(), px(25.0));
        assert_eq!(layout.container().line_cross_size(), px(25.0));
        assert_eq!(layout.items()[0].target_cross_size(), px(20.0));
        assert_eq!(layout.items()[0].cross_offset(), spx(2.0));
        assert_eq!(layout.items()[1].target_cross_size(), px(20.0));
        assert_eq!(layout.items()[1].cross_offset(), spx(1.0));
    }

    #[test]
    fn flex_cross_explicit_container_drives_stretch_target_size() {
        let layout = resolve_flex_cross_axis_layout(
            Some(px(60.0)),
            &[cross_item(
                10.0,
                5.0,
                7.0,
                FlexCrossAxisAlignment::Stretch,
                true,
            )],
        );

        assert_eq!(layout.container().line_cross_size(), px(60.0));
        assert_eq!(layout.container().auto_cross_size(), px(22.0));
        assert_eq!(layout.items()[0].target_cross_size(), px(48.0));
        assert_eq!(layout.items()[0].cross_offset(), spx(5.0));
    }

    #[test]
    fn flex_cross_alignment_offsets_are_deterministic() {
        let layout = resolve_flex_cross_axis_layout(
            Some(px(100.0)),
            &[
                cross_item(20.0, 5.0, 5.0, FlexCrossAxisAlignment::Start, false),
                cross_item(20.0, 5.0, 5.0, FlexCrossAxisAlignment::Center, false),
                cross_item(20.0, 5.0, 5.0, FlexCrossAxisAlignment::End, false),
            ],
        );

        assert_eq!(layout.items()[0].cross_offset(), spx(5.0));
        assert_eq!(layout.items()[1].cross_offset(), spx(40.0));
        assert_eq!(layout.items()[2].cross_offset(), spx(75.0));
    }

    #[test]
    fn flex_cross_stretch_clamps_negative_target_size_to_zero() {
        let layout = resolve_flex_cross_axis_layout(
            Some(px(10.0)),
            &[cross_item(
                30.0,
                8.0,
                9.0,
                FlexCrossAxisAlignment::Stretch,
                true,
            )],
        );

        assert_eq!(layout.items()[0].target_cross_size(), px(0.0));
        assert_eq!(layout.items()[0].cross_offset(), spx(8.0));
    }

    fn item(
        base: f32,
        margin_start: f32,
        margin_end: f32,
        grow: f32,
        shrink: f32,
    ) -> FlexItemMainAxisInput {
        FlexItemMainAxisInput::new(px(base), spx(margin_start), spx(margin_end), grow, shrink)
            .expect("valid flex item input")
    }

    fn cross_item(
        hypothetical: f32,
        margin_before: f32,
        margin_after: f32,
        alignment: FlexCrossAxisAlignment,
        can_stretch: bool,
    ) -> FlexItemCrossAxisInput {
        FlexItemCrossAxisInput::new(
            px(hypothetical),
            spx(margin_before),
            spx(margin_after),
            alignment,
            can_stretch,
        )
    }

    fn px(value: f32) -> CssPx {
        CssPx::new(value).expect("valid css px")
    }

    fn spx(value: f32) -> SignedCssPx {
        SignedCssPx::new(value).expect("valid signed css px")
    }
}
