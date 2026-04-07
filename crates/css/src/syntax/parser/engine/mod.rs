mod components;
mod declarations;
mod recovery;
mod rules;
mod state;

use super::model::{CssDeclaration, CssDeclarationBlock, CssFunction, CssRule, CssSimpleBlock};

pub(super) use self::state::StylesheetParser;

enum RuleParseResult {
    Parsed(CssRule, usize),
    Skipped(usize),
    End,
}

enum DeclarationResult {
    Parsed(CssDeclaration, usize),
    Skipped(usize),
    End,
}

struct ConsumedDeclarationBlock {
    block: CssDeclarationBlock,
    next_index: usize,
    closed: bool,
}

struct ConsumedSimpleBlock {
    block: CssSimpleBlock,
    next_index: usize,
    closed: bool,
}

struct ConsumedFunction {
    function: CssFunction,
    next_index: usize,
}
