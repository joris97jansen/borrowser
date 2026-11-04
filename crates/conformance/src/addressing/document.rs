//! Bounded in-memory fixture transport. All requests are issued by Browser;
//! supplied CSS traverses the existing response protocol and CSS runtime.
use super::{AddressingInput, base_result, input::*, observations::*};
use crate::{
    execute::{begin_document, publish_document},
    model::HarnessError,
};
use browser::{
    Tab,
    tab::{
        AddressingRequestState, BaseOutcome, CssBase, CssReferenceOutcome, CssSource,
        HtmlReferenceBase, HtmlReferenceOutcome,
    },
};
use bus::{CoreCommand, CoreEvent};
use core_types::{NetworkResponseInfo, RequestId, ResourceKind, StylesheetSlotId};
use css::{
    resource_references::{CssReferenceDisposition, CssResourceContext, CssResourceRole},
    syntax::{DiagnosticKind, DiagnosticSeverity},
};
use html::{Node, internal::Id};
use std::{
    sync::mpsc::{self, Receiver, Sender},
    time::{Duration, Instant},
};

fn error(detail: impl Into<String>) -> HarnessError {
    HarnessError::new("addressing.transport", detail)
}
fn apply(tab: &mut Tab, event: CoreEvent) -> Result<(), HarnessError> {
    tab.on_core_event(event)
        .map_err(|e| error(format!("publication rejected: {e:?}")))
}
#[derive(Clone, Copy)]
struct Issued {
    request: RequestId,
    slot: Option<StylesheetSlotId>,
}
struct OfflineDocumentDriver<'a> {
    case: &'a DocumentCase,
    seen: Vec<Option<Issued>>,
    order: Vec<usize>,
    slots: Vec<StylesheetSlotId>,
}
impl<'a> OfflineDocumentDriver<'a> {
    fn new(case: &'a DocumentCase) -> Self {
        Self {
            case,
            seen: vec![None; case.requests.len()],
            order: vec![],
            slots: vec![],
        }
    }
    fn account(&mut self, tab: &Tab, command: CoreCommand) -> Result<(), HarnessError> {
        let CoreCommand::FetchStream {
            tab_id,
            request_id,
            stylesheet_slot_id,
            kind,
            url,
        } = command
        else {
            return Err(error("unexpected non-parser/resource command"));
        };
        if tab_id != tab.tab_id || request_id != tab.nav_gen {
            return Err(error("resource request has wrong document identity"));
        }
        let manifest_kind = match (kind, stylesheet_slot_id) {
            (ResourceKind::Css, Some(_)) => RequestKind::Css,
            (ResourceKind::Image, None) => RequestKind::Image,
            _ => return Err(error("unexpected resource route or slot")),
        };
        let index = self
            .case
            .requests
            .iter()
            .enumerate()
            .position(|(i, expected)| {
                self.seen[i].is_none() && expected.kind == manifest_kind && expected.url == url
            })
            .ok_or_else(|| {
                error(format!(
                    "unlisted/excess resource request: {} {url}",
                    kind.as_str()
                ))
            })?;
        if self.seen.iter().flatten().any(|seen| {
            seen.request == request_id
                && seen.slot == stylesheet_slot_id
                && stylesheet_slot_id.is_some()
        }) {
            return Err(error("stylesheet attachment issued more than once"));
        }
        if tab.observe_addressing_request(kind, &url, request_id, stylesheet_slot_id)
            != Some(AddressingRequestState::Pending)
        {
            return Err(error("issued resource has no pending production state"));
        }
        self.seen[index] = Some(Issued {
            request: request_id,
            slot: stylesheet_slot_id,
        });
        self.order.push(index);
        if let Some(slot) = stylesheet_slot_id {
            self.slots.push(slot);
        }
        Ok(())
    }
    fn forward_html(
        &mut self,
        tab: &mut Tab,
        rx: &Receiver<CoreCommand>,
        parser: &Sender<CoreCommand>,
    ) -> Result<(), HarnessError> {
        for command in rx.try_iter() {
            match &command {
                CoreCommand::ParseHtmlStart { tab_id, request_id }
                | CoreCommand::ParseHtmlChunk {
                    tab_id, request_id, ..
                }
                | CoreCommand::ParseHtmlDone { tab_id, request_id }
                    if *tab_id == tab.tab_id && *request_id == tab.nav_gen =>
                {
                    parser
                        .send(command)
                        .map_err(|_| error("parser receiver closed"))?;
                }
                _ => self.account(tab, command)?,
            }
        }
        Ok(())
    }
    fn all_required_seen(&self) -> Result<(), HarnessError> {
        if self.seen.iter().any(Option::is_none) {
            return Err(error("required declared resource command did not occur"));
        }
        Ok(())
    }
    fn observations(&self, tab: &Tab) -> Result<Vec<RequestObservation>, HarnessError> {
        self.all_required_seen()?;
        self.order
            .iter()
            .map(|&index| {
                let declared = &self.case.requests[index];
                let actual = self.seen[index].ok_or_else(|| error("missing issued identity"))?;
                let kind = match declared.kind {
                    RequestKind::Image => ResourceKind::Image,
                    RequestKind::Css => ResourceKind::Css,
                };
                let expected = if declared.response.is_some() {
                    AddressingRequestState::Complete
                } else {
                    AddressingRequestState::Pending
                };
                if tab.observe_addressing_request(kind, &declared.url, actual.request, actual.slot)
                    != Some(expected)
                {
                    return Err(error(
                        "served/pending resource does not have the required production state",
                    ));
                }
                let stylesheet = actual
                    .slot
                    .and_then(|slot| {
                        self.slots
                            .iter()
                            .position(|s| *s == slot)
                            .map(|n| format!("stylesheet[{n}]"))
                    })
                    .map_or(Availability::Inapplicable, Availability::Present);
                Ok(RequestObservation {
                    kind: kind.as_str().into(),
                    url: declared.url.clone(),
                    stylesheet,
                    state: if declared.response.is_some() {
                        "complete"
                    } else {
                        "pending_unserved"
                    }
                    .into(),
                    supplied_final_url: declared
                        .response
                        .as_ref()
                        .map_or(Availability::Inapplicable, |r| {
                            Availability::Present(r.final_url.clone())
                        }),
                })
            })
            .collect()
    }
}

