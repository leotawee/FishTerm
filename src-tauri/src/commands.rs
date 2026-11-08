//! Tauri IPC surface. Thin wrappers over `AppState` methods so the logic is testable
//! without a Tauri runtime. No command returns a secret (threat model §7).

use crate::error::{AppError, AppResult};
use crate::secrets::{SecretStore, host_account};
use crate::store::{Host, HostInput, HostStore};
use std::sync::{Mutex, MutexGuard};
use tauri::State;
use uuid::Uuid;

const MAX_SECRET_LEN: usize = 8192;

pub struct AppState {
    store: Mutex<HostStore>,
    secrets: Box<dyn SecretStore>,
    warnings: Mutex<Vec<String>>,
}

impl AppState {
    pub fn new(store: HostStore, secrets: Box<dyn SecretStore>, warnings: Vec<String>) -> Self {
        Self {
            store: Mutex::new(store),
            secrets,
            warnings: Mutex::new(warnings),
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::memory::MemoryStore;
    use crate::store::{Auth, Transport};
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
            AppState::new(store, Box::new(Shared(mem.clone())), vec![]),
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
        let s = AppState::new(store, Box::new(MemoryStore::default()), vec!["w".into()]);
        assert_eq!(s.take_warnings().unwrap(), vec!["w".to_string()]);
        assert!(s.take_warnings().unwrap().is_empty());
    }

    #[test]
    fn internal_errors_serialize_generically() {
        let e = AppError::Io(std::io::Error::other("/Users/leo/secret/path denied"));
        let json = serde_json::to_string(&e).unwrap();
        assert!(!json.contains("/Users/leo"));
    }
}
