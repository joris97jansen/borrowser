//! HTML5 tree builder API (placeholder).

#[derive(Clone, Debug, Default)]
pub struct TreeBuilderConfig;

use crate::html5::shared::DocumentParseContext;

#[derive(Clone, Debug)]
pub struct Html5TreeBuilder;

#[derive(Clone, Debug)]
pub enum TreeBuilderStepResult {
    Continue,
    Suspend(SuspendReason),
}

#[derive(Clone, Debug)]
pub enum SuspendReason {
    Script,
    Other,
}

#[derive(Clone, Debug)]
pub struct TreeBuilderError;

impl Html5TreeBuilder {
    pub fn new(_config: TreeBuilderConfig, _ctx: &mut DocumentParseContext) -> Self {
        Self
    }
}

mod emit;
mod formatting;
mod modes;
mod stack;

#[cfg(test)]
mod tests;
