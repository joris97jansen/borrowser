use crate::html5::shared::{
    AtomError, AtomId, AtomTable, DocumentParseContext, ParserFatalError, ParserReservationSite,
};
use crate::html5::tree_builder::stack::ScopeTagSet;
use crate::names::NameInternerStorageReservationError;

const QUALIFIED_FOREIGN_ATTRIBUTE_LOCAL_NAMES: [&str; 12] = [
    "definitionURL",
    "actuate",
    "arcrole",
    "href",
    "role",
    "show",
    "title",
    "type",
    "lang",
    "space",
    "xmlns",
    "xlink",
];

/// Number of atom identities retained as fields by `KnownTagIds`.
///
/// Update this component whenever a retained field is added or removed. The
/// bootstrap regression test independently counts every supported interning
/// call through `KnownTagBootstrap`.
const RETAINED_KNOWN_TAG_FIELD_COUNT: usize = 88;

const KNOWN_TAG_BOOTSTRAP_RESERVATION_BOUND: usize =
    crate::html5::tree_builder::foreign::SVG_TAG_NAME_ADJUSTMENTS.len()
        + crate::html5::tree_builder::foreign::SVG_ATTRIBUTE_ADJUSTMENTS.len()
        + QUALIFIED_FOREIGN_ATTRIBUTE_LOCAL_NAMES.len()
        + RETAINED_KNOWN_TAG_FIELD_COUNT;

struct KnownTagBootstrap<'a> {
    atoms: &'a mut AtomTable,
    #[cfg(test)]
    calls: usize,
}

impl<'a> KnownTagBootstrap<'a> {
    fn new(atoms: &'a mut AtomTable) -> Self {
        Self {
            atoms,
            #[cfg(test)]
            calls: 0,
        }
    }

    fn intern_exact(&mut self, name: &str) -> Result<AtomId, ParserFatalError> {
        #[cfg(test)]
        {
            self.calls += 1;
        }
        self.atoms.intern_exact(name).map_err(atom_bootstrap_error)
    }

