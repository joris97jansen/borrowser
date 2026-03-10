use crate::html5::shared::{AtomError, AtomId, AtomTable};
use crate::html5::tree_builder::stack::ScopeTagSet;

#[derive(Clone, Copy, Debug)]
pub(in crate::html5::tree_builder) struct KnownTagIds {
    pub(in crate::html5::tree_builder) html: AtomId,
    pub(in crate::html5::tree_builder) head: AtomId,
    pub(in crate::html5::tree_builder) body: AtomId,
    pub(in crate::html5::tree_builder) script: AtomId,
    pub(in crate::html5::tree_builder) style: AtomId,
    pub(in crate::html5::tree_builder) title: AtomId,
    pub(in crate::html5::tree_builder) textarea: AtomId,
    pub(in crate::html5::tree_builder) table: AtomId,
    pub(in crate::html5::tree_builder) template: AtomId,
    pub(in crate::html5::tree_builder) td: AtomId,
    pub(in crate::html5::tree_builder) th: AtomId,
    pub(in crate::html5::tree_builder) caption: AtomId,
    pub(in crate::html5::tree_builder) marquee: AtomId,
    pub(in crate::html5::tree_builder) object: AtomId,
    pub(in crate::html5::tree_builder) applet: AtomId,
    pub(in crate::html5::tree_builder) button: AtomId,
    pub(in crate::html5::tree_builder) ol: AtomId,
    pub(in crate::html5::tree_builder) ul: AtomId,
    pub(in crate::html5::tree_builder) li: AtomId,
}

impl KnownTagIds {
    pub(in crate::html5::tree_builder) fn intern(atoms: &mut AtomTable) -> Result<Self, AtomError> {
        Ok(Self {
            html: atoms.intern_ascii_folded("html")?,
            head: atoms.intern_ascii_folded("head")?,
            body: atoms.intern_ascii_folded("body")?,
            script: atoms.intern_ascii_folded("script")?,
            style: atoms.intern_ascii_folded("style")?,
            title: atoms.intern_ascii_folded("title")?,
            textarea: atoms.intern_ascii_folded("textarea")?,
            table: atoms.intern_ascii_folded("table")?,
            template: atoms.intern_ascii_folded("template")?,
            td: atoms.intern_ascii_folded("td")?,
            th: atoms.intern_ascii_folded("th")?,
            caption: atoms.intern_ascii_folded("caption")?,
            marquee: atoms.intern_ascii_folded("marquee")?,
            object: atoms.intern_ascii_folded("object")?,
            applet: atoms.intern_ascii_folded("applet")?,
            button: atoms.intern_ascii_folded("button")?,
            ol: atoms.intern_ascii_folded("ol")?,
            ul: atoms.intern_ascii_folded("ul")?,
            li: atoms.intern_ascii_folded("li")?,
        })
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
}
