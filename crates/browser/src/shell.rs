use egui::{
    Align, Align2, Button, Color32, Context, CornerRadius, FontId, Frame, Key, Margin, Rect,
    ScrollArea, Sense, Stroke, TextEdit, TopBottomPanel, Ui, pos2,
    scroll_area::ScrollBarVisibility, vec2,
};
use std::sync::mpsc;

use app_api::{RepaintHandle, UiApp};
use bus::{CoreCommand, CoreEvent};
use core_types::TabId;

use crate::tab::Tab;

pub struct ShellApp {
    tabs: Vec<Tab>,
    active: usize,

    cmd_tx: Option<mpsc::Sender<CoreCommand>>,
    repaint: Option<RepaintHandle>,

    next_tab_id: TabId,
}

impl ShellApp {
    pub fn new() -> Self {
        let mut s = Self {
            tabs: Vec::new(),
            active: 0,
            cmd_tx: None,
            repaint: None,
            next_tab_id: 1,
        };
        s.add_tab();
        s
    }

    fn alloc_tab_id(&mut self) -> TabId {
        let id = self.next_tab_id;
        self.next_tab_id = self.next_tab_id.wrapping_add(1);
        id
    }

    pub fn add_tab(&mut self) {
        let id = self.alloc_tab_id();
        let mut t = Tab::new(id);

        if let Some(tx) = &self.cmd_tx {
            t.set_bus_sender(tx.clone());
        }
        if let Some(rp) = &self.repaint {
            t.set_repaint_handle(rp.clone());
        }
        self.tabs.push(t);
        self.active = self.tabs.len() - 1;
        self.request_repaint();
    }

    fn close_at(&mut self, idx: usize) {
        // Cancel any in-flight request for that tab
        if let Some(tab) = self.tabs.get(idx) {
            if tab.nav_gen > 0 {
                if let Some(tx) = &self.cmd_tx {
                    let _ = tx.send(CoreCommand::CancelRequest {
                        tab_id: tab.tab_id,
                        request_id: tab.nav_gen,
                    });
                }
            }
        }

        // Remove the tab
        let removed_active = idx == self.active;
        self.tabs.remove(idx);

        // If no tabs remain → open a fresh one immediately
        if self.tabs.is_empty() {
            self.add_tab();
            self.active = 0;
        } else if removed_active {
            // If we closed the active tab, move focus to a logical neighbor
            self.active = self.active.min(self.tabs.len() - 1);
        } else if idx < self.active {
            // Shift active index left to keep pointing to same logical tab
            self.active -= 1;
        }

        self.request_repaint();
    }

    pub fn close_active(&mut self) {
        if self.tabs.is_empty() {
            return;
        }
        let idx = self.active;

        if let Some(tab) = self.tabs.get(idx) {
            if tab.nav_gen > 0 {
                if let Some(tx) = &self.cmd_tx {
                    let _ = tx.send(CoreCommand::CancelRequest {
                        tab_id: tab.tab_id,
                        request_id: tab.nav_gen,
                    });
                }
            }
        }

        self.tabs.remove(idx);
        if self.tabs.is_empty() {
            self.add_tab();
        } else {
            self.active = self.active.saturating_sub(1);
        }
        self.request_repaint();
    }

    fn active_tab_mut(&mut self) -> &mut Tab {
        &mut self.tabs[self.active]
    }

    fn active_tab(&self) -> &Tab {
        &self.tabs[self.active]
    }

    // --- UI helpers ---
    fn request_repaint(&self) {
        if let Some(r) = &self.repaint {
            r.request_now();
        }
    }

