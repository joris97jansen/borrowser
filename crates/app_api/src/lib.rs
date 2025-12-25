use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use bus::{CoreCommand, CoreEvent};
use egui::Context;
use net::NetEvent;

pub type NetStreamCallback = Arc<dyn Fn(NetEvent) + Send + Sync>;

pub trait UiApp {
    // ui
    fn ui(&mut self, ctx: &Context);

    // bus events from runtimes
    fn set_bus_sender(&mut self, _tx: mpsc::Sender<CoreCommand>) {}
    fn on_core_event(&mut self, _event: CoreEvent) {}

    // repaint
    fn set_repaint_handle(&mut self, _h: RepaintHandle) {}
    fn needs_redraw(&self) -> bool {
        false
    }
}

pub trait Repaint: Send + Sync {
    fn request_now(&self);
    fn request_after(&self, duration: Duration);
}

pub type RepaintHandle = Arc<dyn Repaint>;
