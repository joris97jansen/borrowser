#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum Invariant {
    FullEqualsChunkedDom,
    DecodesNamedEntities,
    DecodesNumericEntities,
    PreservesUtf8Text,
    ScriptRawtextVerbatim,
    AcceptsMixedAttributeSyntax,
    HasDoctypeToken,
    HasCommentToken,
    TagBoundariesStable,
    PartialEntityRemainsLiteral,
    RawtextCloseTagRecognized,
    RawtextNearMatchStaysText,
    CustomTagRecognized,
    NamespacedTagRecognized,
    AttributesParsedWithSpacing,
    EmptyAttributeValuePreserved,
    BooleanAttributePresent,
}

impl Invariant {
    pub const fn label(self) -> &'static str {
        match self {
            Self::FullEqualsChunkedDom => "full equals chunked dom",
            Self::DecodesNamedEntities => "decodes named entities",
            Self::DecodesNumericEntities => "decodes numeric entities",
            Self::PreservesUtf8Text => "preserves utf-8 text",
            Self::ScriptRawtextVerbatim => "script rawtext verbatim",
            Self::AcceptsMixedAttributeSyntax => "accepts mixed attribute syntax",
            Self::HasDoctypeToken => "has doctype token",
            Self::HasCommentToken => "has comment token",
            Self::TagBoundariesStable => "tag boundaries stable",
            Self::PartialEntityRemainsLiteral => "partial entity remains literal",
            Self::RawtextCloseTagRecognized => "rawtext close tag recognized",
            Self::RawtextNearMatchStaysText => "rawtext near match stays text",
            Self::CustomTagRecognized => "custom tag recognized",
            Self::NamespacedTagRecognized => "namespaced tag recognized",
            Self::AttributesParsedWithSpacing => "attributes parsed with spacing",
            Self::EmptyAttributeValuePreserved => "empty attribute value preserved",
            Self::BooleanAttributePresent => "boolean attribute present",
        }
    }
}

