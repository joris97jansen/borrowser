use std::sync::Arc;

use egui::Context;
use net::FetchResult;

pub type NetCallback = Arc<dyn Fn(FetchResult) + Send + Sync>;

pub trait UiApp {
    fn ui(&mut self, ctx: &Context);
    fn set_net_callback(&mut self, callback: NetCallback);
    fn on_net_result(&mut self, result: FetchResult);
}
