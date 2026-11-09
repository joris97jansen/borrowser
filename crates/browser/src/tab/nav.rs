use super::Tab;
use super::state::{DocumentState, HistoryEntry};
use addressing::{
    BrowserUrl, Origin, ResourceAddress, ResourceKind as Purpose, UrlFailureKind, UrlParseError,
};
use bus::CoreCommand;
use core_types::ResourceKind;
use net::{FetchRequest, HttpHeaders, RequestAdmissionError, RequestMethod};

// The two existing history actions; preparation never mutates either one.
enum HistoryTarget {
    Append,
    Existing(usize),
}

#[derive(Debug)]
enum NavigationError {
    EmptyInput,
    Url(UrlFailureKind),
    Request(RequestAdmissionError),
    DeliveryUnavailable,
}

impl NavigationError {
    fn status(&self) -> String {
        match self {
            Self::EmptyInput => "Navigation rejected: empty input".into(),
            Self::Url(kind) => format!("Navigation URL rejected: {kind:?}"),
            Self::Request(kind) => kind.to_string(),
            Self::DeliveryUnavailable => "Navigation request delivery failed".into(),
        }
    }
}

impl Tab {
    pub fn navigate_to_new(&mut self, input: String) {
        let result = normalize_address_bar(&input).and_then(|url| {
            self.navigate_url(ResourceAddress::new(url, Purpose::Document), None, None)
        });
        self.report_navigation_result(result);
    }

    pub(crate) fn activate_link(&mut self, href: &str) {
        let Some(base) = self.page.active_base_url() else {
            self.last_status = Some("Cannot follow a link without document address state".into());
            return;
        };
        let target = match base.resolve(href) {
            Ok(parsed) => parsed.url,
            Err(error) => {
                self.report_navigation_result(Err(NavigationError::Url(error.kind)));
                return;
            }
        };
        let initiator = self.page.document_url().cloned();
        let origin = self
            .page
            .addresses
            .as_ref()
            .map(|a| a.document_origin().clone());
        let result = self.navigate_url(
            ResourceAddress::new(target, Purpose::Hyperlink),
            initiator,
            origin,
        );
        self.report_navigation_result(result);
    }

