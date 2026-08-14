use super::Tab;
use crate::dom_store::DomPatchError;
use crate::page::RestyleHint;
use bus::{CoreEvent, DocumentPublication, DocumentPublicationFailure, DocumentPublicationPayload};
use core_types::ResourceKind;
use html::{DomPatch, PatchKey};

impl Tab {
    pub fn on_core_event(&mut self, evt: CoreEvent) {
        match evt {
            CoreEvent::NetworkStart {
                tab_id,
                request_id,
                stylesheet_slot_id: _,
                kind: ResourceKind::Html,
                response,
            } if self.is_current(tab_id, request_id) => {
                self.on_html_network_start(response, request_id);
            }

            CoreEvent::NetworkChunk {
                tab_id,
                request_id,
                stylesheet_slot_id: _,
                kind: ResourceKind::Html,
                url: _,
                bytes,
                ..
            } if self.is_current(tab_id, request_id) => {
                self.on_html_network_chunk(bytes, request_id);
            }

            CoreEvent::NetworkDone {
                tab_id,
                request_id,
                stylesheet_slot_id: _,
                kind: ResourceKind::Html,
                response,
                bytes_received,
            } if self.is_current(tab_id, request_id) => {
                self.on_html_network_done(response, bytes_received, request_id);
            }

            CoreEvent::NetworkError {
                tab_id,
                request_id,
                stylesheet_slot_id: _,
                kind: ResourceKind::Html,
                url,
                error_kind,
                status_code,
                error,
            } if self.is_current(tab_id, request_id) => {
                self.on_html_network_error(url, error_kind, status_code, error);
            }

            CoreEvent::DomPatchUpdate {
                tab_id,
                request_id,
                publication,
            } if self.is_current(tab_id, request_id) => {
                if let Err(failure) = self.commit_document_publication(publication, request_id) {
                    self.on_document_publication_failure(failure);
                }
            }
            CoreEvent::DocumentPublicationFailed {
                tab_id,
                request_id,
                failure,
                ..
            } if self.is_current(tab_id, request_id) => {
                self.on_document_publication_failure(failure);
            }

            CoreEvent::NetworkStart {
                tab_id,
                request_id,
                stylesheet_slot_id: Some(stylesheet_slot_id),
                kind: ResourceKind::Css,
                response,
            } if self.is_current(tab_id, request_id) => {
                self.on_css_network_start(stylesheet_slot_id, response);
            }
            CoreEvent::NetworkChunk {
                tab_id,
                request_id,
                stylesheet_slot_id: Some(stylesheet_slot_id),
                kind: ResourceKind::Css,
                url,
                bytes,
            } if self.is_current(tab_id, request_id) => {
                self.on_css_network_chunk(stylesheet_slot_id, url, bytes, request_id);
            }
            CoreEvent::NetworkChunk {
                tab_id,
                request_id,
                stylesheet_slot_id: _,
                kind: ResourceKind::Image,
                url,
                bytes,
            } if self.is_current(tab_id, request_id) => {
                self.on_image_network_chunk(url, bytes);
            }
            CoreEvent::NetworkDone {
                tab_id,
                request_id,
                stylesheet_slot_id: Some(stylesheet_slot_id),
                kind: ResourceKind::Css,
                response,
                bytes_received,
            } if self.is_current(tab_id, request_id) => {
                self.on_css_network_done(stylesheet_slot_id, response, bytes_received, request_id);
            }
            CoreEvent::NetworkDone {
                tab_id,
                request_id,
                stylesheet_slot_id: _,
                kind: ResourceKind::Image,
                response,
                ..
            } if self.is_current(tab_id, request_id) => {
                self.on_image_network_done(response.requested_url);
            }
            CoreEvent::NetworkError {
                tab_id,
                request_id,
                stylesheet_slot_id: Some(stylesheet_slot_id),
                kind: ResourceKind::Css,
                url,
                error_kind,
                status_code,
                error,
            } if self.is_current(tab_id, request_id) => {
                self.on_css_network_error(
                    stylesheet_slot_id,
                    url,
                    error_kind,
                    status_code,
                    error,
                    request_id,
                );
            }
            CoreEvent::NetworkError {
                tab_id,
                request_id,
                stylesheet_slot_id: _,
                kind: ResourceKind::Image,
                url,
                error_kind: _,
                status_code: _,
                error,
            } if self.is_current(tab_id, request_id) => {
                self.on_image_network_error(url, error);
            }

            CoreEvent::CssDecodedBlock {
                tab_id,
                request_id,
                stylesheet_slot_id,
                css_block,
                ..
            } if self.is_current(tab_id, request_id) => {
                self.on_css_decoded_block(stylesheet_slot_id, css_block);
            }
            CoreEvent::CssSheetDone {
                tab_id,
                request_id,
                stylesheet_slot_id,
                url,
            } if self.is_current(tab_id, request_id) => {
                self.on_css_sheet_done(stylesheet_slot_id, url);
            }

            _ => {}
        }
    }
}

