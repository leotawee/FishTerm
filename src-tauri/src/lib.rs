//! FishTerm core. See docs/adr/ADR-001-rewrite-in-tauri-rust.md.

mod cfaccess;
mod commands;
mod error;
mod secrets;
mod ssh;
mod sftp;
mod stats;
mod store;

use commands::AppState;
use secrets::KeyringStore;
use store::HostStore;
use tauri::menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder};
use tauri::{Emitter, Manager, WindowEvent};

/// Keychain service name. Changing it orphans every stored secret.
const KEYCHAIN_SERVICE: &str = "io.github.leotawee.fishterm";

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let builder = tauri::Builder::default();

    // Must be registered first: blocks a second process that would rewrite hosts.json
    // from stale memory. Release builds only — a `tauri dev` instance may legitimately
    // run next to the installed release app, and both share one identifier/socket.
    // `--multi` opts a demo build out of the lock so it can run beside production.
    #[cfg(not(debug_assertions))]
    let builder = if std::env::args().any(|a| a == "--multi") {
        builder
    } else {
        builder.plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.set_focus();
            }
        }))
    };

    builder
        // Only `open` is granted (capabilities/default.json): picks a key file path.
        .plugin(tauri_plugin_dialog::init())
        // Clipboard writes (OSC 52 from the terminal, Cmd+C on selection). Reads
        // never leave the webview: only allow-write-text is granted.
        .plugin(tauri_plugin_clipboard_manager::init())
        // Rust-side use only (`OpenerExt`): opens the validated Cloudflare Access
        // login URL. No capability entry: the webview has no open-URL primitive.
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let path = app.path().app_config_dir()?.join("hosts.json");
            let (store, warnings) = HostStore::open_or_recover(path)?;
            // Shared with OpenSSH: hosts trusted in one client are trusted in the other.
            let known_hosts = app.path().home_dir()?.join(".ssh").join("known_hosts");
            let secrets = Box::new(KeyringStore::new(KEYCHAIN_SERVICE));
            app.manage(AppState::new(store, secrets, warnings, known_hosts));

            // Native menu: Cmd+W / Cmd+Q accelerators fire here BEFORE the webview
            // sees any key, so the confirms live in App.vue behind these events.
            let quit = MenuItemBuilder::with_id("quit", "Quit FishTerm")
                .accelerator("Cmd+Q")
                .build(app)?;
            let close_tab = MenuItemBuilder::with_id("close_tab", "Close Tab")
                .accelerator("Cmd+W")
                .build(app)?;
            let app_menu = SubmenuBuilder::new(app, "FishTerm")
                .about(None)
                .separator()
                .item(&quit)
                .build()?;
            let file_menu = SubmenuBuilder::new(app, "File").item(&close_tab).build()?;
            let edit_menu = SubmenuBuilder::new(app, "Edit")
                .undo()
                .redo()
                .separator()
                .cut()
                .copy()
                .paste()
                .select_all()
                .build()?;
            let menu = MenuBuilder::new(app)
                .item(&app_menu)
                .item(&file_menu)
                .item(&edit_menu)
                .build()?;
            app.set_menu(menu)?;
            app.on_menu_event(|app, event| {
                let id = event.id().as_ref();
                if id == "quit" {
                    // Live terminals: let App.vue confirm before really quitting.
                    if app.state::<AppState>().open_session_count() > 0 {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.emit("quit-requested", ());
                        }
                    } else {
                        app.exit(0);
                    }
                } else if id == "close_tab" {
                    if let Some(w) = app.get_webview_window("main") {
                        let _ = w.emit("close-active-tab", ());
                    }
                }
            });

            // The red traffic-light close button goes through the same guard.
            if let Some(main) = app.get_webview_window("main") {
                let handle = app.handle().clone();
                main.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        if handle.state::<AppState>().open_session_count() > 0 {
                            api.prevent_close();
                            let _ = handle.get_webview_window("main").unwrap().emit("quit-requested", ());
                        }
                    }
                });
            }
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
            commands::session_stats,
            commands::hostkey_decide,
            commands::url_open,
            sftp::sftp_ls,
            sftp::sftp_download,
            sftp::sftp_upload,
            commands::quit_app,
        ])
        .run(tauri::generate_context!())
        .expect("failed to start FishTerm");
}
