//! Bounded formatting of the same semantic inventory; no source strings escape.
use super::*;
use addressing::{ResourceCapability, ResourceOperation};
use std::fmt::{self, Write};

#[derive(Clone, Copy)]
pub(super) struct FormatLimits {
    pub bytes: usize,
    pub footer: usize,
    pub records: usize,
    pub diagnostics: usize,
    pub scratch: usize,
}
impl Default for FormatLimits {
    fn default() -> Self {
        Self {
            bytes: 8192,
            footer: 512,
            records: 16,
            diagnostics: 16,
            scratch: 1024,
        }
    }
}
pub(crate) struct Snapshot {
    output: String,
    limits: FormatLimits,
    byte_limit: bool,
    scratch_limit: bool,
}
enum InventoryTotals {
    NotApplicable,
    Unavailable,
    Exact {
        records: usize,
        diagnostics: Option<usize>,
    },
}
impl Snapshot {
    pub(crate) fn new() -> Result<Self, StylesheetDiscoverySnapshotError> {
        Self::with_limits(FormatLimits::default())
    }
    pub(super) fn with_limits(
        limits: FormatLimits,
    ) -> Result<Self, StylesheetDiscoverySnapshotError> {
        let mut output = String::new();
        output
            .try_reserve_exact(limits.bytes)
            .map_err(|_| StylesheetDiscoverySnapshotError::OutputAllocation)?;
        Ok(Self {
            output,
            limits,
            byte_limit: false,
            scratch_limit: false,
        })
    }
    fn append(&mut self, args: fmt::Arguments<'_>) -> bool {
        let mut scratch = Scratch {
            bytes: [0; 1024],
            len: 0,
            maximum: self.limits.scratch.min(1024),
        };
        if fmt::write(&mut scratch, args).is_err() {
            self.scratch_limit = true;
            return false;
        }
        if scratch.len
            > self
                .limits
                .bytes
                .saturating_sub(self.limits.footer)
                .saturating_sub(self.output.len())
        {
            self.byte_limit = true;
            return false;
        }
        self.output
            .push_str(std::str::from_utf8(&scratch.bytes[..scratch.len]).expect("formatted UTF-8"));
        true
    }
    pub(crate) fn finish(
        mut self,
        scope: fmt::Arguments<'_>,
        result: Result<Option<StylesheetDiscovery<'_>>, StylesheetDiscoveryError>,
    ) -> String {
        self.append(format_args!("StylesheetDiscovery {{\n  {scope}\n"));
        let mut shown = 0usize;
        let mut diag_shown = 0usize;
        let mut record_limit = false;
        let mut diagnostic_limit = false;
        let (availability, semantic_complete, totals) = match result {
            Ok(None) => {
                self.append(format_args!("  availability: no-committed-document\n"));
                (
                    "unavailable",
                    "not-applicable",
                    InventoryTotals::NotApplicable,
                )
            }
            Err(error) => {
                self.append(format_args!("  availability: failed error={error:?}\n"));
                ("failed", "false", InventoryTotals::Unavailable)
            }
            Ok(Some(inventory)) => {
                let records_total = inventory.records().len();
                let diagnostics_total = inventory
                    .records()
                    .iter()
                    .try_fold(0usize, |n, r| n.checked_add(diagnostics(r).len()));
                self.append(format_args!("  availability: available semantics=authored-candidates runtime-eligibility=not-observed issued-work=not-observed\n"));
                let mut last_owner = None;
                let mut owner_label = 0usize;
                for record in inventory.records().iter().take(self.limits.records) {
                    if last_owner.is_some_and(|owner| owner != record.source_node()) {
                        owner_label += 1;
                    }
                    last_owner = Some(record.source_node());
                    let events = diagnostics(record);
                    let available = self.limits.diagnostics - diag_shown;
                    let selected = &events[..events.len().min(available)];
                    let mut scratch = Scratch {
                        bytes: [0; 1024],
                        len: 0,
                        maximum: self.limits.scratch.min(1024),
                    };
                    let written = write_record(&mut scratch, record, owner_label, selected);
                    if written.is_err() {
                        self.scratch_limit = true;
                        break;
                    }
                    if scratch.len
                        > self
                            .limits
                            .bytes
                            .saturating_sub(self.limits.footer)
                            .saturating_sub(self.output.len())
                    {
                        self.byte_limit = true;
                        break;
                    }
                    self.output.push_str(
                        std::str::from_utf8(&scratch.bytes[..scratch.len])
                            .expect("formatted UTF-8"),
                    );
                    shown += 1;
                    diag_shown += selected.len();
                }
                record_limit = records_total > self.limits.records;
                diagnostic_limit = diag_shown == self.limits.diagnostics
                    && diagnostics_total.is_none_or(|n| n > diag_shown);
                (
                    "available",
                    "true",
                    InventoryTotals::Exact {
                        records: records_total,
                        diagnostics: diagnostics_total,
                    },
                )
            }
        };
        let diagnostics_omitted = match totals {
            InventoryTotals::Exact { diagnostics, .. } => {
                diagnostics.is_none_or(|n| n > diag_shown)
            }
            InventoryTotals::NotApplicable | InventoryTotals::Unavailable => false,
        };
        let formatting_complete =
            !self.byte_limit && !self.scratch_limit && !record_limit && !diagnostics_omitted;
        // Only fixed labels and bounded numeric fields enter this reserved footer.
        writeln!(self.output, "  observation: availability={availability} semantic_complete={semantic_complete} formatting_complete={formatting_complete}").expect("String write");
        match totals {
            InventoryTotals::Exact {
                records,
                diagnostics,
            } => {
                writeln!(self.output, "  shown: occurrences={shown}/{records} diagnostics={diag_shown}/{diagnostics:?}").expect("String write");
                writeln!(self.output, "  diagnostics_omitted: {diagnostics_omitted}")
                    .expect("String write");
            }
            InventoryTotals::NotApplicable | InventoryTotals::Unavailable => {
                let label = match totals {
                    InventoryTotals::NotApplicable => "not-applicable",
                    _ => "unavailable",
                };
                writeln!(
                    self.output,
                    "  shown: occurrences={shown}/{label} diagnostics={diag_shown}/{label}"
                )
                .expect("String write");
                writeln!(self.output, "  diagnostics_omitted: {label}").expect("String write");
            }
        }
        writeln!(self.output, "  formatting_limits: records={record_limit} diagnostics={diagnostic_limit} bytes={} scratch={}", self.byte_limit, self.scratch_limit).expect("String write");
        self.output
            .push_str("  parsing-and-page-assembly: not-certified\n}\n");
        debug_assert!(self.output.len() <= self.limits.bytes);
        self.output
    }
}
struct Scratch {
    bytes: [u8; 1024],
    len: usize,
    maximum: usize,
}
impl Write for Scratch {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        if text.len() > self.maximum - self.len {
            return Err(fmt::Error);
        }
        self.bytes[self.len..self.len + text.len()].copy_from_slice(text.as_bytes());
        self.len += text.len();
        Ok(())
    }
}
fn diagnostics<'a>(record: &'a StylesheetOccurrence<'_>) -> &'a [UrlDiagnostic] {
    match record.source() {
        StylesheetSource::Link(link) => link.href().outcome().diagnostics(),
        _ => &[],
    }
}
fn presence(value: Option<&str>) -> &'static str {
    match value {
        None => "absent",
        Some("") => "empty",
        Some(_) => "redacted",
    }
}
fn media_label(value: &MediaFact<'_>) -> &'static str {
    let _presence = presence(value.authored());
    match value {
        MediaFact::NotStylesheetRelationship(_) => "not-stylesheet",
        MediaFact::ActiveSubset(_) => "af-active-subset",
        MediaFact::DeferredUnsupported(_) => "af-deferred-unsupported",
    }
}
fn policy_label(value: &PolicyFact<'_>) -> &'static str {
    let _presence = presence(value.authored());
    match value {
        PolicyFact::NotAssessedForOtherRelationship(_) => "other-relation-unassessed",
        PolicyFact::Absent => "absent",
        PolicyFact::PresentUnenforced("") => "empty-unenforced",
        PolicyFact::PresentUnenforced(_) => "present-unenforced",
    }
}
fn write_record(
    out: &mut Scratch,
    record: &StylesheetOccurrence<'_>,
    owner: usize,
    events: &[UrlDiagnostic],
) -> fmt::Result {
    let dependency = record
        .dependency_order()
        .map(StylesheetDependencyOrder::get);
    write!(
        out,
        "  occurrence[{}] owner=element[{owner}] dependency={dependency:?} ",
        record.occurrence_index().get()
    )?;
    match record.source() {
        StylesheetSource::Link(link) => {
            let relation = link.relation();
            writeln!(
                out,
                "kind=link rel={:?} raw-rel={} alternate={:?} other-tokens={} href={}",
                relation.kind(),
                presence(relation.authored()),
                relation.alternate(),
                relation.other_tokens(),
                presence(link.href().authored())
            )?;
            let disabled = match link.disabled() {
                DisabledFact::NotStylesheetRelationship(_) => "not-stylesheet",
                DisabledFact::Absent => "absent",
                DisabledFact::Present(_) => "present",
            };
            writeln!(
                out,
                "    type={} advisory-unassessed title={} selection={:?} disabled={disabled}/{} media={}/{}",
                presence(link.type_hint().authored()),
                presence(link.title().authored()),
                link.selection(),
                presence(link.disabled().authored()),
                media_label(link.media()),
                presence(link.media().authored())
            )?;
            writeln!(
                out,
                "    policies: crossorigin={} integrity={} referrerpolicy={}",
                policy_label(link.policies().crossorigin()),
                policy_label(link.policies().integrity()),
                policy_label(link.policies().referrerpolicy())
            )?;
            match link.href().outcome() {
                StylesheetHrefOutcome::NotStylesheetRelationship => {
                    writeln!(out, "    address: not-stylesheet-relationship")?
                }
                StylesheetHrefOutcome::Missing => writeln!(out, "    address: missing")?,
                StylesheetHrefOutcome::Empty => writeln!(out, "    address: empty")?,
                StylesheetHrefOutcome::Resolved { address, .. } => {
                    let capability: ResourceCapability =
                        address.capability(ResourceOperation::Retrieval);
                    writeln!(
                        out,
                        "    address: resolved-redacted retrieval={capability:?}"
                    )?;
                }
                StylesheetHrefOutcome::Failed(failure) => {
                    writeln!(out, "    address: failed {:?} base=redacted", failure.kind)?
                }
            }
            writeln!(
                out,
                "    diagnostics: total={} shown={events:?}",
                link.href().outcome().diagnostics().len()
            )?;
        }
        StylesheetSource::StyleBlock(block) => {
            // Constructing this borrowed iterator does not inspect or copy text.
            let _text = block.content.text_chunks();
            let type_label = match block.type_fact() {
                StyleTypeFact::Missing => "missing-css-default",
                StyleTypeFact::Empty => "empty-css-default",
                StyleTypeFact::Css(_) => "css",
                StyleTypeFact::Other(_) => "unsupported",
            };
            writeln!(
                out,
                "kind=inline-block text=redacted type={type_label}/{} title={} selection={:?} media={}/{}",
                presence(block.type_fact().authored()),
                presence(block.title().authored()),
                block.selection(),
                media_label(block.media()),
                presence(block.media().authored())
            )?;
            let attrs = block.inapplicable_link_attributes();
            writeln!(
                out,
                "    inapplicable-link-attributes: disabled={} crossorigin={} integrity={} referrerpolicy={}",
                presence(attrs.disabled()),
                presence(attrs.crossorigin()),
                presence(attrs.integrity()),
                presence(attrs.referrerpolicy())
            )?;
        }
        StylesheetSource::StyleAttribute { value } => writeln!(
            out,
            "kind=element-attached-attribute value={} contribution=not-assessed",
            presence(Some(value))
        )?,
    }
    Ok(())
}