pub(super) fn execute(
    input: AddressingInput,
    case: &DocumentCase,
) -> Result<DocumentObservation, HarnessError> {
    let (commands, command_rx) = mpsc::channel();
    let (parser, parser_rx) = mpsc::channel();
    let (events, event_rx) = mpsc::channel();
    runtime_parse::start_parse_runtime(parser_rx, events);
    let mut tab = Tab::new(1);
    tab.set_bus_sender(commands);
    begin_document(&mut tab, &command_rx, &case.requested_url)?;
    let mut driver = OfflineDocumentDriver::new(case);
    publish_document(
        &mut tab,
        input.asset(&case.html)?.as_bytes(),
        NetworkResponseInfo {
            requested_url: case.requested_url.clone(),
            final_url: case.final_url.clone(),
            status_code: Some(200),
            content_type: Some("text/html; charset=utf-8".into()),
        },
        &event_rx,
        |tab| driver.forward_html(tab, &command_rx, &parser),
    )?;
    driver.all_required_seen()?;
    // Supply only declared responses, after HTML terminal completion. Nothing
    // fabricates decoded CSS events or treats a pending request as completed.
    for index in driver.order.clone() {
        let declared = &case.requests[index];
        let Some(response) = &declared.response else {
            continue;
        };
        let issued = driver.seen[index].ok_or_else(|| error("missing issued stylesheet"))?;
        let slot = issued
            .slot
            .ok_or_else(|| error("CSS response has no issued slot"))?;
        let bytes = input.asset(&response.css)?.as_bytes();
        let metadata = NetworkResponseInfo {
            requested_url: declared.url.clone(),
            final_url: response.final_url.clone(),
            status_code: Some(200),
            content_type: Some("text/css".into()),
        };
        apply(
            &mut tab,
            CoreEvent::NetworkStart {
                tab_id: 1,
                request_id: issued.request,
                stylesheet_slot_id: Some(slot),
                kind: ResourceKind::Css,
                response: metadata.clone(),
            },
        )?;
        apply(
            &mut tab,
            CoreEvent::NetworkChunk {
                tab_id: 1,
                request_id: issued.request,
                stylesheet_slot_id: Some(slot),
                kind: ResourceKind::Css,
                url: declared.url.clone(),
                bytes: bytes.to_vec(),
            },
        )?;
        apply(
            &mut tab,
            CoreEvent::NetworkDone {
                tab_id: 1,
                request_id: issued.request,
                stylesheet_slot_id: Some(slot),
                kind: ResourceKind::Css,
                response: metadata,
                bytes_received: bytes.len(),
            },
        )?;
        let (css, css_rx) = mpsc::channel();
        let (css_events, css_event_rx) = mpsc::channel();
        runtime_css::start_css_runtime(css_rx, css_events);
        let mut chunks = 0;
        let mut done = false;
        for command in command_rx.try_iter() {
            match &command {
                CoreCommand::CssChunk {
                    tab_id: 1,
                    request_id,
                    stylesheet_slot_id,
                    url,
                    bytes: forwarded,
                } if *request_id == issued.request
                    && *stylesheet_slot_id == slot
                    && *url == declared.url
                    && forwarded == bytes
                    && chunks == 0
                    && !done =>
                {
                    chunks += 1;
                }
                CoreCommand::CssDone {
                    tab_id: 1,
                    request_id,
                    stylesheet_slot_id,
                    url,
                } if *request_id == issued.request
                    && *stylesheet_slot_id == slot
                    && *url == declared.url
                    && chunks == 1
                    && !done =>
                {
                    done = true;
                }
                _ => {
                    return Err(error(
                        "unexpected CSS command, bytes, identity, or ordering",
                    ));
                }
            }
            css.send(command)
                .map_err(|_| error("CSS receiver closed"))?;
        }
        if chunks != 1 || !done {
            return Err(error("required CSS assembly commands did not occur"));
        }
        drop(css);
        let deadline = Instant::now() + Duration::from_secs(5);
        for block in [true, false] {
            let event = css_event_rx
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .map_err(|_| error("required CSS runtime event missing"))?;
            match &event {
                CoreEvent::CssDecodedBlock {
                    tab_id: 1,
                    request_id,
                    stylesheet_slot_id,
                    url,
                    ..
                } if block
                    && *request_id == issued.request
                    && *stylesheet_slot_id == slot
                    && *url == declared.url => {}
                CoreEvent::CssSheetDone {
                    tab_id: 1,
                    request_id,
                    stylesheet_slot_id,
                    url,
                } if !block
                    && *request_id == issued.request
                    && *stylesheet_slot_id == slot
                    && *url == declared.url => {}
                _ => return Err(error("unexpected CSS runtime event/identity/order")),
            }
            apply(&mut tab, event)?;
            if command_rx.try_recv().is_ok() {
                return Err(error("unexpected command during CSS installation"));
            }
        }
        match css_event_rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Err(mpsc::RecvTimeoutError::Disconnected) => {}
            Err(mpsc::RecvTimeoutError::Timeout) => {
                return Err(error("CSS worker did not finish after input closure"));
            }
            Ok(_) => return Err(error("excess CSS runtime event")),
        }
        let view = tab
            .observe_document_addressing()
            .map_err(|e| error(format!("observation: {e:?}")))?
            .ok_or_else(|| error("missing committed document after CSS installation"))?;
        if view.css_sources().iter().filter(|s| matches!(s.source(), CssSource::Stylesheet { slot: actual, .. } if *actual == slot)).count() != 1 {
            return Err(error("supplied CSS has no unique production parsed attachment"));
        }
    }
    let before = tab
        .observe_document_addressing()
        .map_err(|e| error(format!("observation: {e:?}")))?
        .ok_or_else(|| error("missing committed document"))?
        .origin()
        .clone();
    let mut snapshots = vec![snapshot(&tab, &driver.slots)?];
    let retained_origin_same = if let Some(destination) = &case.same_document_url {
        tab.navigate_to_new(destination.clone());
        if command_rx.try_recv().is_ok() {
            return Err(error("same-document fixture emitted a command"));
        }
        snapshots.push(snapshot(&tab, &driver.slots)?);
        let observation = tab
            .observe_document_addressing()
            .map_err(|e| error(format!("observation: {e:?}")))?
            .ok_or_else(|| error("same-document addressing disappeared"))?;
        Availability::Present(before.is_same_origin(observation.origin()))
    } else {
        Availability::Inapplicable
    };
    let requests = driver.observations(&tab)?;
    if command_rx.try_recv().is_ok() {
        return Err(error("observation emitted a command"));
    }
    Ok(DocumentObservation {
        requested_url: case.requested_url.clone(),
        final_url: case.final_url.clone(),
        snapshots,
        retained_origin_same,
        requests,
    })
}

