#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() {
    let app = browser::BrowserApp::new();
    platform::run_with(app);
}
