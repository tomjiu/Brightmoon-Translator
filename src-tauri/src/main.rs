#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    use tracing_subscriber::EnvFilter;

    let filter = if cfg!(debug_assertions) {
        EnvFilter::new("debug")
    } else {
        EnvFilter::new("warn")
    };

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

    moontranslator_lib::run()
}
