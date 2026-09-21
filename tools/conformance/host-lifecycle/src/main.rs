fn main() {
    if let Err(e) = borrowser_host_lifecycle::run_cli() {
        eprintln!("host-lifecycle: {e}");
        std::process::exit(1);
    }
}
