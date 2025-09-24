// browser/src/app.rs
pub struct BrowserApp {
    url: String,
}

impl BrowserApp {
    pub fn new() -> Self {
        Self { url: "https://example.com".into() }
    }

    pub fn ui(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("topbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("URL:");
                let resp = ui.text_edit_singleline(&mut self.url);
                if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    eprintln!("Navigate to: {}", self.url);
                }
                if ui.button("Go").clicked() {
                    eprintln!("Navigate to: {}", self.url);
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label("Page view coming soon…");
        });
    }
}

