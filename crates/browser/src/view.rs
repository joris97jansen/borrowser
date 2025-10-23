use crate::page::PageState;
use crate::BrowserApp;

use html::{
    Node,
    Token,
};
use egui::{
    Align,
    Button,
    Context,
    TopBottomPanel,
    Key,
    CentralPanel,
    ScrollArea,
    Color32,
    Stroke,
    CornerRadius,
    Frame,
    TextEdit,
    Margin,
    Ui,
    RichText,
};

pub enum NavigationAction {
    None,
    Back,
    Forward,
    Refresh,
    Navigate(String),
}

pub struct Panels {
    pub show_debug: bool,
}


pub fn top_bar(ctx: &Context, app: &mut BrowserApp) -> NavigationAction {
    let mut action = NavigationAction::None;

    const BAR_HEIGHT: f32 = 36.0;
    const BUTTON_WIDTH: f32 = 36.0;

    TopBottomPanel::top("topbar").show(ctx, |ui| {
        ui.horizontal(|ui| {
            let can_go_back = app.history_index > 0;
            let can_go_forward = app.history_index + 1 < app.history.len();

            let spacing = ui.spacing().item_spacing.x;
            let available_width = ui.available_width();
            let url_width = available_width - (BUTTON_WIDTH * 3.0 + spacing + 3.0);

            if ui.add_enabled(can_go_back, Button::new("⬅").min_size([BUTTON_WIDTH, BAR_HEIGHT].into())).clicked() {
                action = NavigationAction::Back;
            }
            if ui.add_enabled(can_go_forward, Button::new("➡").min_size([BUTTON_WIDTH, BAR_HEIGHT].into())).clicked() {
                action = NavigationAction::Forward;
            }
            if ui.add_sized([BUTTON_WIDTH, BAR_HEIGHT], Button::new("🔄")).clicked()
            {
                action = NavigationAction::Refresh;
            }

            let response = Frame::new()
                .fill(ui.visuals().extreme_bg_color) // subtle background
                .stroke(egui::Stroke::new(1.0, ui.visuals().widgets.inactive.bg_stroke.color))
                .rounding(6.0)
                .inner_margin(Margin::symmetric(4, 4))
                .show(ui, |ui| {
                    ui.add_sized(
                        [ui.available_width(), BAR_HEIGHT - 8.0],
                        egui::TextEdit::singleline(&mut app.url)
                            .hint_text("Enter URL")
                            .vertical_align(Align::Center),
                    )
                })
                .inner;

            if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                action = NavigationAction::Navigate(app.url.clone());
            }
        });
    });
    action
}

pub fn content(
    ctx: &Context,
    page: &PageState,
    dom_outline: &[String],
    status: Option<&String>,
    loading: bool,
    panels: Panels,
) {
    CentralPanel::default()
        .frame(Frame::default())
        .show(ctx, |ui| {
        if let Some(dom) = page.dom.as_ref() {
            page_viewport(ui, dom);
        }

        if loading { ui.label("⏳ Loading…"); }
        if let Some(s) = status { ui.label(s); }
    });
}

pub fn page_viewport(ui: &mut Ui, dom: &Node) {
    // 1) background color
    let background = BrowserApp::page_background(dom).unwrap_or((255, 255, 255, 255));
    let background_ui = Color32::from_rgba_unmultiplied(background.0, background.1, background.2, background.3);

    // 2) collect visible text
    let mut text = String::new();
    let mut ancestors = Vec::new();
    BrowserApp::collect_visible_text(dom, &mut ancestors, &mut text);

    // 3) inherited text color
    let fg = BrowserApp::inherited_color(dom, &[]);
    let fg_ui = Color32::from_rgba_unmultiplied(fg.0, fg.1, fg.2, fg.3);

    // 4) page surface + scroll
    ScrollArea::vertical()
        .id_source("page_viewport_scroll_area")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let min_height = ui.available_height();

            Frame::new()
                .fill(background_ui)
                .show(ui, |ui| {
                    ui.style_mut().visuals.override_text_color = Some(fg_ui);
                    ui.set_width(ui.available_width());
                    ui.set_min_height(min_height);
                    ui.label(
                        // Placeholder; CSS driven later
                        RichText::new(text).size(16.0)
                    );

                    ui.style_mut().visuals.override_text_color = None;
                });
        });
}
