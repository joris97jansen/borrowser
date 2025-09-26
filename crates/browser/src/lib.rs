use egui::{
    Context,
    TopBottomPanel,
    Key,
    CentralPanel,
};
use app_api::{
    UiApp,
    NetCallback,
};
use net::fetch_text;

pub struct BrowserApp {
    url: String,
    loading: bool,
    last_status: Option<String>,
    last_preview: String,
    net_callback: Option<NetCallback>,
}

impl BrowserApp {
    pub fn new() -> Self {
        Self{
            url: "https://example.com".into(),
            loading: false,
            last_status: None,
            last_preview: String::new(),
            net_callback: None,
        }
    }
}

impl UiApp for BrowserApp {
    fn ui(&mut self, context: &Context) {
        TopBottomPanel::top("topbar").show(context, |ui| {
            ui.horizontal(|ui| {
                ui.label("URL:");
                let response = ui.text_edit_singleline(&mut self.url);
                if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) || ui.button("Go").clicked() {
                    eprintln!("Loading URL: {}", self.url);
                    self.loading = true;
                    self.last_status = Some(format!("Fetching {}…", self.url));
                    self.last_preview.clear();

                    if let Some(cb) = &self.net_callback {
                        fetch_text(self.url.clone(), cb.clone());
                    } else {
                        self.loading = false;
                        self.last_status = Some("No network callback set".into());
                    }
                }
            });
        });
        CentralPanel::default().show(context, |ui| {
            if self.loading { ui.label("⏳ Loading…"); }
            if let Some(s) = &self.last_status { ui.label(s); }
            if !self.last_preview.is_empty() {
                ui.separator();
                ui.label("Preview (first 500 chars):");
                ui.code(self.last_preview.clone());
            }
        });
    }

    fn set_net_callback(&mut self, callback: NetCallback) {
        println!("BrowserApp: setting network callback");
        self.net_callback = Some(callback);
    }

    fn on_net_result(&mut self, result: net::FetchResult) {
        self.loading = false;
        self.last_status = Some(match (result.status, &result.error) {
            (Some(code), None) => format!("OK {code} — {} bytes", result.bytes),
            (Some(code), Some(err)) => format!("HTTP {code} — error: {err}"),
            (None, Some(err)) => format!("Network error: {err}"),
            _ => "Unknown".to_string(),
        });
        self.last_preview = result.snippet;
    }
}
