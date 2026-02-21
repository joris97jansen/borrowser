use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use bus::{CoreCommand, CoreEvent};
use core_types::{DomHandle, DomVersion, RequestId, TabId};
use html::{DomPatch, Token, Tokenizer, TreeBuilder, TreeBuilderConfig};
#[cfg(feature = "patch-stats")]
use log::info;
use log::{error, warn};

#[cfg(test)]
use html::internal::Id;
#[cfg(test)]
use html::{Node, PatchKey};
#[cfg(test)]
use std::collections::HashSet;
#[cfg(test)]
use std::sync::Arc;
const DEFAULT_TICK: Duration = Duration::from_millis(180);
const DEBUG_LARGE_BUFFER_BYTES: usize = 1_048_576;
// Cap retained patch buffer capacity to avoid pinning memory after a spike.
const MAX_PATCH_BUFFER_RETAIN: usize = 4_096;
const MIN_PATCH_BUFFER_RETAIN: usize = 256;
const EST_PATCH_BYTES: usize = 64;
static HANDLE_GEN: AtomicU64 = AtomicU64::new(0);

const HTML_PARSER_ENV: &str = "BORROWSER_HTML_PARSER";
const PARSER_MODE_LEGACY: &str = "legacy";
const PARSER_MODE_HTML5: &str = "html5";

/// Runtime parser mode selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ParserMode {
    Legacy,
    Html5,
}

fn parse_runtime_parser_mode(value: Option<&str>) -> Option<ParserMode> {
    let value = value?.trim().to_ascii_lowercase();
    match value.as_str() {
        PARSER_MODE_LEGACY => Some(ParserMode::Legacy),
        PARSER_MODE_HTML5 => Some(ParserMode::Html5),
        _ => None,
    }
}

fn parser_mode_from_env_with<F>(get_env: F) -> ParserMode
where
    F: Fn(&str) -> Option<String>,
{
    match get_env(HTML_PARSER_ENV) {
        Some(value) => match parse_runtime_parser_mode(Some(&value)) {
            Some(mode) => mode,
            None => {
                warn!(
                    target: "runtime_parse",
                    "unsupported parser mode '{value}' (env {HTML_PARSER_ENV}); defaulting to legacy"
                );
                ParserMode::Legacy
            }
        },
        None => ParserMode::Legacy,
    }
}

fn parser_mode_from_env() -> ParserMode {
    parser_mode_from_env_with(|key| std::env::var(key).ok())
}

fn resolve_parser_mode(requested: ParserMode) -> ParserMode {
    #[cfg(feature = "html5")]
    {
        requested
    }
    #[cfg(not(feature = "html5"))]
    {
        match requested {
            ParserMode::Legacy => ParserMode::Legacy,
            ParserMode::Html5 => {
                warn!(
                    target: "runtime_parse",
                    "html5 parser requested (env {HTML_PARSER_ENV}) but feature not enabled; defaulting to legacy"
                );
                ParserMode::Legacy
            }
        }
    }
}

/// Retain capacity for patch vectors to reduce allocator churn while capping memory.
fn patch_buffer_retain_target(
    patch_threshold: Option<usize>,
    patch_byte_threshold: Option<usize>,
) -> usize {
    let base = if let Some(threshold) = patch_threshold {
        threshold.saturating_mul(2).max(MIN_PATCH_BUFFER_RETAIN)
    } else if let Some(bytes) = patch_byte_threshold {
        // Approximate patch count from bytes; conservative to avoid churn.
        (bytes / EST_PATCH_BYTES).max(MIN_PATCH_BUFFER_RETAIN)
    } else {
        MIN_PATCH_BUFFER_RETAIN
    };
    base.min(MAX_PATCH_BUFFER_RETAIN)
}

/// Preview flush strategy for incremental parse.
///
/// The policy is evaluated when new input arrives. A flush occurs if any enabled
/// threshold is met and there are pending patches. Thresholds include time, input
/// tokens/bytes, and buffered patch count/bytes. Time-based checks are activity
/// driven: ticks are evaluated only when input arrives (no background timer). If
/// input stalls, pending patches are flushed on `ParseHtmlDone`. Boundedness
/// assumes continued input or an eventual `ParseHtmlDone`.
#[derive(Clone, Copy, Debug)]
pub struct PreviewPolicy {
    pub tick: Duration,
    pub token_threshold: Option<usize>,
    pub byte_threshold: Option<usize>,
    pub patch_threshold: Option<usize>,
    pub patch_byte_threshold: Option<usize>,
}

impl Default for PreviewPolicy {
    fn default() -> Self {
        Self {
            tick: DEFAULT_TICK,
            token_threshold: None,
            byte_threshold: None,
            patch_threshold: None,
            patch_byte_threshold: None,
        }
    }
}

impl PreviewPolicy {
    fn is_bounded(&self) -> bool {
        self.tick != Duration::ZERO
            || self.token_threshold.is_some()
            || self.byte_threshold.is_some()
            || self.patch_threshold.is_some()
            || self.patch_byte_threshold.is_some()
    }

    fn ensure_bounded(mut self) -> Self {
        if !self.is_bounded() {
            self.tick = DEFAULT_TICK;
        }
        self
    }

    fn should_flush(
        &self,
        elapsed: Duration,
        pending_tokens: usize,
        pending_bytes: usize,
        pending_patches: usize,
        pending_patch_bytes: usize,
    ) -> bool {
        if pending_patches == 0 {
            return false;
        }
        // tick == 0 disables time-based flushing.
        if self.tick != Duration::ZERO && elapsed >= self.tick {
            return true;
        }
        if let Some(threshold) = self.token_threshold
            && pending_tokens >= threshold
        {
            return true;
        }
        if let Some(threshold) = self.byte_threshold
            && pending_bytes >= threshold
        {
            return true;
        }
        if let Some(threshold) = self.patch_threshold
            && pending_patches >= threshold
        {
            return true;
        }
        if let Some(threshold) = self.patch_byte_threshold
            && pending_patch_bytes >= threshold
        {
            return true;
        }
        false
    }
}

trait PreviewClock: Send {
    fn now(&self) -> Instant;
}

struct SystemClock;

impl PreviewClock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

struct HtmlState {
    total_bytes: usize,
    pending_bytes: usize,
    pending_tokens: usize,
    pending_patch_bytes: usize,
    last_emit: Instant,
    logged_large_buffer: bool,
    failed: bool,
    tokenizer: Tokenizer,
    builder: TreeBuilder,
    token_buffer: Vec<Token>,
    patch_buffer: Vec<DomPatch>,
    patch_buffer_retain: usize,
    max_patch_buffer_len: usize,
    max_patch_buffer_bytes: usize,
    dom_handle: DomHandle,
    version: DomVersion,
}

#[cfg(feature = "html5")]
struct Html5State {
    total_bytes: usize,
    pending_bytes: usize,
    /// Token counts observed from the HTML5 session since last flush.
    pending_tokens: usize,
    pending_patch_bytes: usize,
    last_tokens_processed: u64,
    last_emit: Instant,
    logged_large_buffer: bool,
    failed: bool,
    session: html::html5::Html5ParseSession,
    patch_buffer: Vec<DomPatch>,
    patch_buffer_retain: usize,
    max_patch_buffer_len: usize,
    max_patch_buffer_bytes: usize,
    dom_handle: DomHandle,
    version: DomVersion,
}

