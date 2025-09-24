use egui::{
    Context,
    TopBottomPanel,
    Key,
    CentralPanel,
};

pub struct BrowserApp {
    url: String,
}

impl BrowserApp {
    pub fn new() -> Self {
        Self{
            url: "https://example.com".into(),
        }
    }
}

impl app_api::UiApp for BrowserApp {
    fn ui(&mut self, context: &Context) {
        TopBottomPanel::top("topbar").show(context, |ui| {
            ui.horizontal(|ui| {
                ui.label("URL:");
                let response = ui.text_edit_singleline(&mut self.url);
                if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                    eprintln!("Loading URL: {}", self.url);
                }
                if ui.button("Go").clicked() {
                    eprintln!("Loading URL: {}", self.url);
                }
            });
        });
        CentralPanel::default().show(context, |ui| {
            ui.label("Coming Soon...");
        });
    }
}
