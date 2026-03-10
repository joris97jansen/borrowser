use std::sync::atomic::AtomicU64;
use std::time::Instant;

use core_types::{DomHandle, DomVersion, RequestId, TabId};
use html::{DomPatch, Token, Tokenizer, TreeBuilder, TreeBuilderConfig};

pub(crate) static HANDLE_GEN: AtomicU64 = AtomicU64::new(0);

pub(crate) type Key = (TabId, RequestId);

pub(crate) struct HtmlState {
    pub(crate) total_bytes: usize,
    pub(crate) pending_bytes: usize,
    pub(crate) pending_tokens: usize,
    pub(crate) pending_patch_bytes: usize,
    pub(crate) last_emit: Instant,
    pub(crate) logged_large_buffer: bool,
    pub(crate) failed: bool,
    pub(crate) tokenizer: Tokenizer,
    pub(crate) builder: TreeBuilder,
    pub(crate) token_buffer: Vec<Token>,
    pub(crate) patch_buffer: Vec<DomPatch>,
    pub(crate) patch_buffer_retain: usize,
    pub(crate) max_patch_buffer_len: usize,
    pub(crate) max_patch_buffer_bytes: usize,
    pub(crate) dom_handle: DomHandle,
    pub(crate) version: DomVersion,
}

impl HtmlState {
    pub(crate) fn new(now: Instant, patch_buffer_retain: usize, dom_handle: DomHandle) -> Self {
        Self {
            total_bytes: 0,
            pending_bytes: 0,
            pending_tokens: 0,
            pending_patch_bytes: 0,
            last_emit: now,
            logged_large_buffer: false,
            failed: false,
            tokenizer: Tokenizer::new(),
            builder: TreeBuilder::with_capacity_and_config(0, TreeBuilderConfig::default()),
            token_buffer: Vec::new(),
            patch_buffer: Vec::new(),
            patch_buffer_retain,
            max_patch_buffer_len: 0,
            max_patch_buffer_bytes: 0,
            dom_handle,
            version: DomVersion::INITIAL,
        }
    }
}

#[cfg(feature = "html5")]
pub(crate) struct Html5State {
    pub(crate) total_bytes: usize,
    pub(crate) pending_bytes: usize,
    /// Token counts observed from the HTML5 session since last flush.
    pub(crate) pending_tokens: usize,
    pub(crate) pending_patch_bytes: usize,
    pub(crate) last_tokens_processed: u64,
    pub(crate) last_emit: Instant,
    pub(crate) logged_large_buffer: bool,
    pub(crate) failed: bool,
    pub(crate) session: html::html5::Html5ParseSession,
    pub(crate) patch_buffer: Vec<DomPatch>,
    pub(crate) patch_buffer_retain: usize,
    pub(crate) max_patch_buffer_len: usize,
    pub(crate) max_patch_buffer_bytes: usize,
    pub(crate) dom_handle: DomHandle,
    pub(crate) version: DomVersion,
}

#[cfg(feature = "html5")]
impl Html5State {
    pub(crate) fn new(
        now: Instant,
        patch_buffer_retain: usize,
        dom_handle: DomHandle,
        session: html::html5::Html5ParseSession,
    ) -> Self {
        Self {
            total_bytes: 0,
            pending_bytes: 0,
            pending_tokens: 0,
            pending_patch_bytes: 0,
            last_tokens_processed: 0,
            last_emit: now,
            logged_large_buffer: false,
            failed: false,
            session,
            patch_buffer: Vec::new(),
            patch_buffer_retain,
            max_patch_buffer_len: 0,
            max_patch_buffer_bytes: 0,
            dom_handle,
            version: DomVersion::INITIAL,
        }
    }
}

pub(crate) enum RuntimeState {
    Legacy(Box<HtmlState>),
    #[cfg(feature = "html5")]
    Html5(Box<Html5State>),
}
