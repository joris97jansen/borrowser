#![allow(
    dead_code,
    reason = "table-mode state plumbing lands incrementally across Milestone I"
)]

mod close;
mod delegation;
mod in_caption;
mod in_cell;
mod in_column_group;
mod in_row;
mod in_table;
mod in_table_body;
mod in_table_text;
mod scope;
mod state;

pub(in crate::html5::tree_builder) use state::PendingTableCharacterTokens;
