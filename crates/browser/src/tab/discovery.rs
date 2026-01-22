use super::Tab;
use bus::CoreCommand;
use core_types::{RequestId, ResourceKind};
use html::dom_utils::{collect_img_srcs, collect_stylesheet_hrefs};
use url::Url;

impl Tab {
    pub(crate) fn discover_resources(&mut self, request_id: RequestId) {
        self.discover_stylesheets(request_id);
        self.discover_images(request_id);
    }

    fn discover_stylesheets(&mut self, request_id: RequestId) {
        let (Some(dom_ref), Some(base)) = (self.page.dom.as_deref(), self.base_url()) else {
            return;
        };

        let mut hrefs = Vec::new();
        collect_stylesheet_hrefs(dom_ref, &mut hrefs);

        for h in hrefs {
            if let Ok(abs) = base.join(&h) {
                let href = abs.to_string();
                if self.page.register_css(&href) {
                    self.send_fetch(request_id, href, ResourceKind::Css);
                }
            }
        }
    }

    fn discover_images(&mut self, request_id: RequestId) {
        let (Some(dom_ref), Some(base)) = (self.page.dom.as_deref(), self.base_url()) else {
            return;
        };

        let mut srcs = Vec::new();
        collect_img_srcs(dom_ref, &mut srcs);

        let cmd_tx = self.cmd_tx.clone();
        let tab_id = self.tab_id;

        if srcs.is_empty() {
            return;
        }

        for src in srcs {
            if let Ok(abs) = base.join(&src) {
                let url = abs.to_string();
                self.resources.request_image(url, |url| {
                    if let Some(tx) = &cmd_tx {
                        let _ = tx.send(CoreCommand::FetchStream {
                            tab_id,
                            request_id,
                            url,
                            kind: ResourceKind::Image,
                        });
                    }
                });
            }
        }
    }

    fn base_url(&self) -> Option<Url> {
        let base = self.page.base_url.as_ref()?;
        Url::parse(base).ok()
    }

    fn send_fetch(&self, request_id: RequestId, url: String, kind: ResourceKind) {
        self.send_cmd(CoreCommand::FetchStream {
            tab_id: self.tab_id,
            request_id,
            url,
            kind,
        });
    }
}