impl std::fmt::Display for Invariant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Expectation {
    MustPass,
    AllowedToFail { allowed: &'static [AllowedFailure] },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AllowedFailure {
    pub invariant: Invariant,
    pub reason: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum FixtureKind {
    Utf8,
    Entity,
    Attribute,
    Comment,
    Doctype,
    Rawtext,
    TagName,
}

#[derive(Clone, Copy, Debug)]
pub struct GoldenFixture {
    pub name: &'static str,
    pub input: &'static str,
    pub covers: &'static str,
    pub tags: &'static [&'static str],
    pub invariants: &'static [Invariant],
    pub expectation: Expectation,
    pub kind: FixtureKind,
}

const GOLDEN_CORPUS_V1: &[GoldenFixture] = &[
    GoldenFixture {
        name: "utf8_non_ascii_tags",
        input: "é<b>ï</b>ö",
        covers: "Non-ASCII text around tags.",
        tags: &["utf8", "text", "tags"],
        invariants: &[Invariant::PreservesUtf8Text, Invariant::TagBoundariesStable],
        expectation: Expectation::MustPass,
        kind: FixtureKind::Utf8,
    },
    GoldenFixture {
        name: "utf8_literal_gt_after_element",
        input: "é<em>ï</em>ö>",
        covers: "Trailing literal `>` after element close tag; must remain text.",
        tags: &["utf8", "text", "literal-gt"],
        invariants: &[Invariant::PreservesUtf8Text, Invariant::TagBoundariesStable],
        expectation: Expectation::MustPass,
        kind: FixtureKind::Utf8,
    },
    GoldenFixture {
        name: "entity_named_amp",
        input: "<p>Tom &amp; Jerry</p>",
        covers: "Named entity decoding in text.",
        tags: &["entity", "named", "text"],
        invariants: &[
            Invariant::DecodesNamedEntities,
            Invariant::FullEqualsChunkedDom,
        ],
        expectation: Expectation::MustPass,
        kind: FixtureKind::Entity,
    },
    GoldenFixture {
        name: "entity_numeric",
        input: "<p>&#123;&#x1F600;</p>",
        covers: "Numeric and hex entities.",
        tags: &["entity", "numeric", "text"],
        invariants: &[
            Invariant::DecodesNumericEntities,
            Invariant::FullEqualsChunkedDom,
        ],
        expectation: Expectation::MustPass,
        kind: FixtureKind::Entity,
    },
    GoldenFixture {
        name: "entity_partial",
        input: "<p>Fish &am chips</p>",
        covers: "Partial entity sequence without semicolon remains literal text.",
        tags: &["entity", "partial", "text"],
        invariants: &[
            Invariant::PartialEntityRemainsLiteral,
            Invariant::FullEqualsChunkedDom,
        ],
        expectation: Expectation::MustPass,
        kind: FixtureKind::Entity,
    },
    GoldenFixture {
        name: "entity_mixed_with_tags",
        input: "<p>hi &amp; <b>bye</b> &#169;</p>",
        covers: "Mixed text, entities, and tags.",
        tags: &["entity", "mixed", "text", "tags"],
        invariants: &[
            Invariant::DecodesNamedEntities,
            Invariant::DecodesNumericEntities,
            Invariant::FullEqualsChunkedDom,
        ],
        expectation: Expectation::MustPass,
        kind: FixtureKind::Entity,
    },
    GoldenFixture {
        name: "attr_quoted_unquoted",
        input: "<div class=\"a b\" data-x=1>ok</div>",
        covers: "Quoted and unquoted attribute values.",
        tags: &["attribute", "quoted", "unquoted"],
        invariants: &[
            Invariant::AcceptsMixedAttributeSyntax,
            Invariant::FullEqualsChunkedDom,
        ],
        expectation: Expectation::MustPass,
        kind: FixtureKind::Attribute,
    },
    GoldenFixture {
        name: "attr_quote_variants",
        input: "<input value='a b' title=\"c d\">",
        covers: "Single and double quoted attribute values.",
        tags: &["attribute", "quoted", "single-quote", "double-quote"],
        invariants: &[
            Invariant::AcceptsMixedAttributeSyntax,
            Invariant::FullEqualsChunkedDom,
        ],
        expectation: Expectation::MustPass,
        kind: FixtureKind::Attribute,
    },
    GoldenFixture {
        name: "attr_whitespace_variations",
        input: "<div   id =  \"a\"   class=foo  >ok</div>",
        covers: "Whitespace variations around attributes.",
        tags: &["attribute", "whitespace", "spacing"],
        invariants: &[
            Invariant::AcceptsMixedAttributeSyntax,
            Invariant::AttributesParsedWithSpacing,
        ],
        expectation: Expectation::MustPass,
        kind: FixtureKind::Attribute,
    },
    GoldenFixture {
        name: "attr_boolean_empty",
        input: "<input disabled required data-empty=\"\">",
        covers: "Boolean and empty attributes.",
        tags: &["attribute", "boolean", "empty"],
        invariants: &[
            Invariant::BooleanAttributePresent,
            Invariant::EmptyAttributeValuePreserved,
        ],
        expectation: Expectation::MustPass,
        kind: FixtureKind::Attribute,
    },
    GoldenFixture {
        name: "comment_basic",
        input: "<!--split--><p>ok</p>",
        covers: "Comment start/end markers.",
        tags: &["comment", "markers"],
        invariants: &[Invariant::HasCommentToken, Invariant::FullEqualsChunkedDom],
        expectation: Expectation::MustPass,
        kind: FixtureKind::Comment,
    },
    GoldenFixture {
        name: "comment_terminator_edge",
        input: "text<!--x-->tail",
        covers: "Comment terminator boundary with surrounding text.",
        tags: &["comment", "terminator", "text"],
        invariants: &[Invariant::HasCommentToken, Invariant::FullEqualsChunkedDom],
        expectation: Expectation::MustPass,
        kind: FixtureKind::Comment,
    },
    GoldenFixture {
        name: "doctype_mixed_case",
        input: "<!DoCtYpE html><p>ok</p>",
        covers: "Mixed-case doctype token.",
        tags: &["doctype", "case"],
        invariants: &[Invariant::HasDoctypeToken, Invariant::FullEqualsChunkedDom],
        expectation: Expectation::MustPass,
        kind: FixtureKind::Doctype,
    },
    GoldenFixture {
        name: "rawtext_script_many_lt",
        input: "<script>if (a < b && c << 1) {}</script>",
        covers: "Rawtext containing many < characters.",
        tags: &["rawtext", "script", "lt"],
        invariants: &[
            Invariant::ScriptRawtextVerbatim,
            Invariant::FullEqualsChunkedDom,
        ],
        expectation: Expectation::AllowedToFail {
            allowed: &[AllowedFailure {
                invariant: Invariant::ScriptRawtextVerbatim,
                reason: "rawtext handling is still partial; keep fixture for future parity",
            }],
        },
        kind: FixtureKind::Rawtext,
    },
    GoldenFixture {
        name: "rawtext_close_tag",
        input: "<script>hi</script>",
        covers: "Rawtext close tag present for split tests.",
        tags: &["rawtext", "script", "close-tag"],
        invariants: &[
            Invariant::RawtextCloseTagRecognized,
            Invariant::FullEqualsChunkedDom,
        ],
        expectation: Expectation::AllowedToFail {
            allowed: &[AllowedFailure {
                invariant: Invariant::RawtextCloseTagRecognized,
                reason: "rawtext close tag splitting may be incomplete",
            }],
        },
        kind: FixtureKind::Rawtext,
    },
    GoldenFixture {
        name: "rawtext_near_match",
        input: "<script>var s = \"</scriptx>\";</script>",
        covers: "Near-match rawtext end tag inside body.",
        tags: &["rawtext", "script", "near-match"],
        invariants: &[
            Invariant::RawtextNearMatchStaysText,
            Invariant::FullEqualsChunkedDom,
        ],
        expectation: Expectation::AllowedToFail {
            allowed: &[AllowedFailure {
                invariant: Invariant::RawtextNearMatchStaysText,
                reason: "rawtext near-match rules still under development",
            }],
        },
        kind: FixtureKind::Rawtext,
    },
    GoldenFixture {
        name: "tag_custom_element",
        input: "<my-component data-x=1></my-component>",
        covers: "Custom element tag names.",
        tags: &["tag-name", "custom-element"],
        invariants: &[
            Invariant::CustomTagRecognized,
            Invariant::FullEqualsChunkedDom,
        ],
        expectation: Expectation::MustPass,
        kind: FixtureKind::TagName,
    },
    GoldenFixture {
        name: "tag_namespace",
        input: "<svg:rect width=\"1\" height=\"1\"></svg:rect>",
        covers: "Namespaced tag name with colon.",
        tags: &["tag-name", "namespace", "colon"],
        invariants: &[
            Invariant::NamespacedTagRecognized,
            Invariant::FullEqualsChunkedDom,
        ],
        expectation: Expectation::AllowedToFail {
            allowed: &[AllowedFailure {
                invariant: Invariant::NamespacedTagRecognized,
                reason: "tag-name charset rules for namespaces are incomplete",
            }],
        },
        kind: FixtureKind::TagName,
    },
];

pub fn fixtures() -> &'static [GoldenFixture] {
    GOLDEN_CORPUS_V1
}

