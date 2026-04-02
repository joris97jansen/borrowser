use crate::html5::shared::{AtomError, AtomId, AtomTable};
use crate::html5::tree_builder::stack::ScopeTagSet;

#[derive(Clone, Copy, Debug)]
pub(in crate::html5::tree_builder) struct KnownTagIds {
    pub(in crate::html5::tree_builder) a: AtomId,
    pub(in crate::html5::tree_builder) b: AtomId,
    pub(in crate::html5::tree_builder) big: AtomId,
    pub(in crate::html5::tree_builder) code: AtomId,
    pub(in crate::html5::tree_builder) em: AtomId,
    pub(in crate::html5::tree_builder) font: AtomId,
    pub(in crate::html5::tree_builder) html: AtomId,
    pub(in crate::html5::tree_builder) head: AtomId,
    pub(in crate::html5::tree_builder) body: AtomId,
    pub(in crate::html5::tree_builder) base: AtomId,
    pub(in crate::html5::tree_builder) br: AtomId,
    pub(in crate::html5::tree_builder) embed: AtomId,
    pub(in crate::html5::tree_builder) hr: AtomId,
    pub(in crate::html5::tree_builder) img: AtomId,
    pub(in crate::html5::tree_builder) input: AtomId,
    pub(in crate::html5::tree_builder) link: AtomId,
    pub(in crate::html5::tree_builder) meta: AtomId,
    pub(in crate::html5::tree_builder) param: AtomId,
    pub(in crate::html5::tree_builder) source: AtomId,
    pub(in crate::html5::tree_builder) track: AtomId,
    pub(in crate::html5::tree_builder) wbr: AtomId,
    pub(in crate::html5::tree_builder) p: AtomId,
    pub(in crate::html5::tree_builder) i: AtomId,
    pub(in crate::html5::tree_builder) nobr: AtomId,
    pub(in crate::html5::tree_builder) s: AtomId,
    pub(in crate::html5::tree_builder) script: AtomId,
    pub(in crate::html5::tree_builder) small: AtomId,
    pub(in crate::html5::tree_builder) strike: AtomId,
    pub(in crate::html5::tree_builder) strong: AtomId,
    pub(in crate::html5::tree_builder) style: AtomId,
    pub(in crate::html5::tree_builder) title: AtomId,
    pub(in crate::html5::tree_builder) tt: AtomId,
    pub(in crate::html5::tree_builder) textarea: AtomId,
    pub(in crate::html5::tree_builder) table: AtomId,
    pub(in crate::html5::tree_builder) template: AtomId,
    pub(in crate::html5::tree_builder) tbody: AtomId,
    pub(in crate::html5::tree_builder) td: AtomId,
    pub(in crate::html5::tree_builder) tfoot: AtomId,
    pub(in crate::html5::tree_builder) th: AtomId,
    pub(in crate::html5::tree_builder) thead: AtomId,
    pub(in crate::html5::tree_builder) caption: AtomId,
    #[allow(
        dead_code,
        reason = "table-family insertion-mode dispatch lands incrementally across Milestone I"
    )]
    pub(in crate::html5::tree_builder) col: AtomId,
    #[allow(
        dead_code,
        reason = "table-family insertion-mode dispatch lands incrementally across Milestone I"
    )]
    pub(in crate::html5::tree_builder) colgroup: AtomId,
    pub(in crate::html5::tree_builder) marquee: AtomId,
    pub(in crate::html5::tree_builder) object: AtomId,
    pub(in crate::html5::tree_builder) applet: AtomId,
    pub(in crate::html5::tree_builder) button: AtomId,
    pub(in crate::html5::tree_builder) ol: AtomId,
    pub(in crate::html5::tree_builder) u: AtomId,
    pub(in crate::html5::tree_builder) ul: AtomId,
    pub(in crate::html5::tree_builder) li: AtomId,
    pub(in crate::html5::tree_builder) tr: AtomId,
}

