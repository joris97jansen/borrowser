use std::collections::HashMap;
use std::sync::mpsc;

use app_api::RepaintHandle;
use egui::{ColorImage, TextureHandle, TextureId, TextureOptions};
use gfx::paint::ImageProvider;

const MAX_IMAGE_BYTES: usize = 20 * 1024 * 1024;
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
