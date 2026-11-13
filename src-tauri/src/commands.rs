//! Tauri IPC surface. Thin wrappers over `AppState` methods so the logic is testable
//! without a Tauri runtime. No command returns a secret (threat model §7).

use crate::cfaccess;
use crate::error::{AppError, AppResult};
use crate::secrets::{SecretStore, host_account};
use crate::ssh::{self, Ask, Cmd, Credential, Target};
use crate::stats::{self, Sample, Stats};
use crate::store::{Auth, Host, HostInput, HostStore, Transport};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{AppHandle, Manager, State};
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

const MAX_SECRET_LEN: usize = 8192;
const MAX_WRITE_LEN: usize = 64 * 1024;
const MAX_TERM_DIM: u32 = 1000;
const STATS_WAIT: Duration = Duration::from_secs(6);
// Linux only; missing /proc entries simply leave the matching footer field as `--`.
const STATS_CMD: &str =
    "cat /proc/stat /proc/meminfo /proc/net/dev 2>/dev/null; df -k / 2>/dev/null";

pub struct AppState {
    store: Mutex<HostStore>,
    secrets: Box<dyn SecretStore>,
    warnings: Mutex<Vec<String>>,
    known_hosts: PathBuf,
    sessions: Mutex<HashMap<Uuid, mpsc::UnboundedSender<Cmd>>>,
    /// Previous /proc snapshot per session, for CPU% and network-rate deltas.
    samples: Mutex<HashMap<Uuid, (Sample, Instant)>>,
    prompts: Mutex<HashMap<Uuid, oneshot::Sender<bool>>>,
}

impl AppState {
    pub fn new(
        store: HostStore,
        secrets: Box<dyn SecretStore>,
        warnings: Vec<String>,
        known_hosts: PathBuf,
    ) -> Self {
        Self {
            store: Mutex::new(store),
            secrets,
            warnings: Mutex::new(warnings),
            known_hosts,
            sessions: Mutex::new(HashMap::new()),
            samples: Mutex::new(HashMap::new()),
            prompts: Mutex::new(HashMap::new()),
        }
    }

    /// Build connection parameters for a direct host. The secret is read from the
    /// keychain here and never leaves the Rust core.
    pub fn target_for(&self, id: Uuid) -> AppResult<Target> {
        let host = self.store()?.get(&id).cloned().ok_or(AppError::NotFound)?;
        Ok(Target {
            host: host.host,
            port: host.port,
            username: host.username,
            credential: self.credential_for(id)?,
            known_hosts: self.known_hosts.clone(),
        })
    }

    /// Read the host's SSH credential from the keychain. Shared by every
    /// transport: behind Cloudflare Access the SSH auth to sshd is unchanged.
    pub fn credential_for(&self, id: Uuid) -> AppResult<Credential> {
        let host = self.store()?.get(&id).cloned().ok_or(AppError::NotFound)?;
        let secret = if host.has_secret {
            self.secrets.get(&host_account(&id))?
        } else {
            None
        };
        Ok(match host.auth {
            Auth::Password => Credential::Password(
                secret.ok_or(AppError::Invalid("no password saved for this host"))?,
            ),
            Auth::Key { path } => Credential::Key {
                path,
                passphrase: secret,
            },
        })
    }

    fn send(&self, id: Uuid, cmd: Cmd) -> AppResult<()> {
        let sessions = self.sessions.lock().map_err(|_| AppError::Internal)?;
        sessions
            .get(&id)
            .ok_or(AppError::NotFound)?
            .send(cmd)
            .map_err(|_| AppError::NotFound)
    }

    pub fn write(&self, id: Uuid, data: String) -> AppResult<()> {
        if data.len() > MAX_WRITE_LEN {
            return Err(AppError::Invalid("write too large"));
        }
        self.send(id, Cmd::Data(data.into_bytes()))
    }

    pub fn resize(&self, id: Uuid, cols: u32, rows: u32) -> AppResult<()> {
        check_dims(cols, rows)?;
        self.send(id, Cmd::Resize { cols, rows })
    }