impl KnownTagIds {
    pub(in crate::html5::tree_builder) fn intern(atoms: &mut AtomTable) -> Result<Self, AtomError> {
        Ok(Self {
            a: atoms.intern_ascii_folded("a")?,
            b: atoms.intern_ascii_folded("b")?,
            big: atoms.intern_ascii_folded("big")?,
            code: atoms.intern_ascii_folded("code")?,
            em: atoms.intern_ascii_folded("em")?,
            font: atoms.intern_ascii_folded("font")?,
            html: atoms.intern_ascii_folded("html")?,
            head: atoms.intern_ascii_folded("head")?,
            body: atoms.intern_ascii_folded("body")?,
            base: atoms.intern_ascii_folded("base")?,
            br: atoms.intern_ascii_folded("br")?,
            embed: atoms.intern_ascii_folded("embed")?,
            hr: atoms.intern_ascii_folded("hr")?,
            img: atoms.intern_ascii_folded("img")?,
            input: atoms.intern_ascii_folded("input")?,
            link: atoms.intern_ascii_folded("link")?,
            meta: atoms.intern_ascii_folded("meta")?,
            param: atoms.intern_ascii_folded("param")?,
            source: atoms.intern_ascii_folded("source")?,
            track: atoms.intern_ascii_folded("track")?,
            wbr: atoms.intern_ascii_folded("wbr")?,
            p: atoms.intern_ascii_folded("p")?,
            i: atoms.intern_ascii_folded("i")?,
            nobr: atoms.intern_ascii_folded("nobr")?,
            s: atoms.intern_ascii_folded("s")?,
            script: atoms.intern_ascii_folded("script")?,
            small: atoms.intern_ascii_folded("small")?,
            strike: atoms.intern_ascii_folded("strike")?,
            strong: atoms.intern_ascii_folded("strong")?,
            style: atoms.intern_ascii_folded("style")?,
            title: atoms.intern_ascii_folded("title")?,
            tt: atoms.intern_ascii_folded("tt")?,
            textarea: atoms.intern_ascii_folded("textarea")?,
            table: atoms.intern_ascii_folded("table")?,
            template: atoms.intern_ascii_folded("template")?,
            tbody: atoms.intern_ascii_folded("tbody")?,
            td: atoms.intern_ascii_folded("td")?,
            tfoot: atoms.intern_ascii_folded("tfoot")?,
            th: atoms.intern_ascii_folded("th")?,
            thead: atoms.intern_ascii_folded("thead")?,
            caption: atoms.intern_ascii_folded("caption")?,
            col: atoms.intern_ascii_folded("col")?,
            colgroup: atoms.intern_ascii_folded("colgroup")?,
            marquee: atoms.intern_ascii_folded("marquee")?,
            object: atoms.intern_ascii_folded("object")?,
            applet: atoms.intern_ascii_folded("applet")?,
            button: atoms.intern_ascii_folded("button")?,
            ol: atoms.intern_ascii_folded("ol")?,
            u: atoms.intern_ascii_folded("u")?,
            ul: atoms.intern_ascii_folded("ul")?,
            li: atoms.intern_ascii_folded("li")?,
            tr: atoms.intern_ascii_folded("tr")?,
        })
    }

    #[inline]
    pub(in crate::html5::tree_builder) fn is_formatting_tag(&self, name: AtomId) -> bool {
        name == self.a
            || name == self.b
            || name == self.big
            || name == self.code
            || name == self.em
            || name == self.font
            || name == self.i
            || name == self.nobr
            || name == self.s
            || name == self.small
            || name == self.strike
            || name == self.strong
            || name == self.tt
            || name == self.u
    }

    #[inline]
    pub(in crate::html5::tree_builder) fn is_marker_tag(&self, name: AtomId) -> bool {
        name == self.applet || name == self.marquee || name == self.object
    }

    #[inline]
    pub(in crate::html5::tree_builder) fn is_void_tag(&self, name: AtomId) -> bool {
        name == self.base
            || name == self.br
            || name == self.col
            || name == self.embed
            || name == self.hr
            || name == self.img
            || name == self.input
            || name == self.link
            || name == self.meta
            || name == self.param
            || name == self.source
            || name == self.track
            || name == self.wbr
    }

    #[inline]
    pub(in crate::html5::tree_builder) fn scope_tags(&self) -> ScopeTagSet {
        ScopeTagSet {
            html: self.html,
            table: self.table,
            template: self.template,
            td: self.td,
            th: self.th,
            caption: self.caption,
            marquee: self.marquee,
            object: self.object,
            applet: self.applet,
            button: self.button,
            ol: self.ol,
            ul: self.ul,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::KnownTagIds;
    use crate::html5::shared::DocumentParseContext;

    #[test]
    fn known_tag_scope_tag_view_shares_ids() {
        let mut ctx = DocumentParseContext::new();
        let known = KnownTagIds::intern(&mut ctx.atoms).expect("known tags");
        let scope = known.scope_tags();

        assert_eq!(scope.html, known.html);
        assert_eq!(scope.table, known.table);
        assert_eq!(scope.template, known.template);
        assert_eq!(scope.td, known.td);
        assert_eq!(scope.th, known.th);
        assert_eq!(scope.caption, known.caption);
        assert_eq!(scope.marquee, known.marquee);
        assert_eq!(scope.object, known.object);
        assert_eq!(scope.applet, known.applet);
        assert_eq!(scope.button, known.button);
        assert_eq!(scope.ol, known.ol);
        assert_eq!(scope.ul, known.ul);
    }

    #[test]
    fn known_tag_helpers_classify_formatting_and_marker_tags() {
        let mut ctx = DocumentParseContext::new();
        let known = KnownTagIds::intern(&mut ctx.atoms).expect("known tags");

        assert!(known.is_formatting_tag(known.b));
        assert!(known.is_formatting_tag(known.strong));
        assert!(known.is_formatting_tag(known.a));
        assert!(!known.is_formatting_tag(known.body));

        assert!(known.is_marker_tag(known.applet));
        assert!(known.is_marker_tag(known.marquee));
        assert!(known.is_marker_tag(known.object));
        assert!(!known.is_marker_tag(known.b));
    }
}
