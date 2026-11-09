use super::Tab;
use crate::rendering::{IntrinsicRenderInvalidationSource, render_intrinsic_invalidation_request};
use core_types::RequestId;
use net::{BodyProgress, NetworkErrorKind, NetworkResponseInfo};

impl Tab {
    pub(super) fn on_image_network_start(
        &mut self,
        request_id: RequestId,
        response: NetworkResponseInfo,
    ) {
        if self.resources.on_network_start(request_id, response) {
            self.image_resource_state_changed();
        }
    }
    pub(super) fn on_image_network_chunk(
        &mut self,
        request_id: RequestId,
        url: String,
        bytes: Vec<u8>,
    ) {
        if self.resources.on_network_chunk(request_id, &url, &bytes) {
            self.image_resource_state_changed();
        }
    }
    pub(super) fn on_image_network_done(
        &mut self,
        request_id: RequestId,
        response: NetworkResponseInfo,
        body: BodyProgress,
    ) {
        if self
            .resources
            .on_network_done(request_id, response, body, self.repaint.clone())
        {
            self.image_resource_state_changed();
        }
    }
    pub(super) fn on_image_network_error(
        &mut self,
        request_id: RequestId,
        url: String,
        kind: NetworkErrorKind,
        _error: String,
    ) {
        if self.resources.on_network_error(
            request_id,
            &url,
            kind,
            super::status::format_network_error("image", &url, kind, None, ""),
        ) {
            self.image_resource_state_changed();
        }
    }

    fn image_resource_state_changed(&mut self) {
        self.request_render_work(render_intrinsic_invalidation_request(
            IntrinsicRenderInvalidationSource::ResourceStateChanged,
        ));
    }
}
