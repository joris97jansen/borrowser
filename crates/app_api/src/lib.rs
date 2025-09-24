use egui::Context;

pub trait UiApp {
    fn ui(&mut self, ctx: &Context);
}