#[cfg(feature = "html5")]
fn log_html5_session_error(
    tab_id: TabId,
    request_id: RequestId,
    err: html::html5::Html5SessionError,
) {
    match err {
        html::html5::Html5SessionError::Decode => {
            error!(
                target: "runtime_parse",
                "html5 decode error tab={tab_id:?} request={request_id:?}"
            );
        }
        html::html5::Html5SessionError::Invariant => {
            error!(
                target: "runtime_parse",
                "html5 invariant error tab={tab_id:?} request={request_id:?}"
            );
        }
    }
}

#[cfg(feature = "html5")]
impl Html5State {
    fn drain_patches(&mut self) {
        let new_patches = self.session.take_patches();
        if !new_patches.is_empty() {
            self.pending_patch_bytes = self
                .pending_patch_bytes
                .saturating_add(estimate_patch_bytes_slice(&new_patches));
            self.patch_buffer.extend(new_patches);
            self.update_patch_buffer_max();
        }
    }

    fn flush_patch_buffer(
        &mut self,
        evt_tx: &Sender<CoreEvent>,
        tab_id: TabId,
        request_id: RequestId,
    ) {
        if self.patch_buffer.is_empty() {
            return;
        }
        let patches = std::mem::replace(
            &mut self.patch_buffer,
            Vec::with_capacity(self.patch_buffer_retain),
        );

        #[cfg(feature = "patch-stats")]
        log_patch_stats(tab_id, request_id, &patches);

        let ok = emit_patch_update(
            evt_tx,
            tab_id,
            request_id,
            self.dom_handle,
            &mut self.version,
            patches,
        )
        .is_ok();
        self.reset_pending();
        if !ok {
            self.failed = true;
        }
    }

    fn update_patch_buffer_max(&mut self) {
        let len = self.patch_buffer.len();
        if len > self.max_patch_buffer_len {
            self.max_patch_buffer_len = len;
        }
        if self.pending_patch_bytes > self.max_patch_buffer_bytes {
            self.max_patch_buffer_bytes = self.pending_patch_bytes;
        }
    }

    fn reset_pending(&mut self) {
        self.pending_bytes = 0;
        self.pending_tokens = 0;
        self.pending_patch_bytes = 0;
    }

    fn update_pending_tokens(&mut self) {
        let total = self.session.tokens_processed();
        let delta = total.saturating_sub(self.last_tokens_processed);
        self.last_tokens_processed = total;
        self.pending_tokens = self.pending_tokens.saturating_add(delta as usize);
    }
}

enum RuntimeState {
    Legacy(Box<HtmlState>),
    #[cfg(feature = "html5")]
    Html5(Box<Html5State>),
}
type Key = (TabId, RequestId);

impl HtmlState {
    fn drain_tokens_into_builder(&mut self) {
        if self.token_buffer.is_empty() {
            return;
        }
        let atoms = self.tokenizer.atoms();
        let drained = std::mem::take(&mut self.token_buffer);
        for token in drained {
            if let Err(err) = self.builder.push_token(&token, atoms, &self.tokenizer) {
                error!(target: "runtime_parse", "tree builder error: {err}");
                self.failed = true;
                self.patch_buffer.clear();
                self.reset_pending();
                break;
            }
        }
        if self.failed {
            let _ = self.builder.take_patches();
            return;
        }
        let new_patches = self.builder.take_patches();
        if !new_patches.is_empty() {
            self.pending_patch_bytes = self
                .pending_patch_bytes
                .saturating_add(estimate_patch_bytes_slice(&new_patches));
            self.patch_buffer.extend(new_patches);
            self.update_patch_buffer_max();
        }
    }

    fn flush_patch_buffer(
        &mut self,
        evt_tx: &Sender<CoreEvent>,
        tab_id: TabId,
        request_id: RequestId,
    ) {
        if self.patch_buffer.is_empty() {
            return;
        }
        let patches = std::mem::replace(
            &mut self.patch_buffer,
            Vec::with_capacity(self.patch_buffer_retain),
        );

        #[cfg(feature = "patch-stats")]
        log_patch_stats(tab_id, request_id, &patches);

        let ok = emit_patch_update(
            evt_tx,
            tab_id,
            request_id,
            self.dom_handle,
            &mut self.version,
            patches,
        )
        .is_ok();
        self.reset_pending();
        if !ok {
            self.failed = true;
        }
    }

    fn update_patch_buffer_max(&mut self) {
        let len = self.patch_buffer.len();
        if len > self.max_patch_buffer_len {
            self.max_patch_buffer_len = len;
        }
        if self.pending_patch_bytes > self.max_patch_buffer_bytes {
            self.max_patch_buffer_bytes = self.pending_patch_bytes;
        }
    }

    fn reset_pending(&mut self) {
        self.pending_bytes = 0;
        self.pending_tokens = 0;
        self.pending_patch_bytes = 0;
    }
}

fn emit_patch_update(
    evt_tx: &Sender<CoreEvent>,
    tab_id: TabId,
    request_id: RequestId,
    dom_handle: DomHandle,
    version: &mut DomVersion,
    patches: Vec<DomPatch>,
) -> Result<(), std::sync::mpsc::SendError<CoreEvent>> {
    let from = *version;
    let to = from.next();
    let send_result = evt_tx.send(CoreEvent::DomPatchUpdate {
        tab_id,
        request_id,
        handle: dom_handle,
        from,
        to,
        patches,
    });
    if let Err(err) = send_result {
        error!(
            target: "runtime_parse",
            "patch sink dropped; stopping updates for tab={tab_id:?} request={request_id:?}"
        );
        return Err(err);
    }
    *version = to;
    Ok(())
}

fn estimate_patch_bytes(patch: &DomPatch) -> usize {
    const PATCH_OVERHEAD: usize = 8;
    match patch {
        DomPatch::Clear => PATCH_OVERHEAD,
        DomPatch::CreateDocument { doctype, .. } => {
            PATCH_OVERHEAD + doctype.as_ref().map(|s| s.len()).unwrap_or(0)
        }
        DomPatch::CreateElement {
            name, attributes, ..
        } => {
            let mut total = PATCH_OVERHEAD + name.len();
            for (k, v) in attributes {
                total += k.len();
                if let Some(value) = v {
                    total += value.len();
                }
            }
            total
        }
        DomPatch::CreateText { text, .. } | DomPatch::CreateComment { text, .. } => {
            PATCH_OVERHEAD + text.len()
        }
        DomPatch::AppendChild { .. }
        | DomPatch::InsertBefore { .. }
        | DomPatch::RemoveNode { .. }
        | DomPatch::SetAttributes { .. }
        | DomPatch::SetText { .. } => PATCH_OVERHEAD,
        // NOTE: DomPatch may grow new variants; default to PATCH_OVERHEAD for unknown ones.
        _ => PATCH_OVERHEAD,
    }
}