impl Tab {
    fn commit_document_publication(
        &mut self,
        publication: DocumentPublication,
        request_id: u64,
    ) -> Result<(), DocumentPublicationFailure> {
        let DocumentPublication {
            handle,
            document_mode,
            payload,
        } = publication;
        let mut staged_store = self.dom_store.clone();
        let new_handle = self.dom_handle != Some(handle);
        let (dom, restyle_hint, staged_version) = match payload {
            DocumentPublicationPayload::Patch { from, to, patches } => {
                if !new_handle && self.page.document_mode != Some(document_mode) {
                    return Err(DocumentPublicationFailure::DocumentModeChanged);
                }
                if new_handle {
                    staged_store.clear();
                    staged_store
                        .create(handle)
                        .map_err(|_| DocumentPublicationFailure::InvalidPayload)?;
                }
                staged_store
                    .apply(handle, from, to, &patches)
                    .map_err(map_dom_patch_error)?;
                let dom = staged_store
                    .materialize(handle)
                    .map_err(|_| DocumentPublicationFailure::MaterializationFailed)?;
                let dirty = patch_attribute_keys(&patches);
                let dirty_nodes = staged_store
                    .resolve_live_node_ids(handle, &dirty)
                    .map_err(|_| DocumentPublicationFailure::MaterializationFailed)?;
                let hint = RestyleHint::from_dom_patch_batch(&patches, dirty_nodes);
                (dom, hint, to)
            }
        };

        // Commit only after the candidate store and materialized DOM validate.
        self.dom_store = staged_store;
        self.dom_handle = Some(handle);
        self.dom_version = staged_version;
        let render_work =
            self.page
                .commit_dom_publication(dom, document_mode, restyle_hint, new_handle);
        self.page.update_head_metadata();
        self.page
            .seed_input_values_from_dom(&mut self.document_input.input_values);
        self.page.update_visible_text_cache();
        self.discover_resources(request_id);
        let pending = self.page.pending_count();
        self.loading = pending > 0;
        let base = if pending > 0 {
            format!("Document parsed • fetching {pending} stylesheet(s)")
        } else {
            "Document parsed".to_string()
        };
        self.last_status = Some(match self.document_load.response.as_ref() {
            Some(response) => format!(
                "{base} • {}",
                super::status::response_summary(response, self.document_load.bytes_received)
            ),
            None => base,
        });
        self.request_optional_render_work(render_work);
        Ok(())
    }

    fn on_document_publication_failure(&mut self, failure: DocumentPublicationFailure) {
        self.loading = false;
        self.last_status = Some(format!("Document publication failed: {failure:?}"));
        self.poke_redraw();
    }
}

fn map_dom_patch_error(error: DomPatchError) -> DocumentPublicationFailure {
    match error {
        DomPatchError::VersionMismatch { .. } | DomPatchError::NonMonotonicVersion { .. } => {
            DocumentPublicationFailure::GenerationMismatch
        }
        DomPatchError::UnknownHandle(_) | DomPatchError::DuplicateHandle(_) => {
            DocumentPublicationFailure::InvariantViolation
        }
        DomPatchError::Protocol(_)
        | DomPatchError::InvalidKey(_)
        | DomPatchError::DuplicateKey(_)
        | DomPatchError::MissingKey(_)
        | DomPatchError::WrongNodeKind { .. }
        | DomPatchError::InvalidParent(_)
        | DomPatchError::MoveNotSupported { .. }
        | DomPatchError::IllegalMove { .. }
        | DomPatchError::InvalidSibling { .. }
        | DomPatchError::CycleDetected { .. }
        | DomPatchError::MissingRoot
        | DomPatchError::UnsupportedPatch(_) => DocumentPublicationFailure::InvalidPayload,
    }
}

fn patch_attribute_keys(patches: &[DomPatch]) -> Vec<PatchKey> {
    patches
        .iter()
        .filter_map(|patch| match patch {
            DomPatch::SetAttributes { key, .. } => Some(*key),
            _ => None,
        })
        .collect()
}
