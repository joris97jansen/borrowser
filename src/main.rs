#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() {
    let app = browser::ShellApp::new();
    platform::run_with(app);
}