#[cfg(test)]
mod tests {
    use super::{AllowedFailure, GoldenFixture, fixtures};
    use crate::dom_snapshot::{DomSnapshotOptions, compare_dom};
    use crate::test_harness::{
        ChunkPlan, FuzzMode, ShrinkStats, deterministic_chunk_plans, random_chunk_plan,
        run_chunked_with_tokens, run_full, shrink_chunk_plan_with_stats,
    };
    use crate::{Node, Token, TokenStream};
    use std::collections::{BTreeMap, HashMap, HashSet};

    #[test]
    fn golden_corpus_has_metadata() {
        let corpus = fixtures();
        assert!(!corpus.is_empty(), "expected at least one golden fixture");
        let mut names: HashSet<&'static str> = HashSet::new();
        let mut kind_invariants = HashSet::new();
        for &GoldenFixture {
            name,
            input,
            covers,
            tags,
            invariants,
            expectation,
            kind,
        } in corpus
        {
            assert!(!name.trim().is_empty(), "fixture name must be non-empty");
            assert!(!input.trim().is_empty(), "fixture input must be non-empty");
            assert!(
                !covers.trim().is_empty(),
                "fixture covers must be non-empty"
            );
            assert!(!tags.is_empty(), "fixture tags must be non-empty: {name}");
            for &tag in tags {
                assert!(
                    !tag.trim().is_empty(),
                    "fixture tag must be non-empty: {name}"
                );
            }
            assert!(names.insert(name), "fixture name must be unique: {name}");
            assert!(
                !invariants.is_empty(),
                "fixture invariants must be non-empty: {name}"
            );
            let mut inv_set = HashSet::new();
            for inv in invariants.iter().copied() {
                assert!(
                    inv_set.insert(inv),
                    "duplicate invariant on fixture: {name}: {inv}"
                );
            }
            assert!(
                unique_kind_invariants(kind, invariants, tags, &mut kind_invariants),
                "fixture kind+invariants+tags must be unique: {name}"
            );
            validate_allowed(expectation, invariants, name);
        }
    }

