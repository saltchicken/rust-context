mod app;

use env_logger::Env;

fn main() {
    // Initialize logging (default to info if not set)
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    // Delegate to app module
    if let Err(e) = app::run() {
        log::error!("❌ Application error: {:?}", e);
        std::process::exit(1);
    }
}