fn nodes(document: &Node) -> Vec<&Node> {
    let mut found = vec![];
    let mut stack = vec![document];
    while let Some(node) = stack.pop() {
        match node {
            Node::Document { children, .. } => stack.extend(children.iter().rev()),
            Node::Element { element } => {
                found.push(node);
                stack.extend(element.children().iter().rev());
            }
            _ => {}
        }
    }
    found
}
fn node<'a>(nodes: &[&'a Node], id: Id) -> Result<(&'a Node, String), HarnessError> {
    nodes
        .iter()
        .enumerate()
        .find(|(_, n)| n.id() == id)
        .map(|(index, n)| (*n, format!("element[{index}]")))
        .ok_or_else(|| error("source node is not in the committed ordinary tree"))
}
fn dom_source(label: String, attribute: &str) -> SourceObservation {
    SourceObservation {
        node: label,
        attribute: Availability::Present(attribute.into()),
        input_kind: "stored_dom_attribute".into(),
        location: Availability::Unavailable("HTML DOM does not retain byte locations".into()),
    }
}
fn existing_base(source: &str, url: &::addressing::BrowserUrl) -> BaseObservation {
    BaseObservation {
        source: source.into(),
        input: Availability::Inapplicable,
        url: Availability::Present(parsed(url)),
        diagnostics: vec![],
        failure: Availability::Inapplicable,
    }
}
fn context(value: CssResourceContext) -> &'static str {
    match value {
        CssResourceContext::Import => "import",
        CssResourceContext::FontFace => "font_face",
        CssResourceContext::ImageSet => "image_set",
        CssResourceContext::CustomProperty => "custom_property",
        CssResourceContext::Substitution => "substitution",
        CssResourceContext::UnsupportedFunction => "unsupported_function",
        CssResourceContext::UnsupportedRule => "unsupported_rule",
        CssResourceContext::UrlModifier => "url_modifier",
    }
}
fn role(value: CssResourceRole) -> &'static str {
    match value {
        CssResourceRole::Image => "image",
        CssResourceRole::Font => "font",
        CssResourceRole::StylesheetImport => "stylesheet_import",
        CssResourceRole::Cursor => "cursor",
        CssResourceRole::Mask => "mask",
        CssResourceRole::Unknown => "unknown",
    }
}
fn syntax_kind(value: DiagnosticKind) -> &'static str {
    match value {
        DiagnosticKind::UnexpectedEof => "unexpected_eof",
        DiagnosticKind::UnexpectedToken => "unexpected_token",
        DiagnosticKind::InvariantViolation => "invariant_violation",
        DiagnosticKind::EmptySelectorList => "empty_selector_list",
        DiagnosticKind::InvalidSelector => "invalid_selector",
        DiagnosticKind::UnsupportedSelector => "unsupported_selector",
        DiagnosticKind::InvalidDeclaration => "invalid_declaration",
        DiagnosticKind::UnterminatedComment => "unterminated_comment",
        DiagnosticKind::UnterminatedString => "unterminated_string",
        DiagnosticKind::BadUrl => "bad_url",
        DiagnosticKind::LimitExceeded => "limit_exceeded",
    }
}
fn snapshot(
    tab: &Tab,
    issued_slots: &[StylesheetSlotId],
) -> Result<DocumentSnapshot, HarnessError> {
    let view = tab
        .observe_document_addressing()
        .map_err(|e| error(format!("observation: {e:?}")))?
        .ok_or_else(|| error("missing committed addressing observation"))?;
    let nodes = nodes(view.document());
    let selected_base = view
        .selected_base()
        .map(|selected| {
            let (node, label) = node(&nodes, selected.source_node())?;
            let href = node
                .attr("href")
                .ok_or_else(|| error("selected base href is unavailable"))?;
            let outcome = match selected.outcome() {
                BaseOutcome::Resolved => "resolved".into(),
                BaseOutcome::InvalidHref(e) => format!("invalid_href:{}", parse_error(*e)),
                BaseOutcome::ProhibitedData => "prohibited_data".into(),
                BaseOutcome::ProhibitedJavascript => "prohibited_javascript".into(),
            };
            Ok(SelectedBaseObservation {
                source: dom_source(label, "href"),
                href: href.into(),
                url: parsed(selected.url()),
                outcome,
                diagnostics: diagnostics(selected.diagnostics()),
            })
        })
        .transpose()?
        .map_or(Availability::Inapplicable, Availability::Present);
    let html = view
        .html_references()
        .iter()
        .map(|reference| {
            let (node, label) = node(&nodes, reference.source_node())?;
            let input = node
                .attr(reference.attribute())
                .ok_or_else(|| error("source attribute is unavailable"))?
                .to_owned();
            let base = match reference.resolution_base() {
                HtmlReferenceBase::ActiveDocument => {
                    Availability::Present(existing_base("document_active", view.active_base()))
                }
                HtmlReferenceBase::DocumentUrl => {
                    Availability::Present(existing_base("document_url", view.document_url()))
                }
                HtmlReferenceBase::Inapplicable => Availability::Inapplicable,
            };
            let outcome = match reference.outcome() {
                HtmlReferenceOutcome::Resolved {
                    address,
                    diagnostics,
                } => resolved(address, diagnostics),
                HtmlReferenceOutcome::EmptyReference => ResourceOutcome::EmptyReference {},
                HtmlReferenceOutcome::Failed(e) => ResourceOutcome::Failed {
                    diagnostics: diagnostics(&e.diagnostics),
                    failure: failure(e)?,
                },
            };
            Ok(HtmlObservation {
                input,
                source: dom_source(label, reference.attribute()),
                kind: kind(reference.kind()).into(),
                base,
                outcome,
            })
        })
        .collect::<Result<_, HarnessError>>()?;
    let mut slots = issued_slots.to_vec();
    let css = view
        .css_sources()
        .iter()
        .map(|source| {
            let (_, owner) = node(&nodes, source.owner())?;
            let (source_label, attribute) = match source.source() {
                CssSource::Attribute => (
                    format!("style_attribute[{owner}]"),
                    Availability::Present("style".into()),
                ),
                CssSource::Stylesheet { slot, .. } => {
                    let index = match slots.iter().position(|s| s == slot) {
                        Some(n) => n,
                        None => {
                            slots.push(*slot);
                            slots.len() - 1
                        }
                    };
                    (format!("stylesheet[{index}]"), Availability::Inapplicable)
                }
            };
            let base = match source.base() {
                CssBase::Document(Some(url)) => existing_base("document_active", url),
                CssBase::Document(None) => BaseObservation {
                    source: "document_active".into(),
                    input: Availability::Unavailable("document base unavailable".into()),
                    url: Availability::Unavailable("document base unavailable".into()),
                    diagnostics: vec![],
                    failure: Availability::Inapplicable,
                },
                CssBase::Response(Some(result)) => base_result(
                    "admitted_stylesheet_response",
                    Availability::Unavailable(
                        "raw response URL is transport provenance, not retained CSS input".into(),
                    ),
                    result,
                )?,
                CssBase::Response(None) => BaseObservation {
                    source: "admitted_stylesheet_response".into(),
                    input: Availability::Unavailable("missing final response URL".into()),
                    url: Availability::Unavailable("missing final response URL".into()),
                    diagnostics: vec![],
                    failure: Availability::Inapplicable,
                },
            };
            let discovery = source.discovery();
            let references = discovery
                .references
                .iter()
                .map(|reference| {
                    let raw = discovery
                        .input
                        .slice(reference.span)
                        .ok_or_else(|| error("CSS span is not bound to its source"))?
                        .into();
                    let position = discovery
                        .input
                        .position(reference.span.start)
                        .ok_or_else(|| error("CSS position is unavailable"))?;
                    let (shape, decoded) = match &reference.disposition {
                        CssReferenceDisposition::Literal(value) => {
                            ("literal", Availability::Present(value.value().into()))
                        }
                        CssReferenceDisposition::LocalFragment(value) => (
                            "local_fragment",
                            Availability::Present(value.value().into()),
                        ),
                        CssReferenceDisposition::Empty => {
                            ("empty", Availability::Present(String::new()))
                        }
                        CssReferenceDisposition::InvalidCss => (
                            "invalid_css",
                            Availability::Unavailable(
                                "invalid CSS URL has no decoded value".into(),
                            ),
                        ),
                    };
                    let outcome = match source.outcome(reference) {
                        CssReferenceOutcome::Resolved {
                            address,
                            diagnostics,
                        } => resolved(&address, &diagnostics),
                        CssReferenceOutcome::Failed(e) => ResourceOutcome::Failed {
                            diagnostics: diagnostics(&e.diagnostics),
                            failure: failure(&e)?,
                        },
                        CssReferenceOutcome::IncompleteSource => {
                            ResourceOutcome::IncompleteSource {}
                        }
                        CssReferenceOutcome::BlockedContext => ResourceOutcome::BlockedContext {},
                        CssReferenceOutcome::InvalidCss => ResourceOutcome::InvalidCss {},
                        CssReferenceOutcome::Empty => ResourceOutcome::Empty {},
                        CssReferenceOutcome::LocalFragment => ResourceOutcome::LocalFragment {},
                        CssReferenceOutcome::UnresolvedBase => ResourceOutcome::UnresolvedBase {},
                    };
                    let declaration = reference
                        .declaration_applicability
                        .map(|value| {
                            use css::CascadeDeclarationApplicability::*;
                            match value {
                                Supported(_) => "supported",
                                InvalidValue(_) => "invalid_value",
                                InvalidShorthandValue(_) => "invalid_shorthand_value",
                                UnsupportedProperty => "unsupported_property",
                                CustomProperty => "custom_property",
                                InvalidPropertyName => "invalid_property_name",
                            }
                            .into()
                        })
                        .map_or(Availability::Inapplicable, Availability::Present);
                    let selector = reference
                        .selector_matchability
                        .map(|value| {
                            match value {
                                css::SelectorMatchability::Parsed => "parsed",
                                css::SelectorMatchability::Unsupported => "unsupported",
                                css::SelectorMatchability::Invalid => "invalid",
                            }
                            .into()
                        })
                        .map_or(Availability::Inapplicable, Availability::Present);
                    Ok(CssReferenceObservation {
                        raw,
                        decoded,
                        shape: shape.into(),
                        contexts: reference
                            .contexts
                            .iter()
                            .map(|c| context(*c).into())
                            .collect(),
                        role: role(reference.role).into(),
                        declaration,
                        selector,
                        source: SourceObservation {
                            node: owner.clone(),
                            attribute: attribute.clone(),
                            input_kind: "css_source_slice_and_decoded_reference".into(),
                            location: Availability::Present(CssLocation {
                                start: reference.span.start,
                                end: reference.span.end,
                                line: position.line,
                                column: position.column,
                            }),
                        },
                        outcome,
                    })
                })
                .collect::<Result<_, HarnessError>>()?;
            Ok(CssSourceObservation {
                source: source_label,
                owner,
                base,
                condition_deferred: source.condition_deferred(),
                effective_participation: "unevaluated".into(),
                incomplete: discovery.stats.hit_limit,
                feature_notices_truncated: discovery.feature_notices_truncated,
                diagnostics: discovery
                    .diagnostics
                    .iter()
                    .map(|d| CssDiagnostic {
                        kind: syntax_kind(d.kind).into(),
                        severity: match d.severity {
                            DiagnosticSeverity::Warning => "warning",
                            DiagnosticSeverity::Error => "error",
                        }
                        .into(),
                        offset: d.byte_offset,
                    })
                    .collect(),
                feature_notices: discovery
                    .feature_notices
                    .iter()
                    .map(|(span, c)| CssNotice {
                        context: context(*c).into(),
                        start: span.start,
                        end: span.end,
                    })
                    .collect(),
                references,
            })
        })
        .collect::<Result<_, HarnessError>>()?;
    Ok(DocumentSnapshot {
        document_url: parsed(view.document_url()),
        document_origin: origin(view.origin()),
        fallback_base: parsed(view.fallback_base()),
        active_base: parsed(view.active_base()),
        selected_base,
        html,
        css,
        css_scope: "available_parsed_sources".into(),
    })
}
