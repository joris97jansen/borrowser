use std::collections::HashMap;
use std::sync::mpsc;

use addressing::BrowserUrl;
use app_api::RepaintHandle;
use egui::{ColorImage, TextureHandle, TextureId, TextureOptions};
use gfx::paint::ImageProvider;
use tools::common::MAX_IMAGE_BYTES;

const MAX_IMAGE_PIXELS: usize = 16_777_216; // 4096 * 4096

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ImageId(u64);

impl ImageId {
    fn next(v: &mut u64) -> Self {
        let id = *v;
        *v = v.wrapping_add(1).max(1);
        Self(id.max(1))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceStateKind {
    Loading,
    Decoding,
    Ready,
    Error,
}

#[derive(Clone, Debug)]
pub struct ReadyImage {
    pub id: ImageId,
    pub texture_id: TextureId,
    pub size_px: [usize; 2],
}

#[derive(Clone, Debug)]
pub enum ImageState {
    Missing,
    Loading { id: ImageId },
    Decoding { id: ImageId },
    Ready(ReadyImage),
    Error { id: ImageId, error: String },
}

struct DecodedImage {
    size_px: [usize; 2],
    rgba: Vec<u8>,
}

struct DecodeResult {
    id: ImageId,
    decoded: Result<DecodedImage, String>,
}

enum EntryState {
    Loading,
    Decoding,
    Ready {
        texture: TextureHandle,
        size_px: [usize; 2],
    },
    Error {
        error: String,
    },
}

struct ImageEntry {
    url: String,
    state: EntryState,
    bytes: Vec<u8>,
}

/// Per-tab cache and pipeline for media resources (currently: images).
///
/// Goals:
/// - Stable `ImageId` per URL.
/// - Never block the UI thread on decoding.
/// - Let paint/layout query state cheaply.
/// - Integrate with existing `runtime_net` via `FetchStream` events.
pub struct ResourceManager {
    next_image_id: u64,
    image_id_by_url: HashMap<String, ImageId>,
    images: HashMap<ImageId, ImageEntry>,
    decode_done_rx: mpsc::Receiver<DecodeResult>,
    decode_done_tx: mpsc::Sender<DecodeResult>,
}

impl ResourceManager {
    pub fn new() -> Self {
        let (decode_done_tx, decode_done_rx) = mpsc::channel();
        Self {
            next_image_id: 1,
            image_id_by_url: HashMap::new(),
            images: HashMap::new(),
            decode_done_rx,
            decode_done_tx,
        }
    }

    /// Returns a stable `ImageId` and ensures the image is requested at most once.
    ///
    /// `start_fetch` should enqueue a `CoreCommand::FetchStream { kind: ResourceKind::Image, .. }`.
    pub fn request_image<F>(&mut self, url: String, mut start_fetch: F) -> ImageId
    where
        F: FnMut(String),
    {
        if let Some(id) = self.image_id_by_url.get(&url).copied() {
            return id;
        }

        let id = ImageId::next(&mut self.next_image_id);
        self.image_id_by_url.insert(url.clone(), id);
        self.images.insert(
            id,
            ImageEntry {
                url: url.clone(),
                state: EntryState::Loading,
                bytes: Vec::new(),
            },
        );

        start_fetch(url);
        id
    }

    pub fn image_state(&self, id: ImageId) -> ImageState {
        let Some(entry) = self.images.get(&id) else {
            return ImageState::Missing;
        };

        match &entry.state {
            EntryState::Loading => ImageState::Loading { id },
            EntryState::Decoding => ImageState::Decoding { id },
            EntryState::Ready { texture, size_px } => ImageState::Ready(ReadyImage {
                id,
                texture_id: texture.id(),
                size_px: *size_px,
            }),
            EntryState::Error { error } => ImageState::Error {
                id,
                error: error.clone(),
            },
        }
    }

    pub fn image_state_by_url(&self, url: &str) -> ImageState {
        let Some(id) = self.image_id_by_url.get(url).copied() else {
            return ImageState::Missing;
        };
        self.image_state(id)
    }

    pub fn image_intrinsic_size_px(&self, url: &str) -> Option<(u32, u32)> {
        let id = self.image_id_by_url.get(url).copied()?;
        let entry = self.images.get(&id)?;
        match &entry.state {
            EntryState::Ready { size_px, .. } => Some((size_px[0] as u32, size_px[1] as u32)),
            _ => None,
        }
    }

    pub fn on_network_chunk(&mut self, url: &str, bytes: &[u8]) {
        let Some(id) = self.image_id_by_url.get(url).copied() else {
            return;
        };
        let Some(entry) = self.images.get_mut(&id) else {
            return;
        };

        if !matches!(entry.state, EntryState::Loading) {
            return;
        }

        if entry.bytes.len().saturating_add(bytes.len()) > MAX_IMAGE_BYTES {
            entry.state = EntryState::Error {
                error: format!("image too large (>{} bytes)", MAX_IMAGE_BYTES),
            };
            entry.bytes.clear();
            return;
        }

        entry.bytes.extend_from_slice(bytes);
    }

    pub fn on_network_done(&mut self, url: &str, repaint: Option<RepaintHandle>) {
        let Some(id) = self.image_id_by_url.get(url).copied() else {
            return;
        };
        let Some(entry) = self.images.get_mut(&id) else {
            return;
        };

        if !matches!(entry.state, EntryState::Loading) {
            return;
        }

        let bytes = std::mem::take(&mut entry.bytes);
        entry.state = EntryState::Decoding;

        let tx = self.decode_done_tx.clone();
        let repaint = repaint.clone();
        std::thread::spawn(move || {
            let decoded = decode_image(bytes);
            let _ = tx.send(DecodeResult { id, decoded });
            if let Some(r) = repaint {
                r.request_now();
            }
        });
    }

    pub fn on_network_error(&mut self, url: &str, error: String) {
        if error == "cancelled" {
            self.forget_image(url);
            return;
        }

        let Some(id) = self.image_id_by_url.get(url).copied() else {
            return;
        };
        let Some(entry) = self.images.get_mut(&id) else {
            return;
        };

        entry.state = EntryState::Error { error };
        entry.bytes.clear();
    }

    /// Drains completed decode jobs and uploads textures via egui.
    /// Returns true if any resource state changed (useful to request repaint).
    pub fn pump(&mut self, egui_ctx: &egui::Context) -> bool {
        let mut changed = false;

        while let Ok(msg) = self.decode_done_rx.try_recv() {
            let Some(entry) = self.images.get_mut(&msg.id) else {
                continue;
            };

            match msg.decoded {
                Ok(decoded) => {
                    let size = decoded.size_px;
                    let pixel_count = size[0].saturating_mul(size[1]);
                    if pixel_count == 0 || pixel_count > MAX_IMAGE_PIXELS {
                        entry.state = EntryState::Error {
                            error: format!(
                                "decoded image size {}x{} is not supported",
                                size[0], size[1]
                            ),
                        };
                        changed = true;
                        continue;
                    }

                    let image = ColorImage::from_rgba_unmultiplied(size, &decoded.rgba);
                    let texture = egui_ctx.load_texture(
                        format!("img:{}", entry.url),
                        image,
                        TextureOptions::LINEAR,
                    );

                    entry.state = EntryState::Ready {
                        texture,
                        size_px: size,
                    };
                    changed = true;
                }
                Err(error) => {
                    entry.state = EntryState::Error { error };
                    changed = true;
                }
            }
        }

        changed
    }

    fn forget_image(&mut self, url: &str) {
        let Some(id) = self.image_id_by_url.remove(url) else {
            return;
        };
        self.images.remove(&id);
    }
}

impl Default for ResourceManager {
    fn default() -> Self {
        Self::new()
    }
}

fn decode_image(bytes: Vec<u8>) -> Result<DecodedImage, String> {
    if bytes.is_empty() {
        return Err("empty image response".to_string());
    }

    let img = image::load_from_memory(&bytes).map_err(|e| e.to_string())?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();

    Ok(DecodedImage {
        size_px: [w as usize, h as usize],
        rgba: rgba.into_raw(),
    })
}

impl ImageProvider for ResourceManager {
    fn image_state_by_url(&self, url: &str) -> gfx::paint::ImageState {
        match ResourceManager::image_state_by_url(self, url) {
            ImageState::Missing => gfx::paint::ImageState::Missing,
            ImageState::Loading { .. } => gfx::paint::ImageState::Loading,
            ImageState::Decoding { .. } => gfx::paint::ImageState::Decoding,
            ImageState::Ready(img) => gfx::paint::ImageState::Ready {
                texture_id: img.texture_id,
                size_px: img.size_px,
            },
            ImageState::Error { error, .. } => gfx::paint::ImageState::Error { error },
        }
    }

    fn image_intrinsic_size_px(&self, url: &str) -> Option<(u32, u32)> {
        ResourceManager::image_intrinsic_size_px(self, url)
    }
}

pub(crate) fn resolve_image_source(base_url: Option<&str>, source: &str) -> Option<String> {
    let source = strip_surrounding_html_ascii_whitespace(source);
    if source.is_empty() {
        return None;
    }
    if let Ok(parsed) = BrowserUrl::parse(source) {
        return Some(parsed.url.as_str().to_owned());
    }
    BrowserUrl::parse(base_url?)
        .ok()?
        .url
        .resolve(source)
        .ok()
        .map(|parsed| parsed.url.as_str().to_owned())
}

fn strip_surrounding_html_ascii_whitespace(value: &str) -> &str {
    value.trim_matches(|character| {
        matches!(
            character,
            '\u{0009}' | '\u{000A}' | '\u{000C}' | '\u{000D}' | '\u{0020}'
        )
    })
}

#[cfg(test)]
mod tests {
    use super::{
        EntryState, ImageState, ResourceManager, resolve_image_source,
        strip_surrounding_html_ascii_whitespace,
    };
    use tools::common::MAX_IMAGE_BYTES;

    #[test]
    fn browser_resource_provider_resolves_typed_image_sources() {
        assert_eq!(
            resolve_image_source(
                Some("https://example.com/docs/page.html"),
                "\t\n\u{000C}\r ../hero.png \t\n\u{000C}\r"
            )
            .as_deref(),
            Some("https://example.com/hero.png")
        );
        assert_eq!(
            resolve_image_source(
                Some("https://ignored.example/"),
                " \thttps://cdn.example/hero.png\r\n"
            )
            .as_deref(),
            Some("https://cdn.example/hero.png")
        );
        assert_eq!(resolve_image_source(Some("https://example.com/"), ""), None);
        assert_eq!(
            resolve_image_source(Some("https://example.com/"), " \t\n\u{000C}\r"),
            None
        );
        assert_eq!(resolve_image_source(None, "relative.png"), None);
        assert_eq!(
            resolve_image_source(
                Some("https://ignored.example/base/"),
                "https://cdn.example/hero.png"
            )
            .as_deref(),
            Some("https://cdn.example/hero.png")
        );
    }

    #[test]
    fn image_source_url_boundary_preserves_resolution_and_failure_behavior() {
        for (base, source, expected) in [
            (
                Some("https://example.test/docs/page.html"),
                "../images/logo.png",
                Some("https://example.test/images/logo.png"),
            ),
            (
                Some("invalid base"),
                "HTTP://EXAMPLE.test:80/a/../b?q#f",
                Some("http://example.test/b?q#f"),
            ),
            (Some("invalid base"), "relative.png", None),
            (None, "https://example.test:70000/a.png", None),
            (
                Some("https://example.test/page"),
                "https://example.test:70000/a.png",
                None,
            ),
            (
                Some("https://example.test/docs/page"),
                "//cdn.test:443/a.png",
                Some("https://cdn.test/a.png"),
            ),
            (Some("about:blank"), "relative.png", None),
            (Some("about:blank"), "#image", Some("about:blank#image")),
            (None, "file:///tmp/image.png", Some("file:///tmp/image.png")),
            (None, "custom:image", Some("custom:image")),
            (
                Some("https://u:p@example.test/a/"),
                "../b.png?x=1&x=2#f",
                Some("https://u:p@example.test/b.png?x=1&x=2#f"),
            ),
        ] {
            assert_eq!(resolve_image_source(base, source).as_deref(), expected);
        }
    }

    #[test]
    fn known_url_deviations_match_pre_ah2_image_resolution() {
        // Snapshot of the helper at master 7b2189c0. Literal observations below
        // are separate from the unchanged WHATWG expectations in addressing.
        fn pre_ah2(base_url: Option<&str>, source: &str) -> Option<String> {
            let source = strip_surrounding_html_ascii_whitespace(source);
            if source.is_empty() {
                return None;
            }
            if let Ok(url) = url::Url::parse(source) {
                return Some(url.to_string());
            }
            url::Url::parse(base_url?)
                .ok()?
                .join(source)
                .ok()
                .map(Into::into)
        }

        for (source, observed) in [
            ("file://example.net/C:/", Some("file:///C:/")),
            ("file:///w|/m", Some("file:///w|/m")),
            ("file://host//w|/m", Some("file://host/w|/m")),
            ("file:////w|/m", Some("file:///w|/m")),
            ("file://spider///", Some("file://spider/")),
            ("file:////foo", Some("file:///foo")),
            ("http://example.test/a^b", Some("http://example.test/a^b")),
            (
                "http://example.test/a%5Eb",
                Some("http://example.test/a%5Eb"),
            ),
            ("non-special:opaque  ?hi", Some("non-special:opaque  ?hi")),
            ("https://xn--/", None),
            ("a^b", Some("http://example.test/a^b")),
        ] {
            let base = Some("http://example.test/page.html");
            assert_eq!(
                pre_ah2(base, source).as_deref(),
                observed,
                "master: {source}"
            );
            assert_eq!(
                resolve_image_source(base, source).as_deref(),
                observed,
                "AH2: {source}"
            );
        }
    }

    #[test]
    fn image_source_preprocessing_strips_only_html_ascii_whitespace() {
        let non_ascii_whitespace = "\u{00A0}https://cdn.example/hero.png\u{00A0}";
        assert_eq!(
            strip_surrounding_html_ascii_whitespace(non_ascii_whitespace),
            non_ascii_whitespace
        );
        assert_eq!(resolve_image_source(None, non_ascii_whitespace), None);
    }

    #[test]
    fn resource_limit_error_discards_buffered_image_bytes_and_blocks_decode() {
        let mut resources = ResourceManager::new();
        let url = "https://example.com/large.png".to_string();
        let image_id = resources.request_image(url.clone(), |_| {});

        resources.on_network_chunk(&url, &[1, 2, 3, 4]);
        let entry = resources.images.get(&image_id).expect("image entry");
        assert_eq!(entry.bytes.len(), 4);
        assert!(matches!(entry.state, EntryState::Loading));

        resources.on_network_error(
            &url,
            format!(
                "image response exceeded byte limit of {} bytes",
                MAX_IMAGE_BYTES
            ),
        );

        let entry = resources
            .images
            .get(&image_id)
            .expect("image entry after error");
        assert!(entry.bytes.is_empty(), "buffered bytes should be discarded");
        assert!(matches!(
            resources.image_state(image_id),
            ImageState::Error { ref error, .. } if error.contains("exceeded byte limit")
        ));

        resources.on_network_done(&url, None);
        assert!(matches!(
            resources.image_state(image_id),
            ImageState::Error { ref error, .. } if error.contains("exceeded byte limit")
        ));
    }

    #[test]
    fn on_network_chunk_over_local_limit_transitions_to_error_and_clears_buffer() {
        let mut resources = ResourceManager::new();
        let url = "https://example.com/too-large.png".to_string();
        let image_id = resources.request_image(url.clone(), |_| {});

        resources.on_network_chunk(&url, &vec![0_u8; MAX_IMAGE_BYTES]);
        resources.on_network_chunk(&url, &[1]);

        let entry = resources.images.get(&image_id).expect("image entry");
        assert!(
            entry.bytes.is_empty(),
            "oversized image buffer should be cleared"
        );
        assert!(matches!(
            resources.image_state(image_id),
            ImageState::Error { ref error, .. } if error.contains("image too large")
        ));
    }
}
