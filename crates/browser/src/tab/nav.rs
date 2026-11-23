use super::Tab;
use super::state::{DocumentState, HistoryEntry, HistoryTarget, NavigationAttempt};
use addressing::{
    BrowserUrl, Origin, ResourceAddress, ResourceKind as Purpose, UrlFailureKind, UrlParseError,
};
use bus::CoreCommand;
use core_types::ResourceKind;
use net::{FetchRequest, HttpHeaders, RequestAdmissionError, RequestMethod};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavigationError {
    EmptyInput,
    Url(UrlFailureKind),
    Request(RequestAdmissionError),
    DeliveryUnavailable,
    RequestIdExhausted,
    DocumentAddressUnavailable,
}

impl NavigationError {
    fn status(&self) -> String {
        match self {
            Self::RequestIdExhausted => "Navigation request identities exhausted".into(),
            Self::DocumentAddressUnavailable => {
                "Cannot follow a link without document address state".into()
            }
            Self::EmptyInput => "Navigation rejected: empty input".into(),
            Self::Url(kind) => format!("Navigation URL rejected: {kind:?}"),
            Self::Request(kind) => kind.to_string(),
            Self::DeliveryUnavailable => "Navigation request delivery failed".into(),
        }
    }
}

impl Tab {
    /// Submit navigation; success is queue acceptance or a same-document action.
    pub fn navigate_to_new(&mut self, input: String) -> Result<(), NavigationError> {
        let result = normalize_address_bar(&input).and_then(|url| {
            self.navigate_url(ResourceAddress::new(url, Purpose::Document), None, None)
        });
        self.report_navigation_result(&result);
        result
    }

    pub(crate) fn activate_link(&mut self, href: &str) -> Result<(), NavigationError> {
        let Some(base) = self.page.active_base_url() else {
            let result = Err(NavigationError::DocumentAddressUnavailable);
            self.report_navigation_result(&result);
            return result;
        };
        let target = match base.resolve(href) {
            Ok(parsed) => parsed.url,
            Err(error) => {
                let result = Err(NavigationError::Url(error.kind));
                self.report_navigation_result(&result);
                return result;
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
        self.report_navigation_result(&result);
        result
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
            self.cancel_pending_navigation();
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

    pub fn go_back(&mut self) -> Result<(), NavigationError> {
        let result = if self.history_index > 0 {
            self.load_current(self.history_index - 1, self.active_document_state())
        } else {
            Ok(())
        };
        self.report_navigation_result(&result);
        result
    }

    pub fn go_forward(&mut self) -> Result<(), NavigationError> {
        let result = if self.history_index + 1 < self.history.len() {
            self.load_current(self.history_index + 1, self.active_document_state())
        } else {
            Ok(())
        };
        self.report_navigation_result(&result);
        result
    }

    pub fn refresh(&mut self) -> Result<(), NavigationError> {
        let result = if let Some(entry) = self.history.get(self.history_index) {
            BrowserUrl::parse(&entry.url)
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
                })
        } else {
            Ok(())
        };
        self.report_navigation_result(&result);
        result
    }

    fn prepare_document_request(
        &mut self,
        address: ResourceAddress,
        initiator: Option<BrowserUrl>,
        origin: Option<Origin>,
    ) -> Result<FetchRequest, NavigationError> {
        FetchRequest::try_new(
            self.reserve_navigation_request_id()?,
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
        let command =
            CoreCommand::try_fetch_stream(self.tab_id, None, ResourceKind::Html, request.clone())
                .map_err(NavigationError::Request)?;
        // Enqueue before superseding a pending request. The actual active page
        // and its runtime work remain untouched until response activation.
        self.cmd_tx
            .as_ref()
            .ok_or(NavigationError::DeliveryUnavailable)?
            .send(command)
            .map_err(|_| NavigationError::DeliveryUnavailable)?;
        self.cancel_pending_navigation();
        self.navigation_attempt = Some(NavigationAttempt {
            request,
            history_target: history,
            non_replacing_response: None,
            terminal: None,
        });
        self.url = url;
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
            self.cancel_pending_navigation();
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

    fn report_navigation_result(&mut self, result: &Result<(), NavigationError>) {
        // Presentation only: preserve the typed Result and actual A/B lifecycle.
        // A successful submission/action dismisses the previous rejection;
        // older network/parser events cannot erase a newer submission notice.
        let notice = result.as_ref().err().map(NavigationError::status);
        if self.navigation_notice != notice {
            self.navigation_notice = notice;
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
