use crate::page::PageState;

use html::Token;
use egui::{
    Context,
    TopBottomPanel,
    Key,
    CentralPanel,
    ScrollArea,
    Color32,
    Stroke,
    CornerRadius,
    Frame,
};

pub struct Panels {
    pub show_debug: bool,
}

pub fn top_bar(ctx: &Context, url: &mut String) -> bool {
    let mut go = false;
    TopBottomPanel::top("topbar").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.label("URL:");
            let response = ui.text_edit_singleline(url);
            if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) || ui.button("Go").clicked() {
                go = true;
            }
        });
    });
    go
}

pub fn content(
    ctx: &Context,
    page: &PageState,
    tokens_preview: &[Token],
    dom_outline: &[String],
    status: Option<&String>,
    loading: bool,
    panels: Panels,
) {
    CentralPanel::default().show(ctx, |ui| {
        if let Some(dom) = page.dom.as_ref() {
            ui.heading("Page review (early)");
            let bg = super::BrowserApp::page_background(dom).unwrap_or((255, 255, 255, 255));
            let bg_ui = Color32::from_rgba_unmultiplied(bg.0, bg.1, bg.2, bg.3);

            let mut text = String::new();
            let mut ancestors = Vec::new();
            super::BrowserApp::collect_visible_text(dom, &mut ancestors, &mut text);

            let fg = super::BrowserApp::inherited_color(dom, &[]);
            let fg_egui = Color32::from_rgba_unmultiplied(fg.0, fg.1, fg.2, fg.3);

            Frame::new()
                .fill(bg_ui)
                .stroke(Stroke::NONE)
                .corner_radius(CornerRadius::same(4))
                .show(ui, |ui| {
                    ui.set_min_height(200.0);
                    ui.add_space(6.0);
                    ui.style_mut().visuals.override_text_color = Some(fg_egui);
                    ui.label(text);
                    ui.style_mut().visuals.override_text_color = None;
                    // ui.add_space(6.0);
                });
        }

        if panels.show_debug {
            if !tokens_preview.is_empty() {
                ui.separator();
                ui.heading("Token preview");
                ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                    for (i, t) in tokens_preview.iter().enumerate() {
                        ui.monospace(format!("{i:02}: {:?}", t));
                    }
                });
            }
            if !dom_outline.is_empty() {
                ui.separator();
                ui.heading("DOM outline");
                ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
                    for line in dom_outline {
                        ui.monospace(line);
                    }
                });
            }
        }
        
        ui.separator();
        if loading { ui.label("⏳ Loading…"); }
        if let Some(s) = status { ui.label(s); }
    });
}