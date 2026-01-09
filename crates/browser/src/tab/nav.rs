use super::Tab;
use bus::CoreCommand;
use core_types::ResourceKind;
use url::Url;

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
        let current_url = self.url.clone();
        self.url = url.clone();

        if self.is_same_document_navigation_with(&current_url, &url) {
            self.history.truncate(self.history_index + 1);
            self.history.push(url.clone());
            self.history_index = self.history.len() - 1;
            self.poke_redraw();
            return;
        }

        // record to history (truncate forward branch)
        self.history.truncate(self.history_index + 1);
        self.history.push(url.clone());
        self.history_index = self.history.len() - 1;

        self.start_fetch(url);
    }

    pub fn go_back(&mut self) {
        if self.history_index > 0 {
            self.history_index -= 1;
            let url = self.history[self.history_index].clone();
            self.load_current(url);
        }
    }

    pub fn go_forward(&mut self) {
        if self.history_index + 1 < self.history.len() {
            self.history_index += 1;
            let url = self.history[self.history_index].clone();
            self.load_current(url);
        }
    }

    pub fn refresh(&mut self) {
        if let Some(url) = self.history.get(self.history_index).cloned() {
            self.url = url.clone();
            self.start_fetch(url);
        }
    }

    // -- Internal Helpers ---
    fn start_fetch(&mut self, url: String) {
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
        self.last_status = Some(format!("Fetching {url} …"));
        self.page.start_nav(&url);

        self.send_cmd(CoreCommand::FetchStream {
            tab_id: self.tab_id,
            request_id,
            url,
            kind: ResourceKind::Html,
        });
        self.poke_redraw();
    }

    fn load_current(&mut self, url: String) {
        // do NOT touch history; just fetch the given URL
        if self.is_same_document_navigation(&url) {
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

    fn is_same_document_navigation(&self, next_url: &str) -> bool {
        self.is_same_document_navigation_with(&self.url, next_url)
    }

    fn is_same_document_navigation_with(&self, current_url: &str, next_url: &str) -> bool {
        if current_url.is_empty() {
            return false;
        }

        let Ok(current) = Url::parse(current_url) else {
            return false;
        };
        let Ok(next) = Url::parse(next_url) else {
            return false;
        };

        current.scheme() == next.scheme()
            && current.host_str() == next.host_str()
            && current.port_or_known_default() == next.port_or_known_default()
            && current.path() == next.path()
            && current.query() == next.query()
    }
}
