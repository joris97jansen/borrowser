use core_types::BrowserInput;
use egui::{
    Align, Button, Context, CornerRadius, Frame, Margin, Stroke, TextEdit, TopBottomPanel, Ui,
};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct NavigationIntent {
    pub go_back: bool,
    pub go_forward: bool,
    pub refresh: bool,
    pub navigate_to: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct NavigationWidgetsConfig {
    pub height: f32,
}

impl Default for NavigationWidgetsConfig {
    fn default() -> Self {
        Self { height: 40.0 }
    }
}

pub fn top_bar(
    ctx: &Context,
    url: &mut String,
    can_go_back: bool,
    can_go_forward: bool,
    input: BrowserInput,
) -> NavigationIntent {
    let mut intent = NavigationIntent::default();
    TopBottomPanel::top("borrowser_topbar").show(ctx, |ui| {
        ui.horizontal(|ui| {
            intent = navigation_widgets(ui, url, can_go_back, can_go_forward, input);
        });
    });
    intent
}

pub fn navigation_widgets(
    ui: &mut Ui,
    url: &mut String,
    can_go_back: bool,
    can_go_forward: bool,
    input: BrowserInput,
) -> NavigationIntent {
    navigation_widgets_with_config(
        ui,
        url,
        can_go_back,
        can_go_forward,
        NavigationWidgetsConfig::default(),
        input,
    )
}

pub fn navigation_widgets_with_config(
    ui: &mut Ui,
    url: &mut String,
    can_go_back: bool,
    can_go_forward: bool,
    config: NavigationWidgetsConfig,
    input: BrowserInput,
) -> NavigationIntent {
    let mut intent = NavigationIntent::default();
    let h = config.height.max(1.0);

    if ui
        .add_enabled(can_go_back, Button::new("⬅").min_size([h, h].into()))
        .clicked()
    {
        intent.go_back = true;
    }
    if ui
        .add_enabled(can_go_forward, Button::new("➡").min_size([h, h].into()))
        .clicked()
    {
        intent.go_forward = true;
    }
    if ui.add(Button::new("🔄").min_size([h, h].into())).clicked() {
        intent.refresh = true;
    }

    ui.add_space(6.0);

    let resp = Frame::new()
        .stroke(Stroke::new(
            1.0,
            ui.visuals().widgets.inactive.bg_stroke.color,
        ))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::symmetric(6, 4))
        .show(ui, |ui| {
            ui.add_sized(
                [ui.available_width(), h - 8.0],
                TextEdit::singleline(url)
                    .return_key(None)
                    .hint_text("Enter URL")
                    .vertical_align(Align::Center),
            )
        })
        .inner;

    if input.enter_pressed && resp.has_focus() {
        intent.navigate_to = Some(url.clone());
    }

    intent
}
