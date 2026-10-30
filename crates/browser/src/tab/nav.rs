use super::Tab;
use super::state::{DocumentState, HistoryEntry};
use addressing::BrowserUrl;
use bus::CoreCommand;
use core_types::ResourceKind;

impl Tab {
    // -- Navigation Methods ---
    pub fn navigate_to_new(&mut self, url: String) {
        let input_url = url;
        let url = match self.normalize_url(&input_url) {
            Ok(url) => url,
            Err(err) => {
                self.loading = false;
                self.last_status = Some(err.to_string());
                self.url = input_url;
                self.poke_redraw();
                return;
            }
        };
        self.navigate_url(url);
    }

    // HTML hyperlinks already have URL semantics. They never enter address-bar
    // guessing, including syntactically valid but unsupported absolute schemes.
    pub(crate) fn activate_link(&mut self, href: &str) {
        let Some(base) = self.page.active_base_url() else {
            self.last_status = Some("Cannot follow a link without document address state".into());
            return;
        };
        let destination = match base.resolve(href) {
            Ok(parsed) => parsed.url,
            Err(error) => {
                self.last_status = Some(format!("Invalid hyperlink URL: {error}"));
                return;
            }
        };
        if !matches!(destination.scheme(), "http" | "https" | "file") {
            self.last_status = Some("Unsupported hyperlink navigation scheme".into());
            return;
        }
        self.navigate_url(destination.as_str().to_owned());
    }

    fn navigate_url(&mut self, url: String) {
        self.url = url.clone();

        // New fragment navigation needs URL equivalence; traversal instead
        // requires the target entry's actual document-state association.
        if let Some((destination, document_state)) = self
            .same_document_destination(&url)
            .filter(|destination| destination.fragment().is_some())
            .zip(self.active_document_state())
        {
            self.page
                .addresses
                .as_mut()
                .expect("same-document address exists")
                .set_document_url(destination);
            self.history.truncate(self.history_index + 1);
            self.history.push(HistoryEntry {
                url: url.clone(),
                document_state: Some(document_state),
            });
            self.history_index = self.history.len() - 1;
            self.poke_redraw();
            return;
        }

        // record to history (truncate forward branch)
        self.history.truncate(self.history_index + 1);
        self.history.push(HistoryEntry {
            url: url.clone(),
            document_state: None,
        });
        self.history_index = self.history.len() - 1;

        self.start_fetch(url);
    }

    pub fn go_back(&mut self) {
        if self.history_index > 0 {
            let previous_document = self.active_document_state();
            self.history_index -= 1;
            self.load_current(previous_document);
        }
    }

    pub fn go_forward(&mut self) {
        if self.history_index + 1 < self.history.len() {
            let previous_document = self.active_document_state();
            self.history_index += 1;
            self.load_current(previous_document);
        }
    }

    pub fn refresh(&mut self) {
        if let Some(url) = self
            .history
            .get(self.history_index)
            .map(|entry| entry.url.clone())
        {
            self.url = url.clone();
            self.start_fetch(url);
        }
    }

    // -- Internal Helpers ---
    fn start_fetch(&mut self, url: String) {
        self.retire_document_stylesheets();
        if self.nav_gen > 0 {
            self.send_cmd(CoreCommand::CancelRequest {
                tab_id: self.tab_id,
                request_id: self.nav_gen,
            });
        }
        self.document_input.clear_for_navigation();
        self.nav_gen = self.nav_gen.wrapping_add(1);
        let request_id = self.nav_gen;

        self.loading = true;
        self.document_load = Default::default();
        self.stylesheet_loads.clear();
        self.last_status = Some(format!("Fetching {url} …"));
        self.page.reset_for_navigation();
        self.clear_render_orchestration_state();

        self.send_cmd(CoreCommand::FetchStream {
            tab_id: self.tab_id,
            request_id,
            stylesheet_slot_id: None,
            url,
            kind: ResourceKind::Html,
        });
        self.poke_redraw();
    }

    fn load_current(&mut self, previous_document: Option<DocumentState>) {
        let entry = &self.history[self.history_index];
        let url = entry.url.clone();
        let same_document = previous_document.as_ref().is_some_and(|previous| {
            entry
                .document_state
                .as_ref()
                .is_some_and(|target| previous.is_same(target))
        });
        if same_document && let Ok(destination) = BrowserUrl::parse(&url) {
            self.page
                .addresses
                .as_mut()
                .expect("same-document address exists")
                .set_document_url(destination.url);
            self.url = url;
            self.poke_redraw();
            return;
        }
        self.url = url.clone();
        self.start_fetch(url);
    }

    fn normalize_url(&mut self, url: &str) -> Result<String, &'static str> {
        let trimmed = url.trim();
        if trimmed.is_empty() {
            return Err("Cannot navigate to an empty URL");
        }

        // Already a full URL with scheme we support
        if trimmed.starts_with("http://")
            || trimmed.starts_with("https://")
            || trimmed.starts_with("file://")
        {
            return Ok(trimmed.into());
        }

        // For everything else, keep your old "guess https" behavior
        Ok(format!("https://{trimmed}"))
    }

    fn same_document_destination(&self, next_url: &str) -> Option<BrowserUrl> {
        let current = self.page.document_url()?;
        let next = BrowserUrl::parse(next_url).ok()?.url;
        urls_equal_excluding_fragment(current, &next).then_some(next)
    }

    fn active_document_state(&self) -> Option<DocumentState> {
        self.page.document_url()?;
        self.page.dom.as_ref()?;
        self.history.get(self.history_index)?.document_state.clone()
    }

    pub(super) fn response_matches_history_request(&self, requested_url: &str) -> bool {
        // An untracked response keeps the existing event behavior but cannot
        // acquire a reusable history association.
        self.history
            .get(self.history_index)
            .is_none_or(|entry| entry.url == requested_url)
    }

    /// Bind an accepted response to the target entry, never to a pending
    /// request. Repopulated entries retain their group only if the accepted
    /// URL still describes the same document apart from its fragment.
    pub(super) fn accept_history_document(&mut self, document_url: &BrowserUrl) {
        let Some(entry) = self.history.get(self.history_index) else {
            return;
        };
        let previous = entry.document_state.clone();
        let requested = BrowserUrl::parse(&entry.url).ok().map(|parsed| parsed.url);
        let same_address = requested
            .as_ref()
            .is_some_and(|url| urls_equal_excluding_fragment(url, document_url));
        // Response metadata can omit a fragment even when the requested
        // history entry has one. Do not erase that entry's destination.
        let keep_requested_fragment = same_address
            && document_url.fragment().is_none()
            && requested
                .as_ref()
                .is_some_and(|url| url.fragment().is_some());
        let next = DocumentState::new();
        if same_address && let Some(previous) = previous {
            for entry in &mut self.history {
                if entry
                    .document_state
                    .as_ref()
                    .is_some_and(|state| state.is_same(&previous))
                {
                    entry.document_state = Some(next.clone());
                }
            }
        } else {
            self.history[self.history_index].document_state = Some(next);
        }
        if !keep_requested_fragment {
            self.history[self.history_index].url = document_url.as_str().to_owned();
        }
    }
}

fn urls_equal_excluding_fragment(a: &BrowserUrl, b: &BrowserUrl) -> bool {
    a.scheme() == b.scheme()
        && a.username() == b.username()
        && a.password() == b.password()
        && a.host() == b.host()
        && a.port() == b.port()
        && a.path() == b.path()
        && a.query() == b.query()
}