    fn unique_kind_invariants(
        kind: super::FixtureKind,
        invariants: &[super::Invariant],
        tags: &[&'static str],
        seen: &mut HashSet<(super::FixtureKind, Vec<super::Invariant>, Vec<&'static str>)>,
    ) -> bool {
        let mut invs = invariants.to_vec();
        invs.sort_unstable();
        let mut tag_list = tags.to_vec();
        tag_list.sort_unstable();
        seen.insert((kind, invs, tag_list))
    }

    fn validate_allowed(
        expectation: super::Expectation,
        invariants: &[super::Invariant],
        name: &str,
    ) {
        if let super::Expectation::AllowedToFail { allowed } = expectation {
            assert!(
                !allowed.is_empty(),
                "fixture allowed-to-fail must declare allowed invariants: {name}"
            );
            for AllowedFailure { invariant, reason } in allowed {
                assert!(
                    !reason.trim().is_empty(),
                    "fixture allowed-to-fail must have a reason: {name}"
                );
                assert!(
                    invariants.contains(invariant),
                    "allowed invariant must be listed on fixture: {name}"
                );
            }
        }
    }

    #[test]
    fn golden_corpus_v1_runs_across_deterministic_chunk_plans() {
        let mut failures = Vec::new();
        let mut xfails = Vec::new();
        let mut xfail_invariants: HashMap<super::Invariant, usize> = HashMap::new();
        let mut xfail_kinds: HashMap<super::FixtureKind, usize> = HashMap::new();
        for fixture in fixtures() {
            let plans = deterministic_chunk_plans(fixture.input);
            let run = run_golden_fixture(fixture, &plans);
            failures.extend(run.failures);
            for entry in run.xfails {
                *xfail_invariants.entry(entry.invariant).or_insert(0) += 1;
                *xfail_kinds.entry(fixture.kind).or_insert(0) += 1;
                xfails.push(entry.message);
            }
        }
        if !xfails.is_empty() {
            eprintln!("XFAIL summary ({} total):", xfails.len());
            for (inv, count) in xfail_invariants {
                eprintln!("  {inv}: {count}");
            }
            for (kind, count) in xfail_kinds {
                eprintln!("  {:?}: {count}", kind);
            }
        }
        if !failures.is_empty() {
            let report = failures.join("\n");
            panic!("golden corpus failures:\n{report}");
        }
    }

    #[test]
    fn golden_corpus_v1_runs_across_random_chunk_plans() {
        let seeds_per_fixture = fuzz_seed_count();
        let base_seed = fuzz_seed_base();
        let fixture_filter = fuzz_fixture_filter();
        let fuzz_mode = fuzz_mode();
        let verbose = std::env::var("BORROWSER_FUZZ_VERBOSE").is_ok();
        let strict_xpass = std::env::var("BORROWSER_STRICT_XPASS").is_ok();
        let mut failures = Vec::new();
        let mut primary_repro = None;
        let mut matched = 0usize;
        let mut xfail_invariants: HashMap<super::Invariant, usize> = HashMap::new();
        for (fixture_index, fixture) in fixtures().iter().enumerate() {
            if !fixture_matches_filter(fixture.name, fixture_filter.as_deref()) {
                continue;
            }
            matched += 1;
            let full_dom = run_full(fixture.input);
            let fixture_seed = derive_seed(base_seed, fixture.name, fixture_index as u64);
            for i in 0..seeds_per_fixture {
                let seed = fixture_seed.wrapping_add(i as u64);
                let fuzz_plan = random_chunk_plan(fixture.input, seed, fuzz_mode);
                let (chunked_dom, chunked_tokens) =
                    run_chunked_with_tokens(fixture.input, &fuzz_plan.plan);
                for &inv in fixture.invariants {
                    let result =
                        check_invariant(fixture, inv, &full_dom, &chunked_dom, &chunked_tokens);
                    if let Err(message) = result {
                        if let Some(reason) = is_allowed_to_fail(fixture, inv) {
                            *xfail_invariants.entry(inv).or_insert(0) += 1;
                            if verbose {
                                eprintln!(
                                    "XFAIL: {} [{:?}] :: seed=0x{seed:016x} :: {} :: {} :: {message} :: {} ({reason})",
                                    fixture.name,
                                    fixture.kind,
                                    fuzz_plan.plan,
                                    inv,
                                    fuzz_plan.summary
                                );
                            }
                        } else {
                            let repro = format!(
                                "repro: BORROWSER_FUZZ_SEED=0x{seed:016x} BORROWSER_FUZZ_FIXTURE={} BORROWSER_FUZZ_SEEDS=1 BORROWSER_FUZZ_MODE={} cargo test -p html golden_corpus_v1_runs_across_random_chunk_plans -- --nocapture",
                                fixture.name,
                                fuzz_mode_label(fuzz_mode)
                            );
                            let (minimized, stats) = minimize_plan_for_failure(
                                fixture,
                                inv,
                                &full_dom,
                                fixture.input,
                                &fuzz_plan.plan,
                            );
                            if primary_repro.is_none() {
                                primary_repro = Some(repro.clone());
                            }
                            failures.push(format!(
                                "{} [{:?}] :: seed=0x{seed:016x} :: {} :: {} :: {message} :: {} :: minimized={} :: shrink(orig_boundaries={} orig_chunks={} min_boundaries={} min_chunks={} checks={} policy_upgraded={} budget_exhausted={}) :: {repro}",
                                fixture.name,
                                fixture.kind,
                                fuzz_plan.plan,
                                inv,
                                fuzz_plan.summary,
                                minimized,
                                stats.original_boundaries,
                                stats.original_chunks,
                                stats.minimized_boundaries,
                                stats.minimized_chunks,
                                stats.checks,
                                stats.policy_upgraded,
                                stats.budget_exhausted
                            ));
                        }
                    } else if let Some(reason) = is_allowed_to_fail(fixture, inv) {
                        if strict_xpass {
                            let repro = format!(
                                "repro: BORROWSER_FUZZ_SEED=0x{seed:016x} BORROWSER_FUZZ_FIXTURE={} BORROWSER_FUZZ_SEEDS=1 BORROWSER_FUZZ_MODE={} cargo test -p html golden_corpus_v1_runs_across_random_chunk_plans -- --nocapture",
                                fixture.name,
                                fuzz_mode_label(fuzz_mode)
                            );
                            if primary_repro.is_none() {
                                primary_repro = Some(repro.clone());
                            }
                            failures.push(format!(
                                "{} [{:?}] :: seed=0x{seed:016x} :: {} :: {} :: XPASS (allowed to fail: {reason}) :: {} :: {repro}",
                                fixture.name, fixture.kind, fuzz_plan.plan, inv, fuzz_plan.summary
                            ));
                        } else if verbose {
                            eprintln!(
                                "XPASS: {} [{:?}] :: seed=0x{seed:016x} :: {} :: {} :: {reason} :: {}",
                                fixture.name, fixture.kind, fuzz_plan.plan, inv, fuzz_plan.summary
                            );
                        }
                    }
                }
            }
        }
        if let Some(filter) = fixture_filter.as_deref()
            && matched == 0
        {
            panic!("no fixtures matched BORROWSER_FUZZ_FIXTURE={filter}");
        }
        if !xfail_invariants.is_empty() {
            eprintln!(
                "XFAIL summary ({} total):",
                xfail_invariants.values().sum::<usize>()
            );
            for (inv, count) in xfail_invariants {
                eprintln!("  {inv}: {count}");
            }
        }
        if !failures.is_empty() {
            let report = failures.join("\n");
            if let Some(repro) = primary_repro {
                panic!("golden corpus random-chunk failures:\n{repro}\n{report}");
            }
            panic!("golden corpus random-chunk failures:\n{report}");
        }
    }

    struct FixtureRun {
        failures: Vec<String>,
        xfails: Vec<XfailEntry>,
    }

    fn run_golden_fixture(fixture: &GoldenFixture, plans: &[ChunkPlan]) -> FixtureRun {
        let mut failures = Vec::new();
        let mut xfails = Vec::new();
        let strict_xpass = std::env::var("BORROWSER_STRICT_XPASS").is_ok();
        let full_dom = run_full(fixture.input);
        let tags_label = format!("[{}]", fixture.tags.join(","));
        for plan in plans {
            let (chunked_dom, chunked_tokens) = run_chunked_with_tokens(fixture.input, plan);
            for &inv in fixture.invariants {
                let result =
                    check_invariant(fixture, inv, &full_dom, &chunked_dom, &chunked_tokens);
                match result {
                    Ok(()) => {
                        if let Some(reason) = is_allowed_to_fail(fixture, inv) {
                            if strict_xpass {
                                failures.push(format!(
                                    "{} {} :: {} :: {} :: XPASS (allowed to fail: {reason})",
                                    fixture.name, tags_label, plan, inv
                                ));
                            } else {
                                eprintln!(
                                    "XPASS: {} {} :: {} :: {} :: {reason}",
                                    fixture.name, tags_label, plan, inv
                                );
                            }
                        }
                    }
                    Err(message) => {
                        if let Some(reason) = is_allowed_to_fail(fixture, inv) {
                            xfails.push(XfailEntry {
                                invariant: inv,
                                message: format!(
                                    "XFAIL: {} {} :: {} :: {} :: {message} ({reason})",
                                    fixture.name, tags_label, plan, inv
                                ),
                            });
                        } else {
                            failures.push(format!(
                                "{} {} :: {} :: {} :: {message}",
                                fixture.name, tags_label, plan, inv
                            ));
                        }
                    }
                }
            }
        }
        FixtureRun { failures, xfails }
    }

    fn minimize_plan_for_failure(
        fixture: &GoldenFixture,
        inv: super::Invariant,
        full_dom: &Node,
        input: &str,
        plan: &ChunkPlan,
    ) -> (ChunkPlan, ShrinkStats) {
        shrink_chunk_plan_with_stats(input, plan, |candidate| {
            let (chunked_dom, chunked_tokens) = run_chunked_with_tokens(input, candidate);
            check_invariant(fixture, inv, full_dom, &chunked_dom, &chunked_tokens).is_err()
        })
    }

    fn fuzz_seed_count() -> usize {
        if let Ok(value) = std::env::var("BORROWSER_FUZZ_SEEDS")
            && let Ok(parsed) = value.parse::<usize>()
            && parsed > 0
        {
            return parsed;
        }
        if std::env::var("CI").is_ok() { 50 } else { 200 }
    }

    fn fuzz_seed_base() -> u64 {
        if let Ok(value) = std::env::var("BORROWSER_FUZZ_SEED") {
            if let Ok(parsed) = u64::from_str_radix(value.trim_start_matches("0x"), 16) {
                return parsed;
            }
            if let Ok(parsed) = value.parse::<u64>() {
                return parsed;
            }
        }
        0x6c8e9cf570932bd5
    }

    fn derive_seed(base: u64, name: &str, salt: u64) -> u64 {
        let mut hash = 0xcbf29ce484222325u64;
        for byte in name.as_bytes() {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        base ^ hash ^ salt.wrapping_mul(0x9e3779b97f4a7c15)
    }

    fn fuzz_fixture_filter() -> Option<String> {
        std::env::var("BORROWSER_FUZZ_FIXTURE")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    }

    fn fixture_matches_filter(name: &str, filter: Option<&str>) -> bool {
        let Some(filter) = filter else {
            return true;
        };
        if !filter.contains('*') {
            return name == filter || name.starts_with(filter);
        }
        let mut remainder = name;
        for part in filter.split('*').filter(|part| !part.is_empty()) {
            if let Some(idx) = remainder.find(part) {
                remainder = &remainder[idx + part.len()..];
            } else {
                return false;
            }
        }
        true
    }

    fn fuzz_mode() -> FuzzMode {
        let value = std::env::var("BORROWSER_FUZZ_MODE").unwrap_or_else(|_| "mixed".to_string());
        match value.as_str() {
            "sizes" => FuzzMode::Sizes,
            "boundaries" => FuzzMode::Boundaries,
            "semantic" => FuzzMode::Semantic,
            "mixed" => FuzzMode::Mixed,
            _ => panic!("unknown BORROWSER_FUZZ_MODE={value}"),
        }
    }

    fn fuzz_mode_label(mode: FuzzMode) -> &'static str {
        match mode {
            FuzzMode::Sizes => "sizes",
            FuzzMode::Boundaries => "boundaries",
            FuzzMode::Semantic => "semantic",
            FuzzMode::Mixed => "mixed",
        }
    }

    struct XfailEntry {
        invariant: super::Invariant,
        message: String,
    }

    fn check_invariant(
        fixture: &GoldenFixture,
        invariant: super::Invariant,
        full_dom: &Node,
        chunked_dom: &Node,
        chunked_tokens: &TokenStream,
    ) -> Result<(), String> {
        match invariant {
            super::Invariant::FullEqualsChunkedDom => {
                compare_dom(full_dom, chunked_dom, DomSnapshotOptions::default())
                    .map_err(|err| err.to_string())
            }
            super::Invariant::HasDoctypeToken => {
                let full_has_doctype = match full_dom {
                    Node::Document { doctype, .. } => doctype.is_some(),
                    _ => false,
                };
                let chunked_has_doctype = match chunked_dom {
                    Node::Document { doctype, .. } => doctype.is_some(),
                    _ => false,
                };
                if full_has_doctype == chunked_has_doctype {
                    Ok(())
                } else {
                    Err(format!(
                        "doctype parity mismatch: full={full_has_doctype} chunked={chunked_has_doctype}"
                    ))
                }
            }
            super::Invariant::HasCommentToken => {
                let full_has_comment = has_comment(full_dom);
                let chunked_has_comment = has_comment(chunked_dom);
                if full_has_comment == chunked_has_comment {
                    Ok(())
                } else {
                    Err(format!(
                        "comment parity mismatch: full={full_has_comment} chunked={chunked_has_comment}"
                    ))
                }
            }
            super::Invariant::PreservesUtf8Text => {
                let expected_chars: Vec<char> =
                    fixture.input.chars().filter(|c| !c.is_ascii()).collect();
                if expected_chars.is_empty() {
                    return Ok(());
                }
                let full_text = collect_text(full_dom);
                let chunked_text = collect_text(chunked_dom);
                for ch in expected_chars {
                    let full_has = full_text.contains(ch);
                    let chunked_has = chunked_text.contains(ch);
                    if full_has != chunked_has {
                        return Err(format!(
                            "UTF-8 text parity mismatch for {ch}: full={full_has} chunked={chunked_has}"
                        ));
                    }
                }
                Ok(())
            }
            super::Invariant::DecodesNamedEntities => {
                let full_text = collect_text(full_dom);
                let chunked_text = collect_text(chunked_dom);
                if fixture.input.contains("&amp;") {
                    let full_ok = full_text.contains('&') && !full_text.contains("&amp;");
                    let chunked_ok = chunked_text.contains('&') && !chunked_text.contains("&amp;");
                    if full_ok != chunked_ok {
                        return Err(format!(
                            "named entity parity mismatch: full={full_ok} chunked={chunked_ok}"
                        ));
                    }
                }
                Ok(())
            }
            super::Invariant::DecodesNumericEntities => {
                let full_text = collect_text(full_dom);
                let chunked_text = collect_text(chunked_dom);
                if fixture.input.contains("&#123;") {
                    let full_ok = full_text.contains('{');
                    let chunked_ok = chunked_text.contains('{');
                    if full_ok != chunked_ok {
                        return Err(format!(
                            "numeric entity parity mismatch for &#123;: full={full_ok} chunked={chunked_ok}"
                        ));
                    }
                }
                if fixture.input.contains("&#169;") {
                    let full_ok = full_text.contains('©');
                    let chunked_ok = chunked_text.contains('©');
                    if full_ok != chunked_ok {
                        return Err(format!(
                            "numeric entity parity mismatch for &#169;: full={full_ok} chunked={chunked_ok}"
                        ));
                    }
                }
                if fixture.input.contains("&#x1F600;") {
                    let full_ok = full_text.contains('😀');
                    let chunked_ok = chunked_text.contains('😀');
                    if full_ok != chunked_ok {
                        return Err(format!(
                            "hex entity parity mismatch for &#x1F600;: full={full_ok} chunked={chunked_ok}"
                        ));
                    }
                }
                Ok(())
            }
            super::Invariant::PartialEntityRemainsLiteral => {
                let full_text = collect_text(full_dom);
                let chunked_text = collect_text(chunked_dom);
                let full_ok = full_text.contains("&am");
                let chunked_ok = chunked_text.contains("&am");
                if full_ok == chunked_ok {
                    Ok(())
                } else {
                    Err(format!(
                        "partial entity parity mismatch: full={full_ok} chunked={chunked_ok}"
                    ))
                }
            }
            super::Invariant::AcceptsMixedAttributeSyntax => {
                // Token parity is covered elsewhere; this is a focused chunked-token assertion.
                check_token_attributes(fixture.name, chunked_tokens)
            }
            super::Invariant::AttributesParsedWithSpacing => {
                // Token parity is covered elsewhere; this is a focused chunked-token assertion.
                check_token_attributes(fixture.name, chunked_tokens)
            }
            super::Invariant::BooleanAttributePresent => {
                // Token parity is covered elsewhere; this is a focused chunked-token assertion.
                check_token_attributes(fixture.name, chunked_tokens)
            }
            super::Invariant::EmptyAttributeValuePreserved => {
                // Token parity is covered elsewhere; this is a focused chunked-token assertion.
                check_token_attributes(fixture.name, chunked_tokens)
            }
            super::Invariant::TagBoundariesStable => {
                // Tag name counts are compared, not full tree shape. Use FullEqualsChunkedDom for structure.
                let expected = element_name_counts(full_dom);
                let actual = element_name_counts(chunked_dom);
                if expected == actual {
                    Ok(())
                } else {
                    Err(format!(
                        "tag name parity mismatch: full={expected:?} chunked={actual:?}"
                    ))
                }
            }
            super::Invariant::CustomTagRecognized => {
                let full_has = find_element(full_dom, "my-component").is_some();
                let chunked_has = find_element(chunked_dom, "my-component").is_some();
                if full_has == chunked_has {
                    Ok(())
                } else {
                    Err(format!(
                        "custom tag parity mismatch: full={full_has} chunked={chunked_has}"
                    ))
                }
            }
            super::Invariant::NamespacedTagRecognized => {
                let full_has = find_element(full_dom, "svg:rect").is_some();
                let chunked_has = find_element(chunked_dom, "svg:rect").is_some();
                if full_has == chunked_has {
                    Ok(())
                } else {
                    Err(format!(
                        "namespaced tag parity mismatch: full={full_has} chunked={chunked_has}"
                    ))
                }
            }
            super::Invariant::ScriptRawtextVerbatim => {
                let expected = script_body_from_input(fixture.input)
                    .ok_or_else(|| "expected <script> body in fixture input".to_string())?;
                let full_actual = script_text(full_dom)
                    .ok_or_else(|| "expected <script> element in full DOM".to_string())?;
                let chunked_actual = script_text(chunked_dom)
                    .ok_or_else(|| "expected <script> element in chunked DOM".to_string())?;
                let full_ok = full_actual == expected;
                let chunked_ok = chunked_actual == expected;
                if full_ok == chunked_ok {
                    Ok(())
                } else {
                    Err(format!(
                        "rawtext parity mismatch: full={full_ok} chunked={chunked_ok}"
                    ))
                }
            }
            super::Invariant::RawtextCloseTagRecognized => {
                let full_text = script_text(full_dom)
                    .ok_or_else(|| "expected <script> element in full DOM".to_string())?;
                let chunked_text = script_text(chunked_dom)
                    .ok_or_else(|| "expected <script> element in chunked DOM".to_string())?;
                let full_ok = full_text == "hi";
                let chunked_ok = chunked_text == "hi";
                if full_ok == chunked_ok {
                    Ok(())
                } else {
                    Err(format!(
                        "rawtext close-tag parity mismatch: full={full_ok} chunked={chunked_ok}"
                    ))
                }
            }
            super::Invariant::RawtextNearMatchStaysText => {
                let full_text = script_text(full_dom)
                    .ok_or_else(|| "expected <script> element in full DOM".to_string())?;
                let chunked_text = script_text(chunked_dom)
                    .ok_or_else(|| "expected <script> element in chunked DOM".to_string())?;
                let full_ok = full_text.contains("</scriptx>");
                let chunked_ok = chunked_text.contains("</scriptx>");
                if full_ok == chunked_ok {
                    Ok(())
                } else {
                    Err(format!(
                        "rawtext near-match parity mismatch: full={full_ok} chunked={chunked_ok}"
                    ))
                }
            }
        }
    }

    fn is_allowed_to_fail(
        fixture: &GoldenFixture,
        invariant: super::Invariant,
    ) -> Option<&'static str> {
        match fixture.expectation {
            super::Expectation::MustPass => None,
            super::Expectation::AllowedToFail { allowed } => allowed
                .iter()
                .find(|entry| entry.invariant == invariant)
                .map(|entry| entry.reason),
        }
    }

    fn collect_text(node: &Node) -> String {
        let mut out = String::new();
        collect_text_into(node, &mut out);
        out
    }

    fn collect_text_into(node: &Node, out: &mut String) {
        match node {
            Node::Document { children, .. } | Node::Element { children, .. } => {
                for child in children {
                    collect_text_into(child, out);
                }
            }
            Node::Text { text, .. } => out.push_str(text),
            Node::Comment { .. } => {}
        }
    }

    fn has_comment(node: &Node) -> bool {
        match node {
            Node::Comment { .. } => true,
            Node::Document { children, .. } | Node::Element { children, .. } => {
                children.iter().any(has_comment)
            }
            Node::Text { .. } => false,
        }
    }

    fn find_element<'a>(node: &'a Node, name: &str) -> Option<&'a Node> {
        match node {
            Node::Element { name: tag, .. } => {
                crate::types::debug_assert_lowercase_atom(tag, "golden find_element tag");
                if tag.as_ref() == name {
                    Some(node)
                } else {
                    None
                }
            }
            Node::Document { children, .. } => {
                children.iter().find_map(|child| find_element(child, name))
            }
            _ => None,
        }
    }

    fn script_text(node: &Node) -> Option<String> {
        let script = find_element(node, "script")?;
        let Node::Element { children, .. } = script else {
            return None;
        };
        let mut out = String::new();
        for child in children {
            collect_text_into(child, &mut out);
        }
        Some(out)
    }

    fn script_body_from_input(input: &str) -> Option<&str> {
        let start = input.find("<script>")?;
        let end = input.rfind("</script>")?;
        let start = start + "<script>".len();
        if start > end {
            return None;
        }
        Some(&input[start..end])
    }

    fn element_name_counts(node: &Node) -> BTreeMap<String, usize> {
        let mut out = BTreeMap::new();
        collect_element_names(node, &mut out);
        out
    }

    fn collect_element_names(node: &Node, out: &mut BTreeMap<String, usize>) {
        match node {
            Node::Element { name, children, .. } => {
                crate::types::debug_assert_lowercase_atom(name, "golden element name");
                let key = name.to_string();
                *out.entry(key).or_insert(0) += 1;
                for child in children {
                    collect_element_names(child, out);
                }
            }
            Node::Document { children, .. } => {
                for child in children {
                    collect_element_names(child, out);
                }
            }
            Node::Text { .. } | Node::Comment { .. } => {}
        }
    }

    fn check_token_attributes(fixture_name: &str, stream: &TokenStream) -> Result<(), String> {
        type StartTagAttrs<'a> = (
            &'a str,
            &'a [(crate::AtomId, Option<crate::AttributeValue>)],
        );
        let atoms = stream.atoms();
        let start_tags: Vec<StartTagAttrs<'_>> = stream
            .tokens()
            .iter()
            .filter_map(|token| {
                if let Token::StartTag {
                    name, attributes, ..
                } = token
                {
                    Some((atoms.resolve(*name), attributes.as_slice()))
                } else {
                    None
                }
            })
            .collect();
        match fixture_name {
            "attr_quoted_unquoted" => {
                let (_, attrs) = start_tags
                    .iter()
                    .find(|(tag, _)| *tag == "div")
                    .ok_or_else(|| "expected <div> start tag".to_string())?;
                let class = find_attr(stream, atoms, attrs, "class");
                let data_x = find_attr(stream, atoms, attrs, "data-x");
                if class == Some("a b") && data_x == Some("1") {
                    Ok(())
                } else {
                    Err(format!(
                        "expected class=\"a b\" and data-x=\"1\", got class={class:?} data-x={data_x:?}"
                    ))
                }
            }
            "attr_quote_variants" => {
                let (_, attrs) = start_tags
                    .iter()
                    .find(|(tag, _)| *tag == "input")
                    .ok_or_else(|| "expected <input> start tag".to_string())?;
                let value = find_attr(stream, atoms, attrs, "value");
                let title = find_attr(stream, atoms, attrs, "title");
                if value == Some("a b") && title == Some("c d") {
                    Ok(())
                } else {
                    Err("expected value=\"a b\" and title=\"c d\"".to_string())
                }
            }
            "attr_whitespace_variations" => {
                let (_, attrs) = start_tags
                    .iter()
                    .find(|(tag, _)| *tag == "div")
                    .ok_or_else(|| "expected <div> start tag".to_string())?;
                let id = find_attr(stream, atoms, attrs, "id");
                let class = find_attr(stream, atoms, attrs, "class");
                if id == Some("a") && class == Some("foo") {
                    Ok(())
                } else {
                    Err("expected id=\"a\" and class=\"foo\"".to_string())
                }
            }
            "attr_boolean_empty" => {
                let (_, attrs) = start_tags
                    .iter()
                    .find(|(tag, _)| *tag == "input")
                    .ok_or_else(|| "expected <input> start tag".to_string())?;
                let disabled_present = has_attr(atoms, attrs, "disabled");
                let required_present = has_attr(atoms, attrs, "required");
                let data_empty = find_attr(stream, atoms, attrs, "data-empty");
                if disabled_present && required_present && data_empty == Some("") {
                    Ok(())
                } else {
                    Err("expected disabled+required boolean attrs and data-empty=\"\"".to_string())
                }
            }
            _ => Err(format!(
                "attribute expectations not defined for fixture: {fixture_name}"
            )),
        }
    }

    fn find_attr<'a>(
        stream: &'a TokenStream,
        atoms: &'a crate::AtomTable,
        attrs: &'a [(crate::AtomId, Option<crate::AttributeValue>)],
        name: &str,
    ) -> Option<&'a str> {
        attrs.iter().find_map(|(key, value)| {
            let key_name = atoms.resolve(*key);
            crate::types::debug_assert_lowercase_atom(key_name, "golden attribute name");
            if key_name == name {
                Some(value.as_ref().map(|v| stream.attr_value(v)).unwrap_or(""))
            } else {
                None
            }
        })
    }

    fn has_attr(
        atoms: &crate::AtomTable,
        attrs: &[(crate::AtomId, Option<crate::AttributeValue>)],
        name: &str,
    ) -> bool {
        attrs.iter().any(|(key, _)| {
            let key_name = atoms.resolve(*key);
            crate::types::debug_assert_lowercase_atom(key_name, "golden attribute name");
            key_name == name
        })
    }
}
