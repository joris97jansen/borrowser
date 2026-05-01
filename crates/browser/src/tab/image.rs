use super::Tab;
use crate::rendering::{RenderInvalidationEntryPoint, render_invalidation_request};

impl Tab {
    pub(super) fn on_image_network_chunk(&mut self, url: String, bytes: Vec<u8>) {
        self.resources.on_network_chunk(&url, &bytes);
    }

    pub(super) fn on_image_network_done(&mut self, url: String) {
        self.resources.on_network_done(&url, self.repaint.clone());
    }

    pub(super) fn on_image_network_error(&mut self, url: String, error: String) {
        self.resources.on_network_error(&url, error);
        self.request_render_work(render_invalidation_request(
            RenderInvalidationEntryPoint::ResourceStateChanged,
        ));
    }
}
