use std::sync::Arc;
use std::time::Duration;

use egui::Context;
use net::FetchResult;

pub type NetCallback = Arc<dyn Fn(FetchResult) + Send + Sync>;
pub type RepaintHandle = Arc<dyn Repaint>;

pub trait UiApp {
    fn ui(&mut self, ctx: &Context);

    fn set_net_callback(&mut self, callback: NetCallback);
    fn on_net_result(&mut self, result: FetchResult);

    fn set_repaint_handle(&mut self, _h: RepaintHandle) {}
    fn needs_redraw(&self) -> bool { false }
}

pub trait Repaint: Send + Sync {
    fn request_now(&self);
    fn request_after(&self, duration: Duration);
}
