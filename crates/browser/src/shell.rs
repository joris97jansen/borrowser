use std::sync::mpsc;
use egui::{
    Context,
    TopBottomPanel,
    Button,
    Align,
    Key,
    Frame,
    Margin,
};

use app_api::{UiApp, RepaintHandle};
use bus::{
    CoreCommand,
    CoreEvent
};
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
    }

    pub fn close_active(&mut self) {
        if self.tabs.is_empty() { return; }
        let idx = self.active;

        if let Some(tab) = self.tabs.get(idx) {
            if tab.nav_gen > 0 {
                if let Some(tx) = &self.cmd_tx {
                    let _ = tx.send(CoreCommand::CancelRequest { tab_id: tab.tab_id, request_id: tab.nav_gen });
                }
            }
        }

        self.tabs.remove(idx);
        if self.tabs.is_empty() {
            // altijd minstens 1 tab: maak een nieuwe
            self.add_tab();
        } else {
            self.active = self.active.saturating_sub(1);
        }
    }

    fn active_tab_mut(&mut self) -> &mut Tab { 
        &mut self.tabs[self.active] 
    }

    fn active_tab(&self) -> &Tab { 
        &self.tabs[self.active] 
    }

    // --- UI helpers ---
    fn ui_tabstrip(&mut self, ui: &mut egui::Ui) {
        for (i, t) in self.tabs.iter().enumerate() {
            let title = if t.url.is_empty() { 
                "New Tab" 
            } else {
                &t.url
            };
            let b = ui.selectable_label(
                i == self.active, title
            );
            if b.clicked() {
                self.active = i;
            }
        }
        if ui.button("+").clicked() {
            self.add_tab();
        }
        if ui.button("✖").clicked() {
            self.close_active();
        }
    }

    fn ui_urlbar(&mut self, ui: &mut egui::Ui) {
        // back/forward/refresh
        let t = self.active_tab_mut();
        let can_back = t.history_index > 0;
        let can_forward = t.history_index + 1 < t.history.len();

        if ui.add_enabled(can_back, Button::new("⬅")).clicked() { t.go_back(); }
        if ui.add_enabled(can_forward, Button::new("➡")).clicked() { t.go_forward(); }
        if ui.button("🔄").clicked() { t.refresh(); }

        // url input
        let resp = Frame::new()
            .fill(ui.visuals().extreme_bg_color)
            .stroke(egui::Stroke::new(1.0, ui.visuals().widgets.inactive.bg_stroke.color))
            .corner_radius(6.0)
            .inner_margin(Margin::symmetric(4, 4))
            .show(ui, |ui| {
                let t = self.active_tab_mut();
                ui.add_sized([ui.available_width(), 28.0],
                    egui::TextEdit::singleline(&mut t.url).hint_text("Enter URL").vertical_align(Align::Center),
                )
            }).inner;

        if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
            let url = self.active_tab().url.clone();
            self.active_tab_mut().navigate_to_new(url);
        }
    }
}

impl UiApp for ShellApp {
    fn ui(&mut self, ctx: &Context) {
        TopBottomPanel::top("Browser Shell").show(ctx, |ui| {
            ui.horizontal(|ui| {
                self.ui_tabstrip(ui);
            });
            ui.separator();
            ui.horizontal(|ui| {
                self.ui_urlbar(ui);
            });
        });

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
            CoreEvent::NetworkStart{tab_id, ..}
            | CoreEvent::NetworkChunk{tab_id, ..}
            | CoreEvent::NetworkDone{tab_id, ..}
            | CoreEvent::NetworkError{tab_id, ..}
            | CoreEvent::DomUpdate{tab_id, ..}
            | CoreEvent::CssParsedBlock{tab_id, ..}
            | CoreEvent::CssSheetDone{tab_id, ..} => *tab_id,
        };
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.tab_id == sid) {
            tab.on_core_event(evt);
        }
    }

    fn set_repaint_handle(&mut self, h: RepaintHandle) { self.repaint = Some(h); }
}
