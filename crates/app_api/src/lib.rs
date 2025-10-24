use std::sync::Arc;
use std::time::Duration;

use egui::Context;
use net::{
    FetchResult,
    NetEvent,
};

pub type NetCallback = Arc<dyn Fn(FetchResult) + Send + Sync>;
pub type NetStreamCallback = Arc<dyn Fn(NetEvent) + Send + Sync>;


pub trait UiApp {
    // ui
    fn ui(&mut self, ctx: &Context);

    // network
    fn set_net_callback(&mut self, callback: NetCallback);
    fn on_net_result(&mut self, result: FetchResult);

    fn on_net_stream(&mut self, _event: NetEvent) {}
    fn set_net_stream_callback(&mut self, _callback: NetStreamCallback) {}

    // repaint
    fn set_repaint_handle(&mut self, _h: RepaintHandle) {}
    fn needs_redraw(&self) -> bool { false }
}

pub trait Repaint: Send + Sync {
    fn request_now(&self);
    fn request_after(&self, duration: Duration);
}

pub type RepaintHandle = Arc<dyn Repaint>;
