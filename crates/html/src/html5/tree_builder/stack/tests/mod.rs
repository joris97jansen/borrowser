mod basic;
mod end_tag;
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
        select: ctx.atoms.intern_ascii_folded("select").expect("atom"),
        button: ctx.atoms.intern_ascii_folded("button").expect("atom"),
        ol: ctx.atoms.intern_ascii_folded("ol").expect("atom"),
        ul: ctx.atoms.intern_ascii_folded("ul").expect("atom"),
        math_mi: ctx.atoms.intern_exact("mi").expect("atom"),
        math_mo: ctx.atoms.intern_exact("mo").expect("atom"),
        math_mn: ctx.atoms.intern_exact("mn").expect("atom"),
        math_ms: ctx.atoms.intern_exact("ms").expect("atom"),
        math_mtext: ctx.atoms.intern_exact("mtext").expect("atom"),
        math_annotation_xml: ctx.atoms.intern_exact("annotation-xml").expect("atom"),
        svg_foreign_object: ctx.atoms.intern_exact("foreignObject").expect("atom"),
        svg_desc: ctx.atoms.intern_exact("desc").expect("atom"),
        svg_title: ctx.atoms.intern_exact("title").expect("atom"),
    }
}
