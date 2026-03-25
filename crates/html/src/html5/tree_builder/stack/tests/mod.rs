mod basic;
mod foster;
mod scope;
mod table;

use crate::html5::shared::DocumentParseContext;
use crate::html5::tree_builder::stack::ScopeTagSet;

fn make_scope_tags(ctx: &mut DocumentParseContext) -> ScopeTagSet {
    ScopeTagSet {
        html: ctx.atoms.intern_ascii_folded("html").expect("atom"),
        table: ctx.atoms.intern_ascii_folded("table").expect("atom"),
        template: ctx.atoms.intern_ascii_folded("template").expect("atom"),
        td: ctx.atoms.intern_ascii_folded("td").expect("atom"),
        th: ctx.atoms.intern_ascii_folded("th").expect("atom"),
        caption: ctx.atoms.intern_ascii_folded("caption").expect("atom"),
        marquee: ctx.atoms.intern_ascii_folded("marquee").expect("atom"),
        object: ctx.atoms.intern_ascii_folded("object").expect("atom"),
        applet: ctx.atoms.intern_ascii_folded("applet").expect("atom"),
        button: ctx.atoms.intern_ascii_folded("button").expect("atom"),
        ol: ctx.atoms.intern_ascii_folded("ol").expect("atom"),
        ul: ctx.atoms.intern_ascii_folded("ul").expect("atom"),
    }
}
