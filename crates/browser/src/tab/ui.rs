use super::Tab;
use crate::view::content;
use egui::Context;

impl Tab {
    pub fn ui_content(&mut self, ctx: &Context) {
        // Drain completed decode jobs and upload textures before painting.
        if self.resources.pump(ctx) {
            self.poke_redraw();
        }

        if let Some(action) = content(
            ctx,
            &mut self.page,
            &mut self.document_input,
            &self.resources,
            self.last_status.as_ref(),
            self.loading,
        ) {
            match action {
                crate::view::PageAction::Navigate(url) => self.navigate_to_new(url),
            }
        }
    }

    /// Derive a human-friendly label from the URL:
    /// - for http/https: "host — last/path/segment"
    /// - for file://: just the file name
    /// - otherwise: empty string if parse fails
    fn url_label(&self) -> String {
        if self.url.is_empty() {
            return String::new();
        }

        if let Ok(url) = url::Url::parse(&self.url) {
            // file:// → show file name only
            let file_name = if url.scheme() == "file" {
                url.to_file_path().ok().and_then(|path| {
                    path.file_name()
                        .and_then(|s| s.to_str())
                        .map(|s| s.to_string())
                })
            } else {
                None
            };

            if let Some(name) = file_name {
                return name;
            }

            // http/https → "host — last-segment"
            let mut label = String::new();

            if let Some(host) = url.host_str() {
                label.push_str(host);
            }

            if let Some(last_seg) = url
                .path_segments()
                .and_then(|mut segs| segs.rfind(|s| !s.is_empty()))
            {
                if !label.is_empty() {
                    label.push_str(" — ");
                }
                label.push_str(last_seg);
            }

            return label;
        }

        // Fallback: nothing nice we can format
        String::new()
    }

    pub fn display_title(&self) -> String {
        // 1) Prefer <title> from head
        if let Some(title) = self.page.head.title.as_ref() {
            let trimmed = title.trim();
            if !trimmed.is_empty() {
                return elide_end(trimmed, 30);
            }
        }

        // 2) If still loading and we have a URL, show a loading label
        if self.loading && !self.url.is_empty() {
            let core = self.url_label();
            if core.is_empty() {
                return "Loading…".to_string();
            } else {
                // keep total length modest
                return format!("Loading… — {}", elide_end(&core, 24));
            }
        }

        // 3) No title, not loading → fall back to URL-based label
        let url_label = self.url_label();
        if !url_label.is_empty() {
            return elide_end(&url_label, 30);
        }

        // 4) Absolute last fallback:
        if !self.url.is_empty() {
            return elide_end(&self.url, 30);
        }

        "New Tab".to_string()
    }
}

fn elide_end(text: &str, max_chars: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max_chars {
        return text.to_owned();
    }

    let keep = max_chars.saturating_sub(1);
    let mut s: String = chars[..keep].iter().collect();
    s.push('…');
    s
}
