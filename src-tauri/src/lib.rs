//! FishTerm core. See docs/adr/ADR-001-rewrite-in-tauri-rust.md.

mod commands;
mod error;
mod secrets;
mod ssh;
mod store;

use commands::AppState;
use secrets::KeyringStore;
use store::HostStore;
use tauri::Manager;

/// Keychain service name. Changing it orphans every stored secret.
const KEYCHAIN_SERVICE: &str = "io.github.leotawee.fishterm";

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tauri::Builder::default()
        // Only `open` is granted (capabilities/default.json): picks a key file path.
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let path = app.path().app_config_dir()?.join("hosts.json");
            let (store, warnings) = HostStore::open_or_recover(path)?;
            // Shared with OpenSSH: hosts trusted in one client are trusted in the other.
            let known_hosts = app.path().home_dir()?.join(".ssh").join("known_hosts");
            let secrets = Box::new(KeyringStore::new(KEYCHAIN_SERVICE));
            app.manage(AppState::new(store, secrets, warnings, known_hosts));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::hosts_list,
            commands::host_save,
            commands::host_delete,
            commands::host_set_secret,
            commands::host_clear_secret,
            commands::startup_warnings,
            commands::session_open,
            commands::session_write,
            commands::session_resize,
            commands::session_close,
            commands::hostkey_decide,
        ])
        .run(tauri::generate_context!())
        .expect("failed to start FishTerm");
}
