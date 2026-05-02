//! Pass-local computed-style reuse cache.

use crate::cascade::ResolvedStyle;

use super::{
    super::style::ComputedStyle, error::ComputedStyleResolutionError,
    materialize::compute_style_from_resolved_style, model::ComputedStyleReuseStats,
};

// Pass-local cache for computed-style materialization.
//
// Reuse is valid only while `compute_style_from_resolved_style(...)` is a pure
// function of `(ResolvedStyle, Option<ComputedStyle parent>)`. If future
// computed-value logic depends on additional environment inputs such as
// viewport units, font metrics, writing mode context, visited-link privacy
// state, container queries, or media/device state, those inputs must either be
// added to this cache key or this reuse path must be disabled for affected
// properties.
#[derive(Default)]
pub(super) struct ComputedStyleReuseCache {
    entries: Vec<ComputedStyleReuseEntry>,
    stats: ComputedStyleReuseStats,
}

impl ComputedStyleReuseCache {
    pub(super) fn seed(
        &mut self,
        resolved_style: &ResolvedStyle,
        parent_style: Option<&ComputedStyle>,
        computed: ComputedStyle,
    ) {
        let parent = parent_style.copied();
        if self
            .entries
            .iter()
            .any(|entry| entry.resolved == *resolved_style && entry.parent == parent)
        {
            return;
        }

        self.entries.push(ComputedStyleReuseEntry {
            resolved: resolved_style.clone(),
            parent,
            computed,
        });
    }

    pub(super) fn lookup_or_compute(
        &mut self,
        resolved_style: &ResolvedStyle,
        parent_style: Option<&ComputedStyle>,
    ) -> Result<ComputedStyle, ComputedStyleResolutionError> {
        let parent = parent_style.copied();
        if let Some(entry) = self
            .entries
            .iter()
            .find(|entry| entry.resolved == *resolved_style && entry.parent == parent)
        {
            self.stats.hits = self.stats.hits.saturating_add(1);
            return Ok(entry.computed);
        }

        self.stats.misses = self.stats.misses.saturating_add(1);
        let computed = compute_style_from_resolved_style(resolved_style, parent_style)?;
        self.entries.push(ComputedStyleReuseEntry {
            resolved: resolved_style.clone(),
            parent,
            computed,
        });
        Ok(computed)
    }

    pub(super) fn stats(&self) -> ComputedStyleReuseStats {
        self.stats
    }
}

struct ComputedStyleReuseEntry {
    resolved: ResolvedStyle,
    parent: Option<ComputedStyle>,
    computed: ComputedStyle,
}