fn estimate_patch_bytes_slice(patches: &[DomPatch]) -> usize {
    patches.iter().fold(0usize, |total, patch| {
        total.saturating_add(estimate_patch_bytes(patch))
    })
}

#[cfg(feature = "patch-stats")]
fn log_patch_stats(tab_id: TabId, request_id: RequestId, patches: &[DomPatch]) {
    let mut created = 0usize;
    let mut removed = 0usize;
    for patch in patches {
        match patch {
            DomPatch::CreateDocument { .. }
            | DomPatch::CreateElement { .. }
            | DomPatch::CreateText { .. }
            | DomPatch::CreateComment { .. } => {
                created += 1;
            }
            DomPatch::RemoveNode { .. } => {
                removed += 1;
            }
            _ => {}
        }
    }
    info!(
        target: "runtime_parse",
        "patch_stats tab={tab_id:?} request={request_id:?} patches={} created={} removed={}",
        patches.len(),
        created,
        removed
    );
}

/// Parses HTML incrementally for streaming previews.
///
/// Runtime parser selection can be controlled via the `BORROWSER_HTML_PARSER`
/// environment variable (`legacy` | `html5`). When unset or invalid, legacy
/// parsing is used. The `html5` mode requires the `runtime_parse/html5` feature.
/// Mode is resolved once per runtime thread to avoid mixed-parser state.
/// Token-threshold flushing is supported for HTML5 via session token counters.
///
/// Patch emission is buffered and flushed on ticks; the tokenizer and tree builder
/// retain state between chunks so work is proportional to new input.
pub fn start_parse_runtime(cmd_rx: Receiver<CoreCommand>, evt_tx: Sender<CoreEvent>) {
    start_parse_runtime_with_policy(cmd_rx, evt_tx, PreviewPolicy::default())
}

pub fn start_parse_runtime_with_policy(
    cmd_rx: Receiver<CoreCommand>,
    evt_tx: Sender<CoreEvent>,
    policy: PreviewPolicy,
) {
    let policy = policy.ensure_bounded();
    start_parse_runtime_with_policy_and_clock(cmd_rx, evt_tx, policy, SystemClock)
}

fn start_parse_runtime_with_policy_and_clock<C: PreviewClock + 'static>(
    cmd_rx: Receiver<CoreCommand>,
    evt_tx: Sender<CoreEvent>,
    policy: PreviewPolicy,
    clock: C,
) {
    let mode = resolve_parser_mode(parser_mode_from_env());
    start_parse_runtime_with_policy_and_clock_and_mode(cmd_rx, evt_tx, policy, clock, mode);
}