    /// Footer stats: exec a `/proc`+`df` snapshot on a separate channel, diff against
    /// the previous sample. First call per session yields rates as `None`.
    pub async fn stats(&self, id: Uuid) -> AppResult<Stats> {
        let (tx, rx) = oneshot::channel();
        self.send(
            id,
            Cmd::Exec {
                command: STATS_CMD.into(),
                reply: tx,
            },
        )?;
        let text = match tokio::time::timeout(STATS_WAIT, rx).await {
            Ok(Ok(text)) => text?,
            Ok(Err(_)) => return Err(AppError::NotFound), // shell loop already gone
            Err(_) => return Err(AppError::Timeout),
        };
        let now = stats::parse(&text);
        let prev = self
            .samples
            .lock()
            .map_err(|_| AppError::Internal)?
            .insert(id, (now, Instant::now()));
        let secs = prev
            .as_ref()
            .map(|(_, at)| at.elapsed().as_secs_f64())
            .unwrap_or(0.0);
        Ok(stats::diff(prev.map(|(s, _)| s), now, secs))
    }

    /// Live session count for the quit/close guards (App.vue confirms before exit).
    pub fn open_session_count(&self) -> usize {
        self.sessions.lock().map(|s| s.len()).unwrap_or(0)
    }

    pub fn decide(&self, prompt_id: Uuid, accept: bool) -> AppResult<()> {
        let tx = self
            .prompts
            .lock()
            .map_err(|_| AppError::Internal)?
            .remove(&prompt_id);
        // A dropped receiver means the prompt already timed out; nothing else to do.
        let _ = tx.ok_or(AppError::NotFound)?.send(accept);
        Ok(())
    }

    // WHY: the store lock is held across keychain calls so a concurrent delete cannot
    // leave an orphaned secret or a has_secret flag that lies.
    fn store(&self) -> AppResult<MutexGuard<'_, HostStore>> {
        self.store.lock().map_err(|_| AppError::Internal)
    }

    pub fn list(&self) -> AppResult<Vec<Host>> {
        Ok(self.store()?.list().to_vec())
    }

    pub fn save(&self, input: HostInput) -> AppResult<Host> {
        let mut store = self.store()?;
        let saved = store.upsert(input)?;
        if saved.secret_invalidated {
            self.secrets.delete(&host_account(&saved.host.id))?;
        }
        tracing::info!(host_id = %saved.host.id, "host saved");
        Ok(saved.host)
    }

    pub fn delete(&self, id: Uuid) -> AppResult<()> {
        let mut store = self.store()?;
        store.get(&id).ok_or(AppError::NotFound)?;
        // Secret first: if the keychain fails, the host stays and the user can retry.
        self.secrets.delete(&host_account(&id))?;
        store.delete(&id)?;
        tracing::info!(host_id = %id, "host deleted");
        Ok(())
    }

    pub fn set_secret(&self, id: Uuid, secret: String) -> AppResult<()> {
        if secret.is_empty() || secret.len() > MAX_SECRET_LEN {
            return Err(AppError::Invalid("secret must be 1-8192 bytes"));
        }
        let mut store = self.store()?;
        store.get(&id).ok_or(AppError::NotFound)?;
        // TECH DEBT: `secret` is a plain String; zeroize on drop once russh lands (Phase 3).
        self.secrets.set(&host_account(&id), &secret)?;
        store.set_has_secret(&id, true)
    }

    pub fn clear_secret(&self, id: Uuid) -> AppResult<()> {
        let mut store = self.store()?;
        store.get(&id).ok_or(AppError::NotFound)?;
        self.secrets.delete(&host_account(&id))?;
        store.set_has_secret(&id, false)
    }

    /// Returns pending warnings once, then clears them.
    pub fn take_warnings(&self) -> AppResult<Vec<String>> {
        Ok(std::mem::take(
            &mut *self.warnings.lock().map_err(|_| AppError::Internal)?,
        ))
    }
}

