use crate::error::{AppError, AppResult};

/// OS-keychain-backed secret storage. See ADR-001.
///
/// `get` exists for the Rust core only (SSH auth). No Tauri command exposes it.
pub trait SecretStore: Send + Sync {
    fn set(&self, account: &str, secret: &str) -> AppResult<()>;
    fn get(&self, account: &str) -> AppResult<Option<String>>;
    /// Idempotent: deleting a missing secret is not an error.
    fn delete(&self, account: &str) -> AppResult<()>;
}

pub fn host_account(id: &uuid::Uuid) -> String {
    format!("host:{id}")
}

pub struct KeyringStore {
    service: &'static str,
}

/// All secrets live in ONE keychain item so macOS "Always Allow" grants the binary
/// access to every host/token at once — per-item storage would prompt once per host
/// per rebuild (ad-hoc signing makes each build a "new app" to the keychain).
/// Keys are the legacy account strings (`host:<uuid>`, `cf-access:<domain>:<kid>`),
/// so nothing outside this file changes.
const BLOB_ACCOUNT: &str = "secrets";

impl KeyringStore {
    pub fn new(service: &'static str) -> Self {
        Self { service }
    }

    fn entry(&self, account: &str) -> AppResult<keyring::Entry> {
        Ok(keyring::Entry::new(self.service, account)?)
    }

    fn read_blob(&self) -> AppResult<serde_json::Map<String, serde_json::Value>> {
        match self.entry(BLOB_ACCOUNT)?.get_password() {
            Ok(s) => Ok(serde_json::from_str(&s).map_err(|e| {
                tracing::error!(error = ?e, "keychain secrets blob unreadable");
                AppError::Internal
            })?),
            Err(keyring::Error::NoEntry) => Ok(serde_json::Map::new()),
            Err(e) => Err(e.into()),
        }
    }

    fn write_blob(&self, map: &serde_json::Map<String, serde_json::Value>) -> AppResult<()> {
        self.entry(BLOB_ACCOUNT)?
            .set_password(&serde_json::to_string(map)?)
            .map_err(|e| {
                tracing::error!(error = ?e, "keychain secrets blob write failed");
                AppError::Keyring(e)
            })?;
        Ok(())
    }

    // shortcut: legacy per-host items are absorbed on first read and deleted as
    // hosts are re-saved; a stale legacy item simply lingers harmlessly otherwise.
    fn delete_legacy(&self, account: &str) {
        if let Ok(entry) = self.entry(account) {
            if let Err(e) = entry.delete_credential() {
                tracing::warn!(error = ?e, account, "legacy keychain item cleanup failed");
            }
        }
    }
}

impl SecretStore for KeyringStore {
    fn set(&self, account: &str, secret: &str) -> AppResult<()> {
        let mut map = self.read_blob()?;
        map.insert(account.into(), serde_json::Value::String(secret.into()));
        self.write_blob(&map)?;
        self.delete_legacy(account);
        Ok(())
    }

    fn get(&self, account: &str) -> AppResult<Option<String>> {
        let map = self.read_blob()?;
        if let Some(secret) = map.get(account).and_then(|v| v.as_str()) {
            return Ok(Some(secret.to_owned()));
        }
        // First run on the blob storage: absorb the legacy per-host item so its
        // ACL prompt is answered exactly once and never needed again.
        match self.entry(account)?.get_password() {
            Ok(secret) => {
                self.set(account, &secret)?;
                Ok(Some(secret))
            }
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn delete(&self, account: &str) -> AppResult<()> {
        let mut map = self.read_blob()?;
        map.remove(account);
        self.write_blob(&map)?;
        self.delete_legacy(account);
        Ok(())
    }
}

#[cfg(test)]
pub mod memory {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    #[derive(Default)]
    pub struct MemoryStore(pub Mutex<HashMap<String, String>>);

    impl SecretStore for MemoryStore {
        fn set(&self, account: &str, secret: &str) -> AppResult<()> {
            self.0.lock().unwrap().insert(account.into(), secret.into());
            Ok(())
        }
        fn get(&self, account: &str) -> AppResult<Option<String>> {
            Ok(self.0.lock().unwrap().get(account).cloned())
        }
        fn delete(&self, account: &str) -> AppResult<()> {
            self.0.lock().unwrap().remove(account);
            Ok(())
        }
    }
}
