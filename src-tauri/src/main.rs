#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Initialize logger: RUST_LOG=debug for verbose, default is warn+errors only.
    // In debug builds defaults to info level; release builds default to warn.
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or(
            if cfg!(debug_assertions) { "debug" } else { "warn" },
        ),
    )
    .init();

    moontranslator_lib::run()
}