/// Log the detailed error server-side; the UI only gets the generic `Display`.
fn logged<T>(r: AppResult<T>) -> AppResult<T> {
    if let Err(e) = &r {
        tracing::error!(error = ?e, "command failed");
    }
    r
}

#[tauri::command]
pub fn hosts_list(state: State<'_, AppState>) -> AppResult<Vec<Host>> {
    logged(state.list())
}

#[tauri::command]
pub fn host_save(state: State<'_, AppState>, input: HostInput) -> AppResult<Host> {
    logged(state.save(input))
}

#[tauri::command]
pub fn host_delete(state: State<'_, AppState>, id: Uuid) -> AppResult<()> {
    logged(state.delete(id))
}

#[tauri::command]
pub fn host_set_secret(state: State<'_, AppState>, id: Uuid, secret: String) -> AppResult<()> {
    logged(state.set_secret(id, secret))
}

#[tauri::command]
pub fn host_clear_secret(state: State<'_, AppState>, id: Uuid) -> AppResult<()> {
    logged(state.clear_secret(id))
}

#[tauri::command]
pub fn startup_warnings(state: State<'_, AppState>) -> AppResult<Vec<String>> {
    logged(state.take_warnings())
}

fn check_dims(cols: u32, rows: u32) -> AppResult<()> {
    if (1..=MAX_TERM_DIM).contains(&cols) && (1..=MAX_TERM_DIM).contains(&rows) {
        Ok(())
    } else {
        Err(AppError::Invalid("terminal size out of range"))
    }
}

fn send_json(events: &Channel, value: serde_json::Value) {
    let _ = events.send(InvokeResponseBody::Json(value.to_string()));
}

/// Events on `events`: raw bytes = terminal output; JSON `{type:"hostkey",prompt_id,prompt}`
/// asks the user (answer with `hostkey_decide`); JSON `{type:"closed",error}` ends the session (`error` is null or `{code,message}`).
#[tauri::command]
pub async fn session_open(
    app: AppHandle,
    host_id: Uuid,
    cols: u32,
    rows: u32,
    events: Channel,
) -> AppResult<Uuid> {
    logged(open_session(app, host_id, cols, rows, events).await)
}

async fn open_session(
    app: AppHandle,
    host_id: Uuid,
    cols: u32,
    rows: u32,
    events: Channel,
) -> AppResult<Uuid> {
    check_dims(cols, rows)?;
    let state = app.state::<AppState>();
    let host = state
        .store()?
        .get(&host_id)
        .cloned()
        .ok_or(AppError::NotFound)?;
    let credential = state.credential_for(host_id)?;

    let (ask_app, ask_events) = (app.clone(), events.clone());
    let ask: Ask = Box::new(move |prompt| {
        let (tx, rx) = oneshot::channel();
        let prompt_id = Uuid::new_v4();
        // shortcut: an unanswered prompt's sender stays in the map until app exit (a few bytes); prune if that ever matters.
        if let Ok(mut p) = ask_app.state::<AppState>().prompts.lock() {
            p.insert(prompt_id, tx);
        }
        send_json(
            &ask_events,
            serde_json::json!({"type": "hostkey", "prompt_id": prompt_id, "prompt": prompt}),
        );
        rx
    });

    let handle = match host.transport {
        Transport::Direct => ssh::connect(state.target_for(host_id)?, ask).await?,
        Transport::CloudflareAccess => {
            // ADR-001: Rust validates the login URL and opens the browser itself;
            // the webview only receives progress statuses on the event channel.
            let opener_app = app.clone();
            let open_browser =
                move |url: String| {
                    use tauri_plugin_opener::OpenerExt;
                    opener_app.opener().open_url(url, None::<&str>).map_err(|e| {
                    tracing::warn!(error = ?e, "could not open the sign-in URL in a browser");
                    AppError::CfLogin
                })
                };
            let mut on_status = |status: &str, attempt: u8| {
                send_json(
                    &events,
                    serde_json::json!({"type": "cf-login", "status": status, "attempt": attempt}),
                );
            };
            let cf = cfaccess::open_session_stream(
                &host.host,
                state.secrets.as_ref(),
                &open_browser,
                &mut on_status,
            )
            .await?;
            // known_hosts identity is the canonical Access domain, not an edge IP.
            let target = Target {
                host: cf.domain,
                port: host.port,
                username: host.username.clone(),
                credential,
                known_hosts: state.known_hosts.clone(),
            };
            ssh::connect_on(target, ask, cf.pipe).await?
        }
    };

    let (tx, rx) = mpsc::unbounded_channel();
    let sid = Uuid::new_v4();
    app.state::<AppState>()
        .sessions
        .lock()
        .map_err(|_| AppError::Internal)?
        .insert(sid, tx);
    tracing::info!(session = %sid, %host_id, "session opened");

    tauri::async_runtime::spawn(async move {
        let data = events.clone();
        let result = ssh::run_shell(handle, cols, rows, rx, move |bytes| {
            let _ = data.send(InvokeResponseBody::Raw(bytes));
        })
        .await;
        if let Ok(mut s) = app.state::<AppState>().sessions.lock() {
            s.remove(&sid);
        }
        if let Ok(mut s) = app.state::<AppState>().samples.lock() {
            s.remove(&sid);
        }
        if let Err(e) = &result {
            tracing::error!(session = %sid, error = ?e, "session ended with error");
        }
        tracing::info!(session = %sid, "session closed");
        send_json(
            &events,
            serde_json::json!({"type": "closed", "error": result.err()}),
        );
    });
    Ok(sid)
}