    fn ui_tabstrip(&mut self, ui: &mut Ui) {
        let h = 32.0;
        let tab_w = 160.0;
        let close_w = 22.0;

        // Defer the actual close until after the loop to avoid borrow/index issues
        let mut close_idx: Option<usize> = None;

        ScrollArea::horizontal()
            .scroll_bar_visibility(ScrollBarVisibility::AlwaysHidden)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;

                    for (i, t) in self.tabs.iter().enumerate() {
                        // Reserve the full tab rect
                        let (tab_rect, tab_resp) =
                            ui.allocate_exact_size(vec2(tab_w, h), Sense::click());

                        // Colors
                        let inactive_fill = Color32::from_rgb(70, 70, 70);
                        let active_fill   = Color32::from_rgb(120, 120, 120);
                        let hovered_fill  = Color32::from_rgb(90, 90, 90);

                        let vis = ui.visuals();
                        let rounding = CornerRadius::same(8);
                        let fill = if i == self.active {
                            active_fill
                        } else if tab_resp.hovered() {
                            hovered_fill
                        } else {
                            inactive_fill
                        };

                        // ---- 1) Background FIRST ----
                        ui.painter().rect_filled(tab_rect, rounding, fill);

                        // ---- 2) Close button rect ----
                        let close_rect = Rect::from_min_max(
                            pos2(tab_rect.right() - close_w - 6.0, tab_rect.top() + 4.0),
                            pos2(tab_rect.right() - 6.0,           tab_rect.bottom() - 4.0),
                        );

                        let close_id = ui.make_persistent_id(("tab_close", i));
                        let close_resp = ui.interact(close_rect, close_id, Sense::click());

                        // Hover/active backdrop
                        if close_resp.hovered() {
                            ui.painter().rect_filled(
                                close_rect,
                                CornerRadius::same(6),
                                vis.widgets.hovered.bg_fill,
                            );
                        }
                        if close_resp.is_pointer_button_down_on() {
                            ui.painter().rect_filled(
                                close_rect,
                                CornerRadius::same(6),
                                vis.widgets.active.bg_fill,
                            );
                        }

                        // ✖ glyph on top of background
                        ui.painter().text(
                            close_rect.center(),
                            Align2::CENTER_CENTER,
                            "✖",
                            FontId::proportional(12.5),
                            vis.widgets.inactive.fg_stroke.color,
                        );

                        if close_resp.clicked() {
                            close_idx = Some(i);
                        }

                        // ---- 3) Text, clipped to "tab minus close" ----
                        let title = t.display_title();

                        // Area reserved for text only
                        let text_rect = Rect::from_min_max(
                            pos2(tab_rect.left() + 12.0, tab_rect.top()),
                            pos2(close_rect.left() - 4.0, tab_rect.bottom()),
                        );

                        if text_rect.width() > 0.0 {
                            let tab_painter = ui.painter_at(text_rect);
                            let text_pos = pos2(text_rect.left(), text_rect.center().y);

                            tab_painter.text(
                                text_pos,
                                Align2::LEFT_CENTER,
                                title,
                                FontId::proportional(13.0),
                                vis.widgets.inactive.fg_stroke.color,
                            );
                        }

                        // ---- 4) Tab activation click ----
                        if tab_resp.clicked() {
                            self.active = i;
                            self.request_repaint();
                        }
                    }

                    let plus_size = vec2(28.0, h);
                    let (plus_rect, plus_resp) = ui.allocate_exact_size(plus_size, Sense::click());
                    let vis = ui.visuals();
                    if plus_resp.hovered() {
                        ui.painter().rect_filled(
                            plus_rect,
                            CornerRadius::same(6),
                            vis.widgets.hovered.bg_fill,
                        );
                    }
                    if plus_resp.is_pointer_button_down_on() {
                        ui.painter().rect_filled(
                            plus_rect,
                            CornerRadius::same(6),
                            vis.widgets.active.bg_fill,
                        );
                    }
                    ui.painter().text(
                        plus_rect.center(),
                        Align2::CENTER_CENTER,
                        "+",
                        FontId::proportional(14.0),
                        vis.widgets.inactive.fg_stroke.color,
                    );
                    if plus_resp.clicked() {
                        self.add_tab();
                    }
                });
            });

        if let Some(i) = close_idx {
            self.close_at(i);
        }
    }

    fn ui_urlbar(&mut self, ui: &mut Ui) {
        let h = 40.0; // unified height

        // Back / Forward / Refresh
        let t = self.active_tab_mut();
        let can_back = t.history_index > 0;
        let can_forward = t.history_index + 1 < t.history.len();

        // Square buttons matching URL bar height
        if ui
            .add_enabled(can_back, Button::new("⬅").min_size([h, h].into()))
            .clicked()
        {
            t.go_back();
        }
        if ui
            .add_enabled(can_forward, Button::new("➡").min_size([h, h].into()))
            .clicked()
        {
            t.go_forward();
        }
        if ui.add(Button::new("🔄").min_size([h, h].into())).clicked() {
            t.refresh();
        }

        ui.add_space(6.0);

        // ---- URL input frame ----
        let resp = Frame::new()
            .stroke(Stroke::new(
                1.0,
                ui.visuals().widgets.inactive.bg_stroke.color,
            ))
            .corner_radius(CornerRadius::same(6))
            .inner_margin(Margin::symmetric(6, 4))
            .show(ui, |ui| {
                let t = self.active_tab_mut();
                ui.add_sized(
                    [ui.available_width(), h - 8.0],
                    TextEdit::singleline(&mut t.url)
                        .hint_text("Enter URL")
                        .vertical_align(Align::Center),
                )
            })
            .inner;

        if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
            let url = self.active_tab().url.clone();
            self.active_tab_mut().navigate_to_new(url);
            self.request_repaint();
        }
    }
}

impl UiApp for ShellApp {
    fn ui(&mut self, ctx: &Context) {
        TopBottomPanel::top("Browser Shell")
            .frame(Frame::new().inner_margin(Margin::symmetric(0, 0)))
            .show(ctx, |ui| {
                // Consistent vertical spacing
                ui.spacing_mut().item_spacing.y = 0.0;

                // ---- Tabstrip ----
                Frame::new()
                    .fill(Color32::from_rgb(30, 30, 30)) // Darker grey for the whole tab bar
                    .inner_margin(Margin::symmetric(6, 6))
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        self.ui_tabstrip(ui);
                    });

                // ---- URL bar ----
                Frame::new()
                    .fill(Color32::from_rgb(40, 40, 40)) // slightly lighter grey
                    .inner_margin(Margin::symmetric(6, 6))
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            self.ui_urlbar(ui);
                        });
                    });
            });

        // ---- Page content below ----
        self.active_tab_mut().ui_content(ctx);
    }

    fn set_bus_sender(&mut self, tx: mpsc::Sender<CoreCommand>) {
        self.cmd_tx = Some(tx.clone());
        for t in &mut self.tabs {
            t.set_bus_sender(tx.clone());
        }
    }

    fn on_core_event(&mut self, evt: CoreEvent) {
        let sid = match &evt {
            CoreEvent::NetworkStart { tab_id, .. }
            | CoreEvent::NetworkChunk { tab_id, .. }
            | CoreEvent::NetworkDone { tab_id, .. }
            | CoreEvent::NetworkError { tab_id, .. }
            | CoreEvent::DomUpdate { tab_id, .. }
            | CoreEvent::CssParsedBlock { tab_id, .. }
            | CoreEvent::CssSheetDone { tab_id, .. } => *tab_id,
        };
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.tab_id == sid) {
            tab.on_core_event(evt);
        }
    }

    fn set_repaint_handle(&mut self, h: RepaintHandle) {
        self.repaint = Some(h);
    }
}