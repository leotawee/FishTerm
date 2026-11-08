//! MEO FishTerm core. See docs/adr/ADR-001-rewrite-in-tauri-rust.md.

mod commands;
mod error;
mod secrets;
mod store;

use commands::AppState;
use secrets::KeyringStore;
use store::HostStore;
use tauri::Manager;

/// Keychain service name. Changing it orphans every stored secret.
const KEYCHAIN_SERVICE: &str = "dev.meo.fishterm";

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tauri::Builder::default()
        .setup(|app| {
            let path = app.path().app_config_dir()?.join("hosts.json");
            let (store, warnings) = HostStore::open_or_recover(path)?;
            app.manage(AppState::new(
                store,
                Box::new(KeyringStore::new(KEYCHAIN_SERVICE)),
                warnings,
            ));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::hosts_list,
            commands::host_save,
            commands::host_delete,
            commands::host_set_secret,
            commands::host_clear_secret,
            commands::startup_warnings,
        ])
        .run(tauri::generate_context!())
        .expect("failed to start MEO FishTerm");
}