    fn intern_ascii_folded(&mut self, name: &str) -> Result<AtomId, ParserFatalError> {
        #[cfg(test)]
        {
            self.calls += 1;
        }
        self.atoms
            .intern_ascii_folded(name)
            .map_err(atom_bootstrap_error)
    }
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct KnownTagBootstrapStats {
    calls: usize,
    unique_entries: usize,
    capacities_after_reservation: (usize, usize),
    capacities_after_population: (usize, usize),
}

fn atom_bootstrap_error(error: AtomError) -> ParserFatalError {
    match error {
        AtomError::InvalidUtf8 | AtomError::OutOfIds => ParserFatalError::EngineInvariant,
    }
}

fn reserve_atom_storage(ctx: &mut DocumentParseContext) -> Result<(), ParserFatalError> {
    let site = ParserReservationSite::KnownTagAtomStorage;
    ctx.before_reservation(site)
        .map_err(ParserFatalError::ResourceExhaustion)?;
    match ctx
        .atoms
        .try_reserve_atom_storage(KNOWN_TAG_BOOTSTRAP_RESERVATION_BOUND)
    {
        Ok(()) => Ok(()),
        Err(NameInternerStorageReservationError::LengthOverflow) => {
            Err(ParserFatalError::EngineInvariant)
        }
        Err(NameInternerStorageReservationError::ReservationFailed) => {
            Err(ParserFatalError::ResourceExhaustion(
                crate::html5::shared::ParserResourceExhaustion::at(site),
            ))
        }
    }
}

fn reserve_lookup_storage(ctx: &mut DocumentParseContext) -> Result<(), ParserFatalError> {
    let site = ParserReservationSite::KnownTagLookupStorage;
    ctx.before_reservation(site)
        .map_err(ParserFatalError::ResourceExhaustion)?;
    match ctx
        .atoms
        .try_reserve_lookup_storage(KNOWN_TAG_BOOTSTRAP_RESERVATION_BOUND)
    {
        Ok(()) => Ok(()),
        Err(NameInternerStorageReservationError::LengthOverflow) => {
            Err(ParserFatalError::EngineInvariant)
        }
        Err(NameInternerStorageReservationError::ReservationFailed) => {
            Err(ParserFatalError::ResourceExhaustion(
                crate::html5::shared::ParserResourceExhaustion::at(site),
            ))
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(in crate::html5::tree_builder) struct KnownTagIds {
    pub(in crate::html5::tree_builder) a: AtomId,
    pub(in crate::html5::tree_builder) address: AtomId,
    pub(in crate::html5::tree_builder) article: AtomId,
    pub(in crate::html5::tree_builder) aside: AtomId,
    pub(in crate::html5::tree_builder) b: AtomId,
    pub(in crate::html5::tree_builder) big: AtomId,
    pub(in crate::html5::tree_builder) blockquote: AtomId,
    pub(in crate::html5::tree_builder) code: AtomId,
    pub(in crate::html5::tree_builder) div: AtomId,
    pub(in crate::html5::tree_builder) em: AtomId,
    pub(in crate::html5::tree_builder) footer: AtomId,
    pub(in crate::html5::tree_builder) font: AtomId,
    pub(in crate::html5::tree_builder) form: AtomId,
    pub(in crate::html5::tree_builder) fieldset: AtomId,
    pub(in crate::html5::tree_builder) h1: AtomId,
    pub(in crate::html5::tree_builder) h2: AtomId,
    pub(in crate::html5::tree_builder) h3: AtomId,
    pub(in crate::html5::tree_builder) h4: AtomId,
    pub(in crate::html5::tree_builder) h5: AtomId,
    pub(in crate::html5::tree_builder) h6: AtomId,
    pub(in crate::html5::tree_builder) header: AtomId,
    pub(in crate::html5::tree_builder) html: AtomId,
    pub(in crate::html5::tree_builder) head: AtomId,
    pub(in crate::html5::tree_builder) body: AtomId,
    pub(in crate::html5::tree_builder) area: AtomId,
    pub(in crate::html5::tree_builder) base: AtomId,
    pub(in crate::html5::tree_builder) basefont: AtomId,
    pub(in crate::html5::tree_builder) bgsound: AtomId,
    pub(in crate::html5::tree_builder) br: AtomId,
    pub(in crate::html5::tree_builder) embed: AtomId,
    pub(in crate::html5::tree_builder) hr: AtomId,
    pub(in crate::html5::tree_builder) img: AtomId,
    pub(in crate::html5::tree_builder) input: AtomId,
    pub(in crate::html5::tree_builder) keygen: AtomId,
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
    pub(in crate::html5::tree_builder) select: AtomId,
    pub(in crate::html5::tree_builder) option: AtomId,
    pub(in crate::html5::tree_builder) optgroup: AtomId,
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
    pub(in crate::html5::tree_builder) main: AtomId,
    pub(in crate::html5::tree_builder) nav: AtomId,
    pub(in crate::html5::tree_builder) ol: AtomId,
    pub(in crate::html5::tree_builder) pre: AtomId,
    pub(in crate::html5::tree_builder) section: AtomId,
    pub(in crate::html5::tree_builder) u: AtomId,
    pub(in crate::html5::tree_builder) ul: AtomId,
    pub(in crate::html5::tree_builder) li: AtomId,
    pub(in crate::html5::tree_builder) tr: AtomId,
    pub(in crate::html5::tree_builder) svg: AtomId,
    pub(in crate::html5::tree_builder) math: AtomId,
    pub(in crate::html5::tree_builder) mi: AtomId,
    pub(in crate::html5::tree_builder) mo: AtomId,
    pub(in crate::html5::tree_builder) mn: AtomId,
    pub(in crate::html5::tree_builder) ms: AtomId,
    pub(in crate::html5::tree_builder) mtext: AtomId,
    pub(in crate::html5::tree_builder) annotation_xml: AtomId,
    pub(in crate::html5::tree_builder) desc: AtomId,
    pub(in crate::html5::tree_builder) foreign_object: AtomId,
}

impl KnownTagIds {
    pub(in crate::html5::tree_builder) fn intern(
        ctx: &mut DocumentParseContext,
    ) -> Result<Self, ParserFatalError> {
        Self::intern_impl(
            ctx,
            #[cfg(test)]
            None,
        )
    }

    #[cfg(test)]
    fn intern_with_stats(
        ctx: &mut DocumentParseContext,
    ) -> Result<(Self, KnownTagBootstrapStats), ParserFatalError> {
        let mut stats = KnownTagBootstrapStats::default();
        let known = Self::intern_impl(ctx, Some(&mut stats))?;
        Ok((known, stats))
    }

    fn intern_impl(
        ctx: &mut DocumentParseContext,
        #[cfg(test)] mut stats: Option<&mut KnownTagBootstrapStats>,
    ) -> Result<Self, ParserFatalError> {
        reserve_atom_storage(ctx)?;
        reserve_lookup_storage(ctx)?;
        #[cfg(test)]
        let initial_len = ctx.atoms.len();
        #[cfg(test)]
        if let Some(stats) = stats.as_deref_mut() {
            stats.capacities_after_reservation = ctx.atoms.storage_capacities();
        }
        let known = {
            let mut bootstrap = KnownTagBootstrap::new(&mut ctx.atoms);

            for &(_, adjusted) in &crate::html5::tree_builder::foreign::SVG_TAG_NAME_ADJUSTMENTS {
                let _ = bootstrap.intern_exact(adjusted)?;
            }
            for &(_, adjusted) in &crate::html5::tree_builder::foreign::SVG_ATTRIBUTE_ADJUSTMENTS {
                let _ = bootstrap.intern_exact(adjusted)?;
            }
            for local in QUALIFIED_FOREIGN_ATTRIBUTE_LOCAL_NAMES {
                let _ = bootstrap.intern_exact(local)?;
            }
            let known = Self {
                a: bootstrap.intern_ascii_folded("a")?,
                address: bootstrap.intern_ascii_folded("address")?,
                article: bootstrap.intern_ascii_folded("article")?,
                aside: bootstrap.intern_ascii_folded("aside")?,
                b: bootstrap.intern_ascii_folded("b")?,
                big: bootstrap.intern_ascii_folded("big")?,
                blockquote: bootstrap.intern_ascii_folded("blockquote")?,
                code: bootstrap.intern_ascii_folded("code")?,
                div: bootstrap.intern_ascii_folded("div")?,
                em: bootstrap.intern_ascii_folded("em")?,
                footer: bootstrap.intern_ascii_folded("footer")?,
                font: bootstrap.intern_ascii_folded("font")?,
                form: bootstrap.intern_ascii_folded("form")?,
                fieldset: bootstrap.intern_ascii_folded("fieldset")?,
                h1: bootstrap.intern_ascii_folded("h1")?,
                h2: bootstrap.intern_ascii_folded("h2")?,
                h3: bootstrap.intern_ascii_folded("h3")?,
                h4: bootstrap.intern_ascii_folded("h4")?,
                h5: bootstrap.intern_ascii_folded("h5")?,
                h6: bootstrap.intern_ascii_folded("h6")?,
                header: bootstrap.intern_ascii_folded("header")?,
                html: bootstrap.intern_ascii_folded("html")?,
                head: bootstrap.intern_ascii_folded("head")?,
                body: bootstrap.intern_ascii_folded("body")?,
                area: bootstrap.intern_ascii_folded("area")?,
                base: bootstrap.intern_ascii_folded("base")?,
                basefont: bootstrap.intern_ascii_folded("basefont")?,
                bgsound: bootstrap.intern_ascii_folded("bgsound")?,
                br: bootstrap.intern_ascii_folded("br")?,
                embed: bootstrap.intern_ascii_folded("embed")?,
                hr: bootstrap.intern_ascii_folded("hr")?,
                img: bootstrap.intern_ascii_folded("img")?,
                input: bootstrap.intern_ascii_folded("input")?,
                keygen: bootstrap.intern_ascii_folded("keygen")?,
                link: bootstrap.intern_ascii_folded("link")?,
                meta: bootstrap.intern_ascii_folded("meta")?,
                param: bootstrap.intern_ascii_folded("param")?,
                source: bootstrap.intern_ascii_folded("source")?,
                track: bootstrap.intern_ascii_folded("track")?,
                wbr: bootstrap.intern_ascii_folded("wbr")?,
                p: bootstrap.intern_ascii_folded("p")?,
                i: bootstrap.intern_ascii_folded("i")?,
                nobr: bootstrap.intern_ascii_folded("nobr")?,
                s: bootstrap.intern_ascii_folded("s")?,
                script: bootstrap.intern_ascii_folded("script")?,
                select: bootstrap.intern_ascii_folded("select")?,
                option: bootstrap.intern_ascii_folded("option")?,
                optgroup: bootstrap.intern_ascii_folded("optgroup")?,
                small: bootstrap.intern_ascii_folded("small")?,
                strike: bootstrap.intern_ascii_folded("strike")?,
                strong: bootstrap.intern_ascii_folded("strong")?,
                style: bootstrap.intern_ascii_folded("style")?,
                title: bootstrap.intern_ascii_folded("title")?,
                tt: bootstrap.intern_ascii_folded("tt")?,
                textarea: bootstrap.intern_ascii_folded("textarea")?,
                table: bootstrap.intern_ascii_folded("table")?,
                template: bootstrap.intern_ascii_folded("template")?,
                tbody: bootstrap.intern_ascii_folded("tbody")?,
                td: bootstrap.intern_ascii_folded("td")?,
                tfoot: bootstrap.intern_ascii_folded("tfoot")?,
                th: bootstrap.intern_ascii_folded("th")?,
                thead: bootstrap.intern_ascii_folded("thead")?,
                caption: bootstrap.intern_ascii_folded("caption")?,
                col: bootstrap.intern_ascii_folded("col")?,
                colgroup: bootstrap.intern_ascii_folded("colgroup")?,
                marquee: bootstrap.intern_ascii_folded("marquee")?,
                object: bootstrap.intern_ascii_folded("object")?,
                applet: bootstrap.intern_ascii_folded("applet")?,
                button: bootstrap.intern_ascii_folded("button")?,
                main: bootstrap.intern_ascii_folded("main")?,
                nav: bootstrap.intern_ascii_folded("nav")?,
                ol: bootstrap.intern_ascii_folded("ol")?,
                pre: bootstrap.intern_ascii_folded("pre")?,
                section: bootstrap.intern_ascii_folded("section")?,
                u: bootstrap.intern_ascii_folded("u")?,
                ul: bootstrap.intern_ascii_folded("ul")?,
                li: bootstrap.intern_ascii_folded("li")?,
                tr: bootstrap.intern_ascii_folded("tr")?,
                svg: bootstrap.intern_ascii_folded("svg")?,
                math: bootstrap.intern_ascii_folded("math")?,
                mi: bootstrap.intern_ascii_folded("mi")?,
                mo: bootstrap.intern_ascii_folded("mo")?,
                mn: bootstrap.intern_ascii_folded("mn")?,
                ms: bootstrap.intern_ascii_folded("ms")?,
                mtext: bootstrap.intern_ascii_folded("mtext")?,
                annotation_xml: bootstrap.intern_ascii_folded("annotation-xml")?,
                desc: bootstrap.intern_ascii_folded("desc")?,
                foreign_object: bootstrap.intern_exact("foreignObject")?,
            };
            #[cfg(test)]
            if let Some(stats) = stats.as_deref_mut() {
                stats.calls = bootstrap.calls;
            }
            known
        };
        #[cfg(test)]
        if let Some(stats) = stats {
            stats.unique_entries = ctx.atoms.len() - initial_len;
            stats.capacities_after_population = ctx.atoms.storage_capacities();
        }
        Ok(known)
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
    pub(in crate::html5::tree_builder) fn is_supported_implied_end_tag(
        &self,
        name: AtomId,
    ) -> bool {
        name == self.p || name == self.li || name == self.option || name == self.optgroup
    }

    #[inline]
    pub(in crate::html5::tree_builder) fn is_heading_tag(&self, name: AtomId) -> bool {
        name == self.h1
            || name == self.h2
            || name == self.h3
            || name == self.h4
            || name == self.h5
            || name == self.h6
    }

    #[inline]
    pub(in crate::html5::tree_builder) fn is_ae7_p_closing_block_start(
        &self,
        name: AtomId,
    ) -> bool {
        name == self.address
            || name == self.article
            || name == self.aside
            || name == self.blockquote
            || name == self.div
            || name == self.footer
            || name == self.fieldset
            || name == self.header
            || name == self.hr
            || name == self.li
            || name == self.main
            || name == self.nav
            || name == self.ol
            || name == self.p
            || name == self.pre
            || name == self.section
            || name == self.ul
            || self.is_heading_tag(name)
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
            || name == self.keygen
            || name == self.link
            || name == self.meta
            || name == self.param
            || name == self.source
            || name == self.track
            || name == self.wbr
    }

    /// Start tags whose implemented in-body production performs the HTML
    /// self-closing acknowledgement step.
    ///
    /// This is deliberately production-rule-specific rather than a generic
    /// `is_void_tag` check: ignored tokens and tags handled by another
    /// insertion mode must reach their own acknowledgement decision.
    #[inline]
    pub(in crate::html5::tree_builder) fn is_in_body_acknowledged_void_start_tag(
        &self,
        name: AtomId,
    ) -> bool {
        name == self.area
            || name == self.base
            || name == self.basefont
            || name == self.bgsound
            || name == self.br
            || name == self.embed
            || name == self.img
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
            select: self.select,
            button: self.button,
            ol: self.ol,
            ul: self.ul,
            math_mi: self.mi,
            math_mo: self.mo,
            math_mn: self.mn,
            math_ms: self.ms,
            math_mtext: self.mtext,
            math_annotation_xml: self.annotation_xml,
            svg_foreign_object: self.foreign_object,
            svg_desc: self.desc,
            svg_title: self.title,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        KNOWN_TAG_BOOTSTRAP_RESERVATION_BOUND, KnownTagIds, RETAINED_KNOWN_TAG_FIELD_COUNT,
    };
    use crate::html5::shared::DocumentParseContext;

    #[test]
    fn bootstrap_bound_covers_calls_unique_entries_and_collection_capacity() {
        let mut ctx = DocumentParseContext::new();
        let (_, stats) = KnownTagIds::intern_with_stats(&mut ctx).expect("known tags");

        assert_eq!(RETAINED_KNOWN_TAG_FIELD_COUNT, 88);
        assert_eq!(KNOWN_TAG_BOOTSTRAP_RESERVATION_BOUND, 195);
        assert_eq!(stats.calls, KNOWN_TAG_BOOTSTRAP_RESERVATION_BOUND);
        assert!(stats.unique_entries <= KNOWN_TAG_BOOTSTRAP_RESERVATION_BOUND);
        assert_eq!(
            stats.capacities_after_population, stats.capacities_after_reservation,
            "bootstrap population must not grow either reserved collection"
        );
        ctx.atoms.debug_validate();
    }

    #[cfg(feature = "parser-failure-injection")]
    #[test]
    fn bootstrap_reservation_failures_leave_logical_interner_contents_unchanged() {
        use crate::html5::shared::{
            ErrorPolicy, ParserFailureInjection, ParserFatalError, ParserReservationSite,
        };
        use std::num::NonZeroU64;

        for site in [
            ParserReservationSite::KnownTagAtomStorage,
            ParserReservationSite::KnownTagLookupStorage,
        ] {
            let mut ctx = DocumentParseContext::with_failure_injection(
                ErrorPolicy::default(),
                ParserFailureInjection::new(site, NonZeroU64::MIN),
            );
            let initial_len = ctx.atoms.len();
            assert_eq!(ctx.atoms.lookup_exact("html"), None);

            let error = KnownTagIds::intern(&mut ctx).expect_err("injected reservation failure");
            assert!(matches!(
                error,
                ParserFatalError::ResourceExhaustion(exhaustion)
                    if exhaustion.site() == site
            ));
            assert_eq!(ctx.atoms.len(), initial_len);
            assert_eq!(ctx.atoms.lookup_exact("html"), None);
            ctx.atoms.debug_validate();
        }
    }

    #[test]
    fn known_tag_scope_tag_view_shares_ids() {
        let mut ctx = DocumentParseContext::new();
        let known = KnownTagIds::intern(&mut ctx).expect("known tags");
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
        assert_eq!(scope.select, known.select);
        assert_eq!(scope.button, known.button);
        assert_eq!(scope.ol, known.ol);
        assert_eq!(scope.ul, known.ul);
    }

    #[test]
    fn known_tag_helpers_classify_formatting_and_marker_tags() {
        let mut ctx = DocumentParseContext::new();
        let known = KnownTagIds::intern(&mut ctx).expect("known tags");

        assert!(known.is_formatting_tag(known.b));
        assert!(known.is_formatting_tag(known.strong));
        assert!(known.is_formatting_tag(known.a));
        assert!(!known.is_formatting_tag(known.body));

        assert!(known.is_marker_tag(known.applet));
        assert!(known.is_marker_tag(known.marquee));
        assert!(known.is_marker_tag(known.object));
        assert!(!known.is_marker_tag(known.b));
    }

    #[test]
    fn known_tag_helpers_classify_supported_body_recovery_tags() {
        let mut ctx = crate::html5::shared::DocumentParseContext::new();
        let known = KnownTagIds::intern(&mut ctx).expect("known tags");

        assert!(known.is_supported_implied_end_tag(known.p));
        assert!(known.is_supported_implied_end_tag(known.li));
        assert!(known.is_supported_implied_end_tag(known.option));
        assert!(known.is_supported_implied_end_tag(known.optgroup));
        assert!(!known.is_supported_implied_end_tag(known.div));

        for name in [
            known.address,
            known.article,
            known.aside,
            known.blockquote,
            known.div,
            known.footer,
            known.fieldset,
            known.header,
            known.h1,
            known.h6,
            known.hr,
            known.li,
            known.main,
            known.nav,
            known.ol,
            known.p,
            known.pre,
            known.section,
            known.ul,
        ] {
            assert!(
                known.is_ae7_p_closing_block_start(name),
                "expected AE7 p-closing block-start tag"
            );
        }

        let span = ctx.atoms.intern_ascii_folded("span").expect("span atom");
        assert!(!known.is_ae7_p_closing_block_start(span));
        assert!(!known.is_ae7_p_closing_block_start(known.table));
    }

    #[test]
    fn known_tag_helpers_classify_ae9_void_tags() {
        let mut ctx = crate::html5::shared::DocumentParseContext::new();
        let known = KnownTagIds::intern(&mut ctx).expect("known tags");

        assert!(known.is_void_tag(known.input));
        assert!(known.is_void_tag(known.keygen));
        assert!(!known.is_void_tag(known.form));
        assert!(!known.is_void_tag(known.textarea));
        assert!(!known.is_void_tag(known.button));
        assert!(!known.is_void_tag(known.fieldset));
    }
}