#[tauri::command]
pub fn session_write(state: State<'_, AppState>, id: Uuid, data: String) -> AppResult<()> {
    logged(state.write(id, data))
}

#[tauri::command]
pub fn session_resize(state: State<'_, AppState>, id: Uuid, cols: u32, rows: u32) -> AppResult<()> {
    logged(state.resize(id, cols, rows))
}

#[tauri::command]
pub async fn session_stats(state: State<'_, AppState>, id: Uuid) -> AppResult<Stats> {
    logged(state.stats(id).await)
}

#[tauri::command]
pub fn session_close(state: State<'_, AppState>, id: Uuid) -> AppResult<()> {
    logged(state.send(id, Cmd::Close))
}

#[tauri::command]
pub fn hostkey_decide(state: State<'_, AppState>, prompt_id: Uuid, accept: bool) -> AppResult<()> {
    logged(state.decide(prompt_id, accept))
}

/// Quit for real — the webview only calls this past the live-terminal confirm.
#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}

/// Open a terminal link (ctrl/cmd+click) in the default browser.
/// WHY scheme allowlist: the webview is a trust boundary; never let it open
/// `file://`, custom app schemes, or anything else via the opener plugin.
#[tauri::command]
pub fn url_open(app: AppHandle, url: String) -> AppResult<()> {
    if !(url.starts_with("http://") || url.starts_with("https://") || url.starts_with("mailto:")) {
        return Err(AppError::Invalid("only http, https, and mailto URLs can be opened"));
    }
    use tauri_plugin_opener::OpenerExt;
    app.opener().open_url(url, None::<&str>).map_err(|e| {
        tracing::warn!(error = ?e, "could not open URL");
        AppError::Internal
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::memory::MemoryStore;
    use crate::store::Os;
    use std::sync::Arc;

    struct Shared(Arc<MemoryStore>);
    impl SecretStore for Shared {
        fn set(&self, a: &str, s: &str) -> AppResult<()> {
            self.0.set(a, s)
        }
        fn get(&self, a: &str) -> AppResult<Option<String>> {
            self.0.get(a)
        }
        fn delete(&self, a: &str) -> AppResult<()> {
            self.0.delete(a)
        }
    }

    fn setup() -> (AppState, Arc<MemoryStore>, tempfile::TempDir) {
        let d = tempfile::tempdir().unwrap();
        let store = HostStore::open(d.path().join("hosts.json")).unwrap();
        let mem = Arc::new(MemoryStore::default());
        (
            AppState::new(
                store,
                Box::new(Shared(mem.clone())),
                vec![],
                d.path().join("kh"),
            ),
            mem,
            d,
        )
    }

    fn input() -> HostInput {
        HostInput {
            id: None,
            name: "box".into(),
            host: "10.0.0.2".into(),
            port: 22,
            username: "leo".into(),
            transport: Transport::Direct,
            os: Os::Generic,
            group: String::new(),
            auth: Auth::Password,
        }
    }

    #[test]
    fn secret_goes_to_keychain_never_to_listing() {
        let (s, mem, d) = setup();
        let h = s.save(input()).unwrap();
        s.set_secret(h.id, "hunter2".into()).unwrap();

        assert_eq!(
            mem.get(&host_account(&h.id)).unwrap().as_deref(),
            Some("hunter2")
        );
        assert!(s.list().unwrap()[0].has_secret);
        let listing = serde_json::to_string(&s.list().unwrap()).unwrap();
        assert!(!listing.contains("hunter2"));
        let on_disk = std::fs::read_to_string(d.path().join("hosts.json")).unwrap();
        assert!(!on_disk.contains("hunter2"));
    }

    #[test]
    fn delete_removes_secret_too() {
        let (s, mem, _d) = setup();
        let h = s.save(input()).unwrap();
        s.set_secret(h.id, "pw".into()).unwrap();
        s.delete(h.id).unwrap();
        assert!(s.list().unwrap().is_empty());
        assert!(mem.get(&host_account(&h.id)).unwrap().is_none());
    }

    #[test]
    fn switching_to_key_auth_drops_password() {
        let (s, mem, _d) = setup();
        let h = s.save(input()).unwrap();
        s.set_secret(h.id, "pw".into()).unwrap();
        let mut i = input();
        i.id = Some(h.id);
        i.auth = Auth::Key {
            path: std::env::temp_dir().join("k"),
        };
        let h2 = s.save(i).unwrap();
        assert!(!h2.has_secret);
        assert!(mem.get(&host_account(&h.id)).unwrap().is_none());
    }

    #[test]
    fn clear_secret_is_idempotent_and_unsets_flag() {
        let (s, mem, _d) = setup();
        let h = s.save(input()).unwrap();
        s.set_secret(h.id, "pw".into()).unwrap();
        s.clear_secret(h.id).unwrap();
        assert!(!s.list().unwrap()[0].has_secret);
        assert!(mem.get(&host_account(&h.id)).unwrap().is_none());
        // Deleting twice is fine.
        assert!(s.clear_secret(h.id).is_ok());
    }

    #[test]
    fn secret_bounds_and_unknown_host() {
        let (s, _mem, _d) = setup();
        let h = s.save(input()).unwrap();
        assert!(matches!(
            s.set_secret(h.id, String::new()),
            Err(AppError::Invalid(_))
        ));
        assert!(matches!(
            s.set_secret(h.id, "x".repeat(8193)),
            Err(AppError::Invalid(_))
        ));
        assert!(matches!(
            s.set_secret(Uuid::new_v4(), "pw".into()),
            Err(AppError::NotFound)
        ));
        assert!(matches!(s.delete(Uuid::new_v4()), Err(AppError::NotFound)));
    }

    #[test]
    fn warnings_are_returned_once() {
        let d = tempfile::tempdir().unwrap();
        let store = HostStore::open(d.path().join("h.json")).unwrap();
        let s = AppState::new(
            store,
            Box::new(MemoryStore::default()),
            vec!["w".into()],
            d.path().join("kh"),
        );
        assert_eq!(s.take_warnings().unwrap(), vec!["w".to_string()]);
        assert!(s.take_warnings().unwrap().is_empty());
    }

    #[test]
    fn internal_errors_serialize_generically() {
        let e = AppError::Io(std::io::Error::other("/Users/leo/secret/path denied"));
        let json = serde_json::to_string(&e).unwrap();
        assert!(!json.contains("/Users/leo"));
        assert_eq!(
            serde_json::to_value(&AppError::AuthFailed).unwrap(),
            serde_json::json!({"code": "auth_failed", "message": "authentication failed"})
        );
    }
    #[test]
    fn target_reads_secret_from_keychain() {
        let (s, _mem, _d) = setup();
        let h = s.save(input()).unwrap();
        assert!(
            matches!(s.target_for(h.id), Err(AppError::Invalid(m)) if m.contains("no password"))
        );

        s.set_secret(h.id, "pw".into()).unwrap();
        let t = s.target_for(h.id).unwrap();
        assert!(matches!(t.credential, Credential::Password(ref p) if p == "pw"));
        assert_eq!(
            (t.host.as_str(), t.port, t.username.as_str()),
            ("10.0.0.2", 22, "leo")
        );

        let mut k = input();
        k.id = Some(h.id);
        k.auth = Auth::Key {
            path: std::env::temp_dir().join("k"),
        };
        s.save(k).unwrap();
        let t = s.target_for(h.id).unwrap();
        assert!(matches!(
            t.credential,
            Credential::Key {
                passphrase: None,
                ..
            }
        ));
        assert!(matches!(
            s.target_for(Uuid::new_v4()),
            Err(AppError::NotFound)
        ));
    }

    #[test]
    fn cloudflare_hosts_build_target_like_direct() {
        // Phase 5: CF hosts resolve credentials identically; the tunnel replaces
        // only the TCP stream, and known_hosts keys on the Access domain.
        let (s, _mem, _d) = setup();
        let mut i = input();
        i.transport = Transport::CloudflareAccess;
        let h = s.save(i).unwrap();
        assert!(matches!(
            s.target_for(h.id),
            Err(AppError::Invalid(m)) if m.contains("no password")
        ));
        s.set_secret(h.id, "pw".into()).unwrap();
        let t = s.target_for(h.id).unwrap();
        assert!(matches!(t.credential, Credential::Password(ref p) if p == "pw"));
    }

    #[test]
    fn session_inputs_are_bounded() {
        let (s, _mem, _d) = setup();
        let id = Uuid::new_v4();
        let (tx, mut rx) = mpsc::unbounded_channel();
        s.sessions.lock().unwrap().insert(id, tx);

        assert!(matches!(
            s.write(id, "x".repeat(MAX_WRITE_LEN + 1)),
            Err(AppError::Invalid(_))
        ));
        assert!(matches!(s.resize(id, 0, 24), Err(AppError::Invalid(_))));
        assert!(matches!(s.resize(id, 80, 1001), Err(AppError::Invalid(_))));
        assert!(matches!(
            s.write(Uuid::new_v4(), "x".into()),
            Err(AppError::NotFound)
        ));

        s.write(id, "ls\r".into()).unwrap();
        assert!(matches!(rx.try_recv(), Ok(Cmd::Data(d)) if d == b"ls\r"));
    }

    #[test]
    fn open_session_count_tracks_sessions() {
        let (s, _mem, _d) = setup();
        assert_eq!(s.open_session_count(), 0);
        let id = Uuid::new_v4();
        let (tx, _rx) = mpsc::unbounded_channel();
        s.sessions.lock().unwrap().insert(id, tx);
        assert_eq!(s.open_session_count(), 1);
    }

    #[test]
    fn hostkey_decision_is_delivered_once() {
        let (s, _mem, _d) = setup();
        let pid = Uuid::new_v4();
        let (tx, mut rx) = oneshot::channel();
        s.prompts.lock().unwrap().insert(pid, tx);
        s.decide(pid, true).unwrap();
        assert_eq!(rx.try_recv(), Ok(true));
        assert!(matches!(s.decide(pid, true), Err(AppError::NotFound)));
    }
}