    fn navigate_url(
        &mut self,
        address: ResourceAddress,
        initiator: Option<BrowserUrl>,
        origin: Option<Origin>,
    ) -> Result<(), NavigationError> {
        let url = address.url().as_str().to_owned();
        if let Some((destination, state)) = self
            .same_document_destination(address.url())
            .filter(|target| target.fragment().is_some())
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
                document_state: Some(state),
            });
            self.history_index = self.history.len() - 1;
            self.url = url;
            self.poke_redraw();
            return Ok(());
        }
        let request = self.prepare_document_request(address, initiator, origin)?;
        self.start_fetch(request, HistoryTarget::Append)
    }

    pub fn go_back(&mut self) {
        if self.history_index > 0 {
            let result = self.load_current(self.history_index - 1, self.active_document_state());
            self.report_navigation_result(result);
        }
    }

    pub fn go_forward(&mut self) {
        if self.history_index + 1 < self.history.len() {
            let result = self.load_current(self.history_index + 1, self.active_document_state());
            self.report_navigation_result(result);
        }
    }

    pub fn refresh(&mut self) {
        if let Some(entry) = self.history.get(self.history_index) {
            let result = BrowserUrl::parse(&entry.url)
                .map_err(|e| NavigationError::Url(e.kind))
                .and_then(|parsed| {
                    self.prepare_document_request(
                        ResourceAddress::new(parsed.url, Purpose::Document),
                        None,
                        None,
                    )
                })
                .and_then(|request| {
                    self.start_fetch(request, HistoryTarget::Existing(self.history_index))
                });
            self.report_navigation_result(result);
        }
    }

    fn prepare_document_request(
        &self,
        address: ResourceAddress,
        initiator: Option<BrowserUrl>,
        origin: Option<Origin>,
    ) -> Result<FetchRequest, NavigationError> {
        FetchRequest::try_new(
            self.nav_gen.wrapping_add(1),
            address,
            RequestMethod::Get,
            HttpHeaders::new(),
            initiator,
            origin,
        )
        .map_err(NavigationError::Request)
    }

    fn start_fetch(
        &mut self,
        request: FetchRequest,
        history: HistoryTarget,
    ) -> Result<(), NavigationError> {
        let url = request.original_url().as_str().to_owned();
        let generation = request.request_id();
        let command = CoreCommand::try_fetch_stream(self.tab_id, None, ResourceKind::Html, request)
            .map_err(NavigationError::Request)?;
        // Queue acceptance precedes all navigation mutation. Exclusive &mut Tab
        // prevents processing any fast response until commit completes. A failed
        // send returns the command; never format that payload as an error.
        self.cmd_tx
            .as_ref()
            .ok_or(NavigationError::DeliveryUnavailable)?
            .send(command)
            .map_err(|_| NavigationError::DeliveryUnavailable)?;
        self.retire_document_stylesheets();
        self.resources.retire_pending_images(self.nav_gen);
        if self.nav_gen > 0 {
            self.send_cmd(CoreCommand::CancelRequest {
                tab_id: self.tab_id,
                request_id: self.nav_gen,
            });
        }
        match history {
            HistoryTarget::Append => {
                self.history.truncate(self.history_index + 1);
                self.history.push(HistoryEntry {
                    url: url.clone(),
                    document_state: None,
                });
                self.history_index = self.history.len() - 1;
            }
            HistoryTarget::Existing(index) => {
                // Retain the old association for response-time group rebinding.
                self.history[index].url = url.clone();
                self.history_index = index;
            }
        }
        self.url = url;
        self.document_input.clear_for_navigation();
        self.nav_gen = generation;
        self.loading = true;
        self.document_load = Default::default();
        self.stylesheet_loads.clear();
        self.last_status = Some("Fetching document".into());
        self.page.reset_for_navigation();
        self.clear_render_orchestration_state();
        self.poke_redraw();
        Ok(())
    }

    fn load_current(
        &mut self,
        target_index: usize,
        previous: Option<DocumentState>,
    ) -> Result<(), NavigationError> {
        let entry = &self.history[target_index];
        let destination = BrowserUrl::parse(&entry.url)
            .map_err(|e| NavigationError::Url(e.kind))?
            .url;
        let same_document = previous.as_ref().is_some_and(|active| {
            entry
                .document_state
                .as_ref()
                .is_some_and(|target| active.is_same(target))
        });
        if same_document {
            let url = destination.as_str().to_owned();
            self.page
                .addresses
                .as_mut()
                .expect("same-document address exists")
                .set_document_url(destination);
            self.history_index = target_index;
            self.history[target_index].url = url.clone();
            self.url = url;
            self.poke_redraw();
            return Ok(());
        }
        let request = self.prepare_document_request(
            ResourceAddress::new(destination, Purpose::Document),
            None,
            None,
        )?;
        self.start_fetch(request, HistoryTarget::Existing(target_index))
    }

    fn report_navigation_result(&mut self, result: Result<(), NavigationError>) {
        if let Err(error) = result {
            self.last_status = Some(error.status());
            self.poke_redraw();
        }
    }

    fn same_document_destination(&self, next: &BrowserUrl) -> Option<BrowserUrl> {
        let current = self.page.document_url()?;
        urls_equal_excluding_fragment(current, next).then(|| next.clone())
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

// Address-bar intent only. An authority with a decimal port takes precedence
// over scheme parsing, including ambiguous `custom:3000`. AH still owns parsing
// and validation of the HTTPS candidate; links never enter this input policy.
fn normalize_address_bar(input: &str) -> Result<BrowserUrl, NavigationError> {
    let input = input.trim();
    if input.is_empty() {
        return Err(NavigationError::EmptyInput);
    }
    let authority = input.split(['/', '?', '#']).next().unwrap_or("");
    let host_port = !authority.contains('@')
        && authority.rsplit_once(':').is_some_and(|(host, port)| {
            !host.is_empty() && !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit())
        });
    if host_port {
        return BrowserUrl::parse(&format!("https://{input}"))
            .map(|p| p.url)
            .map_err(|e| NavigationError::Url(e.kind));
    }
    match BrowserUrl::parse(input) {
        Ok(parsed) => Ok(parsed.url),
        Err(error)
            if error.kind == UrlFailureKind::InvalidInput(UrlParseError::RelativeWithoutBase) =>
        {
            BrowserUrl::parse(&format!("https://{input}"))
                .map(|p| p.url)
                .map_err(|e| NavigationError::Url(e.kind))
        }
        Err(error) => Err(NavigationError::Url(error.kind)),
    }
}