fn start_parse_runtime_with_policy_and_clock_and_mode<C: PreviewClock + 'static>(
    cmd_rx: Receiver<CoreCommand>,
    evt_tx: Sender<CoreEvent>,
    policy: PreviewPolicy,
    clock: C,
    mode: ParserMode,
) {
    thread::spawn(move || {
        // Mode is chosen once per runtime thread to keep behavior deterministic.
        let patch_buffer_retain =
            patch_buffer_retain_target(policy.patch_threshold, policy.patch_byte_threshold);
        let mut htmls: HashMap<Key, RuntimeState> = HashMap::new();

        while let Ok(cmd) = cmd_rx.recv() {
            let now = clock.now();
            match cmd {
                CoreCommand::ParseHtmlStart { tab_id, request_id } => {
                    // DomHandle is per-runtime unique today; future: global allocator.
                    let prev = match HANDLE_GEN.fetch_update(
                        Ordering::Relaxed,
                        Ordering::Relaxed,
                        |v| v.checked_add(1),
                    ) {
                        Ok(prev) => prev,
                        Err(_) => {
                            error!(
                                target: "runtime_parse",
                                "dom handle overflow; dropping ParseHtmlStart tab={tab_id:?} request={request_id:?}"
                            );
                            continue;
                        }
                    };
                    let next = prev + 1;
                    let dom_handle = DomHandle(next);
                    let state = match mode {
                        ParserMode::Legacy => RuntimeState::Legacy(Box::new(HtmlState {
                            total_bytes: 0,
                            pending_bytes: 0,
                            pending_tokens: 0,
                            pending_patch_bytes: 0,
                            last_emit: now,
                            logged_large_buffer: false,
                            failed: false,
                            tokenizer: Tokenizer::new(),
                            builder: TreeBuilder::with_capacity_and_config(
                                0,
                                TreeBuilderConfig::default(),
                            ),
                            token_buffer: Vec::new(),
                            patch_buffer: Vec::new(),
                            patch_buffer_retain,
                            max_patch_buffer_len: 0,
                            max_patch_buffer_bytes: 0,
                            dom_handle,
                            version: DomVersion::INITIAL,
                        })),
                        ParserMode::Html5 => {
                            #[cfg(feature = "html5")]
                            {
                                let ctx = html::html5::DocumentParseContext::new();
                                let session = match html::html5::Html5ParseSession::new(
                                    html::html5::TokenizerConfig::default(),
                                    html::html5::TreeBuilderConfig::default(),
                                    ctx,
                                ) {
                                    Ok(session) => session,
                                    Err(err) => {
                                        error!(
                                            target: "runtime_parse",
                                            "failed to initialize html5 parse session: {err:?}"
                                        );
                                        continue;
                                    }
                                };
                                RuntimeState::Html5(Box::new(Html5State {
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
                                }))
                            }
                            #[cfg(not(feature = "html5"))]
                            {
                                unreachable!(
                                    "resolve_parser_mode prevents Html5 when feature is disabled"
                                );
                            }
                        }
                    };
                    htmls.insert((tab_id, request_id), state);
                }
                CoreCommand::ParseHtmlChunk {
                    tab_id,
                    request_id,
                    bytes,
                } => {
                    let mut remove_state = false;
                    if let Some(state) = htmls.get_mut(&(tab_id, request_id)) {
                        match state {
                            RuntimeState::Legacy(st) => {
                                if st.failed {
                                    // failed means terminal; drop state to free memory.
                                    remove_state = true;
                                } else {
                                    st.total_bytes = st.total_bytes.saturating_add(bytes.len());
                                    st.pending_bytes = st.pending_bytes.saturating_add(bytes.len());
                                    st.pending_tokens =
                                        st.pending_tokens.saturating_add(st.tokenizer.feed(&bytes));
                                    // Always drain tokenizer tokens immediately so flush decisions
                                    // reflect patch backlog, not buffered tokens.
                                    st.tokenizer.drain_into(&mut st.token_buffer);
                                    st.drain_tokens_into_builder();

                                    if policy.should_flush(
                                        now.saturating_duration_since(st.last_emit),
                                        st.pending_tokens,
                                        st.pending_bytes,
                                        st.patch_buffer.len(),
                                        st.pending_patch_bytes,
                                    ) {
                                        st.last_emit = now;
                                        #[cfg(debug_assertions)]
                                        {
                                            if !st.logged_large_buffer
                                                && st.total_bytes >= DEBUG_LARGE_BUFFER_BYTES
                                            {
                                                warn!(
                                                    target: "runtime_parse",
                                                    "large buffer ({} bytes), incremental parse active",
                                                    st.total_bytes
                                                );
                                                st.logged_large_buffer = true;
                                            }
                                        }
                                        st.flush_patch_buffer(&evt_tx, tab_id, request_id);
                                        if st.failed {
                                            remove_state = true;
                                        }
                                    }
                                }
                            }
                            #[cfg(feature = "html5")]
                            RuntimeState::Html5(st) => {
                                if st.failed {
                                    remove_state = true;
                                } else {
                                    st.total_bytes = st.total_bytes.saturating_add(bytes.len());
                                    st.pending_bytes = st.pending_bytes.saturating_add(bytes.len());
                                    // pending_tokens accumulates observed session tokens since last flush.
                                    if let Err(err) = st.session.push_bytes(&bytes) {
                                        log_html5_session_error(tab_id, request_id, err);
                                        // Decode errors stop further processing; any already-produced
                                        // patches may still flush on ParseHtmlDone.
                                        st.failed = true;
                                        st.reset_pending();
                                    } else if let Err(err) = st.session.pump() {
                                        log_html5_session_error(tab_id, request_id, err);
                                        st.failed = true;
                                        st.reset_pending();
                                    } else {
                                        st.update_pending_tokens();
                                        st.drain_patches();
                                    }

                                    if policy.should_flush(
                                        now.saturating_duration_since(st.last_emit),
                                        st.pending_tokens,
                                        st.pending_bytes,
                                        st.patch_buffer.len(),
                                        st.pending_patch_bytes,
                                    ) {
                                        st.last_emit = now;
                                        #[cfg(debug_assertions)]
                                        {
                                            if !st.logged_large_buffer
                                                && st.total_bytes >= DEBUG_LARGE_BUFFER_BYTES
                                            {
                                                warn!(
                                                    target: "runtime_parse",
                                                    "large buffer ({} bytes), incremental parse active",
                                                    st.total_bytes
                                                );
                                                st.logged_large_buffer = true;
                                            }
                                        }
                                        st.flush_patch_buffer(&evt_tx, tab_id, request_id);
                                        if st.failed {
                                            remove_state = true;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if remove_state {
                        htmls.remove(&(tab_id, request_id));
                    }
                }
                CoreCommand::ParseHtmlDone { tab_id, request_id } => {
                    if let Some(state) = htmls.remove(&(tab_id, request_id)) {
                        match state {
                            RuntimeState::Legacy(mut st) => {
                                if st.failed {
                                    continue;
                                }
                                st.pending_tokens =
                                    st.pending_tokens.saturating_add(st.tokenizer.finish());
                                st.tokenizer.drain_into(&mut st.token_buffer);
                                st.drain_tokens_into_builder();
                                if let Err(err) = st.builder.finish() {
                                    error!(target: "runtime_parse", "tree builder finish error: {err}");
                                }
                                let final_patches = st.builder.take_patches();
                                if !final_patches.is_empty() {
                                    st.pending_patch_bytes = st
                                        .pending_patch_bytes
                                        .saturating_add(estimate_patch_bytes_slice(&final_patches));
                                    st.patch_buffer.extend(final_patches);
                                    st.update_patch_buffer_max();
                                }
                                st.flush_patch_buffer(&evt_tx, tab_id, request_id);
                            }
                            #[cfg(feature = "html5")]
                            RuntimeState::Html5(mut st) => {
                                if st.failed {
                                    st.flush_patch_buffer(&evt_tx, tab_id, request_id);
                                    continue;
                                }
                                if let Err(err) = st.session.pump() {
                                    log_html5_session_error(tab_id, request_id, err);
                                    if matches!(err, html::html5::Html5SessionError::Decode) {
                                        st.update_pending_tokens();
                                        st.drain_patches();
                                        st.flush_patch_buffer(&evt_tx, tab_id, request_id);
                                    }
                                    st.failed = true;
                                    st.reset_pending();
                                    continue;
                                }
                                st.update_pending_tokens();
                                st.drain_patches();
                                st.flush_patch_buffer(&evt_tx, tab_id, request_id);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    });
}

#[cfg(test)]
#[derive(Debug)]
struct PatchState {
    id_to_key: HashMap<Id, PatchKey>,
    next_key: u32,
}

#[cfg(test)]
impl PatchState {
    fn new() -> Self {
        Self {
            id_to_key: HashMap::new(),
            next_key: 0,
        }
    }

    fn allocate_key(&mut self) -> Option<PatchKey> {
        self.next_key = self.next_key.checked_add(1)?;
        Some(PatchKey(self.next_key))
    }

    fn assign_key(&mut self, id: Id) -> Option<PatchKey> {
        if let Some(existing) = self.id_to_key.get(&id) {
            return Some(*existing);
        }
        let key = self.allocate_key()?;
        self.id_to_key.insert(id, key);
        Some(key)
    }
}

#[cfg(test)]
#[derive(Clone, Debug)]
enum PrevNodeInfo {
    Document {
        doctype: Option<String>,
        children: Vec<Id>,
    },
    Element {
        name: Arc<str>,
        attributes: Vec<(Arc<str>, Option<String>)>,
        children: Vec<Id>,
    },
    Text {
        text: String,
    },
    Comment {
        text: String,
    },
}

#[cfg(test)]
fn diff_dom(prev: &Node, next: &Node, patch_state: &mut PatchState) -> Option<Vec<DomPatch>> {
    let mut prev_map = HashMap::new();
    build_prev_map(prev, &mut prev_map);
    let mut next_ids = HashSet::new();
    collect_ids(next, &mut next_ids);

    let mut patches = Vec::new();
    let mut removed_ids = HashSet::new();
    let mut need_reset = !root_is_compatible(prev, next);

    if !need_reset {
        emit_removals(
            prev,
            &next_ids,
            patch_state,
            &mut patches,
            &mut removed_ids,
            &mut need_reset,
        );
    }

    if !need_reset {
        emit_updates(
            next,
            None,
            &prev_map,
            &next_ids,
            patch_state,
            &mut patches,
            &mut need_reset,
        );
    }

    if need_reset {
        patches.clear();
        // Reset emits a fresh create stream without relying on RemoveNode;
        // applier state may be out of sync and should tolerate replacement.
        patch_state.id_to_key.clear();
        patches.push(DomPatch::Clear);
        emit_create_subtree(next, None, patch_state, &mut patches, &mut need_reset);
        if need_reset {
            patches.clear();
            error!(target: "runtime_parse", "failed to emit reset patch stream; dropping update");
            return None;
        }
        return Some(patches);
    }

    for removed in removed_ids {
        patch_state.id_to_key.remove(&removed);
    }

    Some(patches)
}

#[cfg(test)]
fn build_prev_map(node: &Node, map: &mut HashMap<Id, PrevNodeInfo>) {
    match node {
        Node::Document {
            id,
            doctype,
            children,
        } => {
            map.insert(
                *id,
                PrevNodeInfo::Document {
                    doctype: doctype.clone(),
                    children: children.iter().map(Node::id).collect(),
                },
            );
            for child in children {
                build_prev_map(child, map);
            }
        }
        Node::Element {
            id,
            name,
            attributes,
            children,
            ..
        } => {
            map.insert(
                *id,
                PrevNodeInfo::Element {
                    name: Arc::clone(name),
                    attributes: attributes.clone(),
                    children: children.iter().map(Node::id).collect(),
                },
            );
            for child in children {
                build_prev_map(child, map);
            }
        }
        Node::Text { id, text } => {
            map.insert(*id, PrevNodeInfo::Text { text: text.clone() });
        }
        Node::Comment { id, text } => {
            map.insert(*id, PrevNodeInfo::Comment { text: text.clone() });
        }
    }
}

#[cfg(test)]
fn collect_ids(node: &Node, out: &mut HashSet<Id>) {
    out.insert(node.id());
    match node {
        Node::Document { children, .. } | Node::Element { children, .. } => {
            for child in children {
                collect_ids(child, out);
            }
        }
        Node::Text { .. } | Node::Comment { .. } => {}
    }
}

#[cfg(test)]
fn emit_removals(
    node: &Node,
    next_ids: &HashSet<Id>,
    patch_state: &PatchState,
    patches: &mut Vec<DomPatch>,
    removed_ids: &mut HashSet<Id>,
    need_reset: &mut bool,
) {
    if !next_ids.contains(&node.id()) {
        if let Some(key) = patch_state.id_to_key.get(&node.id()).copied() {
            patches.push(DomPatch::RemoveNode { key });
        } else {
            *need_reset = true;
            return;
        }
        collect_ids(node, removed_ids);
        return;
    }
    match node {
        Node::Document { children, .. } | Node::Element { children, .. } => {
            for child in children {
                emit_removals(
                    child,
                    next_ids,
                    patch_state,
                    patches,
                    removed_ids,
                    need_reset,
                );
                if *need_reset {
                    return;
                }
            }
        }
        Node::Text { .. } | Node::Comment { .. } => {}
    }
}

#[cfg(test)]
fn emit_updates(
    node: &Node,
    parent_key: Option<PatchKey>,
    prev_map: &HashMap<Id, PrevNodeInfo>,
    next_ids: &HashSet<Id>,
    patch_state: &mut PatchState,
    patches: &mut Vec<DomPatch>,
    need_reset: &mut bool,
) {
    let id = node.id();
    let is_new = !prev_map.contains_key(&id);
    let key = if is_new {
        match patch_state.assign_key(id) {
            Some(key) => key,
            None => {
                *need_reset = true;
                return;
            }
        }
    } else if let Some(key) = patch_state.id_to_key.get(&id).copied() {
        key
    } else {
        *need_reset = true;
        return;
    };

    if is_new {
        emit_create_node(node, key, patches);
        if let Some(parent_key) = parent_key {
            patches.push(DomPatch::AppendChild {
                parent: parent_key,
                child: key,
            });
        }
    } else if let Some(prev) = prev_map.get(&id) {
        match (prev, node) {
            (
                PrevNodeInfo::Document { doctype, .. },
                Node::Document {
                    doctype: next_doctype,
                    ..
                },
            ) => {
                if doctype != next_doctype {
                    *need_reset = true;
                    return;
                }
            }
            (
                PrevNodeInfo::Element {
                    name, attributes, ..
                },
                Node::Element {
                    name: next_name,
                    attributes: next_attrs,
                    ..
                },
            ) => {
                if name != next_name {
                    *need_reset = true;
                    return;
                }
                if attributes != next_attrs {
                    patches.push(DomPatch::SetAttributes {
                        key,
                        attributes: next_attrs.clone(),
                    });
                }
            }
            (
                PrevNodeInfo::Text { text },
                Node::Text {
                    text: next_text, ..
                },
            ) => {
                if text != next_text {
                    patches.push(DomPatch::SetText {
                        key,
                        text: next_text.clone(),
                    });
                }
            }
            (
                PrevNodeInfo::Comment { text },
                Node::Comment {
                    text: next_text, ..
                },
            ) => {
                if text != next_text {
                    *need_reset = true;
                    return;
                }
            }
            _ => {
                *need_reset = true;
                return;
            }
        }
    }

    match node {
        Node::Document { children, .. } | Node::Element { children, .. } => {
            if !is_new {
                let prev_children_live = match prev_map.get(&id) {
                    Some(PrevNodeInfo::Document { children, .. })
                    | Some(PrevNodeInfo::Element { children, .. }) => children
                        .iter()
                        .copied()
                        .filter(|child| next_ids.contains(child))
                        .collect::<Vec<_>>(),
                    _ => Vec::new(),
                };
                let next_children = children.iter().map(Node::id).collect::<Vec<_>>();
                if next_children.len() < prev_children_live.len() {
                    *need_reset = true;
                    return;
                }
                if next_children[..prev_children_live.len()] != prev_children_live[..] {
                    *need_reset = true;
                    return;
                }
            }
            for child in children {
                emit_updates(
                    child,
                    Some(key),
                    prev_map,
                    next_ids,
                    patch_state,
                    patches,
                    need_reset,
                );
                if *need_reset {
                    return;
                }
            }
        }
        Node::Text { .. } | Node::Comment { .. } => {}
    }
}

#[cfg(test)]
fn emit_create_node(node: &Node, key: PatchKey, patches: &mut Vec<DomPatch>) {
    match node {
        Node::Document { doctype, .. } => {
            patches.push(DomPatch::CreateDocument {
                key,
                doctype: doctype.clone(),
            });
        }
        Node::Element {
            name, attributes, ..
        } => {
            patches.push(DomPatch::CreateElement {
                key,
                name: Arc::clone(name),
                attributes: attributes.clone(),
            });
        }
        Node::Text { text, .. } => {
            patches.push(DomPatch::CreateText {
                key,
                text: text.clone(),
            });
        }
        Node::Comment { text, .. } => {
            patches.push(DomPatch::CreateComment {
                key,
                text: text.clone(),
            });
        }
    }
}

#[cfg(test)]
fn emit_create_subtree(
    node: &Node,
    parent_key: Option<PatchKey>,
    patch_state: &mut PatchState,
    patches: &mut Vec<DomPatch>,
    need_reset: &mut bool,
) {
    let Some(key) = patch_state.assign_key(node.id()) else {
        *need_reset = true;
        return;
    };
    emit_create_node(node, key, patches);
    if let Some(parent_key) = parent_key {
        patches.push(DomPatch::AppendChild {
            parent: parent_key,
            child: key,
        });
    }
    match node {
        Node::Document { children, .. } | Node::Element { children, .. } => {
            for child in children {
                emit_create_subtree(child, Some(key), patch_state, patches, need_reset);
                if *need_reset {
                    return;
                }
            }
        }
        Node::Text { .. } | Node::Comment { .. } => {}
    }
}

#[cfg(test)]
fn root_is_compatible(prev: &Node, next: &Node) -> bool {
    match (prev, next) {
        (Node::Document { .. }, Node::Document { .. }) => true,
        (Node::Element { name: a, .. }, Node::Element { name: b, .. }) => a == b,
        (Node::Text { .. }, Node::Text { .. }) => true,
        (Node::Comment { .. }, Node::Comment { .. }) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DomHandle, DomVersion, HtmlState, MAX_PATCH_BUFFER_RETAIN, MIN_PATCH_BUFFER_RETAIN,
        ParserMode, PatchState, PreviewClock, PreviewPolicy, SystemClock, TreeBuilderConfig,
        diff_dom, emit_create_subtree, estimate_patch_bytes_slice, parse_runtime_parser_mode,
        parser_mode_from_env_with, patch_buffer_retain_target, resolve_parser_mode,
        start_parse_runtime_with_policy_and_clock_and_mode,
    };
    use bus::{CoreCommand, CoreEvent};
    use html::{DomPatch, Node, Tokenizer, TreeBuilder, build_owned_dom, tokenize};
    use std::sync::{Arc, Mutex, mpsc};
    use std::time::{Duration, Instant};

    fn tokenize_bytes_in_chunks(bytes: &[u8], boundaries: &[usize]) -> String {
        let mut tokenizer = Tokenizer::new();
        let mut last = 0usize;
        for &idx in boundaries {
            assert!(idx > last && idx <= bytes.len(), "invalid boundary {idx}");
            tokenizer.feed(&bytes[last..idx]);
            last = idx;
        }
        if last < bytes.len() {
            tokenizer.feed(&bytes[last..]);
        }
        tokenizer.finish();
        let stream = tokenizer.into_stream();
        let text = stream.iter().find_map(|t| stream.text(t)).unwrap_or("");
        text.to_string()
    }

    #[test]
    fn utf8_chunk_assembly_smoke_test() {
        let input = "café 😀";
        let bytes = input.as_bytes();
        let boundaries = vec![1, bytes.len() - 1];
        let rebuilt = tokenize_bytes_in_chunks(bytes, &boundaries);
        assert_eq!(
            rebuilt, input,
            "expected UTF-8 roundtrip for boundaries={boundaries:?}"
        );
    }

    #[test]
    fn preview_policy_flushes_on_thresholds() {
        let policy = PreviewPolicy {
            tick: Duration::from_millis(100),
            token_threshold: Some(10),
            byte_threshold: Some(256),
            patch_threshold: Some(5),
            patch_byte_threshold: Some(64),
        };

        assert!(
            !policy.should_flush(Duration::from_millis(50), 0, 0, 0, 0),
            "should not flush before thresholds"
        );
        assert!(
            policy.should_flush(Duration::from_millis(150), 0, 0, 1, 0),
            "should flush on time"
        );
        assert!(
            policy.should_flush(Duration::from_millis(10), 10, 0, 1, 0),
            "should flush on token threshold"
        );
        assert!(
            policy.should_flush(Duration::from_millis(10), 0, 256, 1, 0),
            "should flush on byte threshold"
        );
        assert!(
            policy.should_flush(Duration::from_millis(10), 0, 0, 5, 0),
            "should flush on patch threshold"
        );
        assert!(
            policy.should_flush(Duration::from_millis(10), 0, 0, 1, 64),
            "should flush on patch byte threshold"
        );
        assert!(
            !policy.should_flush(Duration::from_millis(150), 10, 256, 0, 64),
            "should not flush without pending patches"
        );
    }

    #[test]
    fn preview_policy_unbounded_is_clamped() {
        let policy = PreviewPolicy {
            tick: Duration::ZERO,
            token_threshold: None,
            byte_threshold: None,
            patch_threshold: None,
            patch_byte_threshold: None,
        };
        let bounded = policy.ensure_bounded();
        assert!(
            bounded.is_bounded(),
            "expected unbounded policy to be clamped"
        );
        assert!(
            bounded.tick != Duration::ZERO,
            "expected clamped policy to restore a tick"
        );
    }

    #[test]
    fn parser_mode_defaults_to_legacy() {
        let mode = parser_mode_from_env_with(|_| None);
        assert_eq!(mode, ParserMode::Legacy);
    }

    #[test]
    fn parser_mode_parses_known_values() {
        assert_eq!(
            parse_runtime_parser_mode(Some("legacy")),
            Some(ParserMode::Legacy)
        );
        assert_eq!(
            parse_runtime_parser_mode(Some("html5")),
            Some(ParserMode::Html5)
        );
        assert_eq!(
            parse_runtime_parser_mode(Some("LeGaCy")),
            Some(ParserMode::Legacy)
        );
    }

    #[test]
    fn parser_mode_from_env_handles_invalid_value() {
        let mode = parser_mode_from_env_with(|_| Some("unknown".to_string()));
        assert_eq!(mode, ParserMode::Legacy);
    }

    #[cfg(not(feature = "html5"))]
    #[test]
    fn resolve_parser_mode_falls_back_without_feature() {
        assert_eq!(resolve_parser_mode(ParserMode::Html5), ParserMode::Legacy);
    }

    #[cfg(feature = "html5")]
    #[test]
    fn resolve_parser_mode_allows_html5_with_feature() {
        assert_eq!(resolve_parser_mode(ParserMode::Html5), ParserMode::Html5);
    }

    #[cfg(feature = "html5")]
    #[test]
    fn runtime_html5_mode_updates_are_well_formed_if_any() {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (evt_tx, evt_rx) = mpsc::channel();
        let policy = PreviewPolicy::default();

        start_parse_runtime_with_policy_and_clock_and_mode(
            cmd_rx,
            evt_tx,
            policy,
            SystemClock,
            ParserMode::Html5,
        );

        let tab_id = 7;
        let request_id = 99;
        cmd_tx
            .send(CoreCommand::ParseHtmlStart { tab_id, request_id })
            .unwrap();
        cmd_tx
            .send(CoreCommand::ParseHtmlChunk {
                tab_id,
                request_id,
                bytes: b"<div>ok</div>".to_vec(),
            })
            .unwrap();
        cmd_tx
            .send(CoreCommand::ParseHtmlDone { tab_id, request_id })
            .unwrap();

        for _ in 0..5 {
            match evt_rx.recv_timeout(Duration::from_millis(20)) {
                Ok(CoreEvent::DomPatchUpdate {
                    from, to, patches, ..
                }) => {
                    assert_ne!(from, to, "expected version bump on patch update");
                    assert!(!patches.is_empty(), "expected non-empty patch updates");
                }
                Ok(_) => {}
                Err(_) => {}
            }
        }
    }

    #[test]
    fn runtime_flushes_on_tick_without_sleeping() {
        #[derive(Clone)]
        struct ManualClock {
            now: Arc<Mutex<Instant>>,
        }

        impl PreviewClock for ManualClock {
            fn now(&self) -> Instant {
                *self.now.lock().expect("manual clock lock poisoned")
            }
        }

        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (evt_tx, evt_rx) = mpsc::channel();
        let clock = ManualClock {
            now: Arc::new(Mutex::new(Instant::now())),
        };
        let clock_handle = Arc::clone(&clock.now);

        let policy = PreviewPolicy {
            tick: Duration::from_millis(50),
            token_threshold: None,
            byte_threshold: None,
            patch_threshold: None,
            patch_byte_threshold: None,
        };

        start_parse_runtime_with_policy_and_clock_and_mode(
            cmd_rx,
            evt_tx,
            policy,
            clock,
            ParserMode::Legacy,
        );

        let tab_id = 1;
        let request_id = 1;
        cmd_tx
            .send(CoreCommand::ParseHtmlStart { tab_id, request_id })
            .unwrap();
        cmd_tx
            .send(CoreCommand::ParseHtmlChunk {
                tab_id,
                request_id,
                bytes: b"<div>".to_vec(),
            })
            .unwrap();

        assert!(
            evt_rx.recv_timeout(Duration::from_millis(10)).is_err(),
            "should not flush before tick"
        );

        {
            let mut now = clock_handle.lock().expect("manual clock lock poisoned");
            *now += Duration::from_millis(100);
        }

        cmd_tx
            .send(CoreCommand::ParseHtmlChunk {
                tab_id,
                request_id,
                bytes: b" ".to_vec(),
            })
            .unwrap();

        let evt = evt_rx
            .recv_timeout(Duration::from_millis(50))
            .expect("expected DomPatchUpdate after tick");
        match evt {
            CoreEvent::DomPatchUpdate { .. } => {}
            other => panic!("unexpected event: {other:?}"),
        }

        let _ = cmd_tx.send(CoreCommand::ParseHtmlDone { tab_id, request_id });
    }

    #[test]
    fn patch_buffer_does_not_grow_unbounded_in_streaming() {
        let policy = PreviewPolicy {
            tick: Duration::ZERO,
            token_threshold: None,
            byte_threshold: None,
            patch_threshold: Some(256),
            patch_byte_threshold: Some(64 * 1024),
        };
        let patch_threshold = policy.patch_threshold.expect("patch threshold missing");
        let patch_byte_threshold = policy
            .patch_byte_threshold
            .expect("patch byte threshold missing");
        // Slack allows for bursts between threshold checks (builder emits in batches).
        let slack_patches = 64usize;
        // Byte slack covers worst-case single-burst patch payloads.
        let slack_bytes = 32 * 1024usize;

        let now = Instant::now();
        let mut st = HtmlState {
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
            patch_buffer_retain: patch_buffer_retain_target(
                policy.patch_threshold,
                policy.patch_byte_threshold,
            ),
            max_patch_buffer_len: 0,
            max_patch_buffer_bytes: 0,
            dom_handle: DomHandle(1),
            version: DomVersion::INITIAL,
        };

        let (evt_tx, _evt_rx) = mpsc::channel();
        let tab_id = 1;
        let request_id = 1;
        let input = "<div><span>hi</span></div>".repeat(20_000);

        for chunk in input.as_bytes().chunks(1) {
            st.total_bytes = st.total_bytes.saturating_add(chunk.len());
            st.pending_bytes = st.pending_bytes.saturating_add(chunk.len());
            st.pending_tokens = st.pending_tokens.saturating_add(st.tokenizer.feed(chunk));
            st.tokenizer.drain_into(&mut st.token_buffer);
            st.drain_tokens_into_builder();

            if policy.should_flush(
                Duration::ZERO,
                st.pending_tokens,
                st.pending_bytes,
                st.patch_buffer.len(),
                st.pending_patch_bytes,
            ) {
                st.last_emit = now;
                st.flush_patch_buffer(&evt_tx, tab_id, request_id);
            }
        }

        st.pending_tokens = st.pending_tokens.saturating_add(st.tokenizer.finish());
        st.tokenizer.drain_into(&mut st.token_buffer);
        st.drain_tokens_into_builder();
        let _ = st.builder.finish();
        let final_patches = st.builder.take_patches();
        if !final_patches.is_empty() {
            st.pending_patch_bytes = st
                .pending_patch_bytes
                .saturating_add(estimate_patch_bytes_slice(&final_patches));
            st.patch_buffer.extend(final_patches);
            st.update_patch_buffer_max();
        }
        st.flush_patch_buffer(&evt_tx, tab_id, request_id);

        assert!(
            st.max_patch_buffer_len <= patch_threshold + slack_patches,
            "patch buffer grew beyond bound: max_len={} threshold={} slack={}",
            st.max_patch_buffer_len,
            patch_threshold,
            slack_patches
        );
        assert!(
            st.max_patch_buffer_bytes <= patch_byte_threshold + slack_bytes,
            "patch buffer bytes grew beyond bound: max_bytes={} threshold={} slack={}",
            st.max_patch_buffer_bytes,
            patch_byte_threshold,
            slack_bytes
        );
    }

    #[test]
    fn patch_updates_are_bounded_under_streaming_policy() {
        let policy = PreviewPolicy {
            tick: Duration::ZERO,
            token_threshold: None,
            byte_threshold: None,
            patch_threshold: Some(200),
            patch_byte_threshold: Some(64 * 1024),
        };

        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (evt_tx, evt_rx) = mpsc::channel();
        start_parse_runtime_with_policy_and_clock_and_mode(
            cmd_rx,
            evt_tx,
            policy,
            SystemClock,
            ParserMode::Legacy,
        );

        let tab_id = 1;
        let request_id = 42;
        cmd_tx
            .send(CoreCommand::ParseHtmlStart { tab_id, request_id })
            .unwrap();

        let input = "<div><span>hi</span></div>".repeat(20_000);
        for chunk in input.as_bytes().chunks(1) {
            cmd_tx
                .send(CoreCommand::ParseHtmlChunk {
                    tab_id,
                    request_id,
                    bytes: chunk.to_vec(),
                })
                .unwrap();
        }
        cmd_tx
            .send(CoreCommand::ParseHtmlDone { tab_id, request_id })
            .unwrap();

        let mut max_patches = 0usize;
        let mut max_bytes = 0usize;
        let slack_patches = 64usize;
        // Slack allows for bursts between threshold checks (builder emits in batches).
        let slack_bytes = 16 * 1024usize;

        let mut saw_update = false;
        let mut idle_ticks = 0usize;
        while idle_ticks < 10 {
            match evt_rx.recv_timeout(Duration::from_millis(100)) {
                Ok(CoreEvent::DomPatchUpdate { patches, .. }) => {
                    saw_update = true;
                    idle_ticks = 0;
                    let count = patches.len();
                    let bytes = estimate_patch_bytes_slice_test(&patches);
                    if count > max_patches {
                        max_patches = count;
                    }
                    if bytes > max_bytes {
                        max_bytes = bytes;
                    }
                    assert!(
                        count <= 200 + slack_patches,
                        "patch update exceeded bound: count={count}"
                    );
                    assert!(
                        bytes <= 64 * 1024 + slack_bytes,
                        "patch update exceeded byte bound: bytes={bytes}"
                    );
                }
                Ok(_) => {}
                Err(_) => {
                    idle_ticks += 1;
                }
            }
        }

        assert!(saw_update, "expected at least one patch update");
        assert!(max_patches > 0, "expected patch count to be non-zero");
        assert!(max_bytes > 0, "expected patch payload to be non-zero");
    }

    #[test]
    fn patch_buffer_retain_capacity_is_bounded_on_flush() {
        let now = Instant::now();
        let mut st = HtmlState {
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
            patch_buffer: Vec::with_capacity(100_000),
            patch_buffer_retain: patch_buffer_retain_target(Some(128), None),
            max_patch_buffer_len: 0,
            max_patch_buffer_bytes: 0,
            dom_handle: DomHandle(1),
            version: DomVersion::INITIAL,
        };
        st.patch_buffer.push(DomPatch::Clear);
        let (evt_tx, _evt_rx) = mpsc::channel();
        st.flush_patch_buffer(&evt_tx, 1, 1);
        let cap = st.patch_buffer.capacity();
        assert!(
            cap <= MAX_PATCH_BUFFER_RETAIN,
            "expected capped retain capacity, got {cap}"
        );
        assert!(
            cap >= MIN_PATCH_BUFFER_RETAIN,
            "expected retain capacity to be at least the floor, got {cap}"
        );
    }

    #[test]
    fn patch_buffer_retain_target_clamps_to_max() {
        let huge = MAX_PATCH_BUFFER_RETAIN.saturating_mul(10);
        let retain = patch_buffer_retain_target(Some(huge), None);
        assert_eq!(
            retain, MAX_PATCH_BUFFER_RETAIN,
            "expected retain target to clamp to max"
        );
    }

    fn estimate_patch_bytes_slice_test(patches: &[html::DomPatch]) -> usize {
        super::estimate_patch_bytes_slice(patches)
    }

    fn full_create_patches(dom: &Node) -> Vec<html::DomPatch> {
        let mut patch_state = PatchState::new();
        let mut patches = Vec::new();
        let mut need_reset = false;
        emit_create_subtree(dom, None, &mut patch_state, &mut patches, &mut need_reset);
        assert!(!need_reset, "full create stream failed");
        patches
    }

    #[test]
    fn patch_updates_do_not_resend_full_tree_each_tick() {
        let inputs = [
            "<div>",
            "<div><span>",
            "<div><span>hi</span>",
            "<div><span>hi</span><em>ok</em>",
        ];
        let mut patch_state = PatchState::new();
        let mut prev_dom: Option<Box<Node>> = None;

        for (tick, input) in inputs.iter().enumerate() {
            let stream = tokenize(input);
            let dom = build_owned_dom(&stream);
            let full_patches = full_create_patches(&dom);
            let full_bytes = estimate_patch_bytes_slice_test(&full_patches);
            let patches = match prev_dom.as_deref() {
                Some(prev) => diff_dom(prev, &dom, &mut patch_state).expect("diff failed"),
                None => {
                    let mut patches = Vec::new();
                    let mut need_reset = false;
                    emit_create_subtree(
                        &dom,
                        None,
                        &mut patch_state,
                        &mut patches,
                        &mut need_reset,
                    );
                    assert!(!need_reset, "initial create stream failed");
                    patches
                }
            };

            if tick == 0 {
                assert!(
                    !patches.is_empty(),
                    "expected initial create stream on first tick"
                );
                assert_eq!(
                    patches.len(),
                    full_patches.len(),
                    "first tick should be a full create stream"
                );
            } else {
                assert!(
                    !matches!(patches.first(), Some(html::DomPatch::Clear)),
                    "unexpected reset on append-only tick {tick}"
                );
                let created = patches
                    .iter()
                    .filter(|p| {
                        matches!(
                            p,
                            html::DomPatch::CreateDocument { .. }
                                | html::DomPatch::CreateElement { .. }
                                | html::DomPatch::CreateText { .. }
                                | html::DomPatch::CreateComment { .. }
                        )
                    })
                    .count();
                let full_created = full_patches
                    .iter()
                    .filter(|p| {
                        matches!(
                            p,
                            html::DomPatch::CreateDocument { .. }
                                | html::DomPatch::CreateElement { .. }
                                | html::DomPatch::CreateText { .. }
                                | html::DomPatch::CreateComment { .. }
                        )
                    })
                    .count();
                let removed = patches
                    .iter()
                    .filter(|p| matches!(p, html::DomPatch::RemoveNode { .. }))
                    .count();
                assert_eq!(removed, 0, "unexpected removals on append-only tick {tick}");
                let bytes = estimate_patch_bytes_slice_test(&patches);
                assert!(
                    bytes <= full_bytes,
                    "patch payload exceeded full create stream: tick={tick} bytes={bytes} full_bytes={full_bytes}"
                );
                assert!(
                    patches.len() <= full_patches.len(),
                    "patch count exceeded full create stream: tick={tick} patches={} full_patches={}",
                    patches.len(),
                    full_patches.len()
                );
                if full_created > 20 {
                    assert!(
                        created < full_created,
                        "patch created too many nodes: tick={tick} created={created} full_created={full_created}"
                    );
                }
                assert!(
                    bytes < full_bytes,
                    "patch payload regressed: tick={tick} bytes={bytes} full_bytes={full_bytes}"
                );
            }

            prev_dom = Some(Box::new(dom));
        }
    }

    #[test]
    fn patch_updates_do_not_rebuild_medium_tree_each_tick() {
        let mut inputs = Vec::new();
        let mut buf = String::from("<div>");
        inputs.push(buf.clone());
        for i in 0..200 {
            buf.push_str("<span>item</span>");
            if i == 49 || i == 119 || i == 199 {
                inputs.push(buf.clone());
            }
        }

        let mut patch_state = PatchState::new();
        let mut prev_dom: Option<Box<Node>> = None;

        for (tick, input) in inputs.iter().enumerate() {
            let stream = tokenize(input);
            let dom = build_owned_dom(&stream);
            let full_patches = full_create_patches(&dom);
            let full_bytes = estimate_patch_bytes_slice_test(&full_patches);
            let patches = match prev_dom.as_deref() {
                Some(prev) => diff_dom(prev, &dom, &mut patch_state).expect("diff failed"),
                None => {
                    let mut patches = Vec::new();
                    let mut need_reset = false;
                    emit_create_subtree(
                        &dom,
                        None,
                        &mut patch_state,
                        &mut patches,
                        &mut need_reset,
                    );
                    assert!(!need_reset, "initial create stream failed");
                    patches
                }
            };

            if tick == 0 {
                assert!(
                    !patches.is_empty(),
                    "expected initial create stream on first tick"
                );
            } else {
                assert!(
                    !matches!(patches.first(), Some(html::DomPatch::Clear)),
                    "unexpected reset on append-only tick {tick}"
                );
                let created = patches
                    .iter()
                    .filter(|p| {
                        matches!(
                            p,
                            html::DomPatch::CreateDocument { .. }
                                | html::DomPatch::CreateElement { .. }
                                | html::DomPatch::CreateText { .. }
                                | html::DomPatch::CreateComment { .. }
                        )
                    })
                    .count();
                assert!(
                    created > 0,
                    "expected growth to create nodes on tick {tick}"
                );
                let removed = patches
                    .iter()
                    .filter(|p| matches!(p, html::DomPatch::RemoveNode { .. }))
                    .count();
                assert_eq!(removed, 0, "unexpected removals on append-only tick {tick}");
                let bytes = estimate_patch_bytes_slice_test(&patches);
                assert!(
                    bytes < full_bytes,
                    "patch payload regressed: tick={tick} bytes={bytes} full_bytes={full_bytes}"
                );
                assert!(
                    patches.len() <= full_patches.len(),
                    "patch count exceeded full create stream: tick={tick} patches={} full_patches={}",
                    patches.len(),
                    full_patches.len()
                );
            }

            prev_dom = Some(Box::new(dom));
        }
    }
}
