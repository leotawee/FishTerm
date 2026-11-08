use crate::error::AppResult;

/// OS-keychain-backed secret storage. See ADR-001.
///
/// `get` exists for the Rust core only (SSH auth). No Tauri command exposes it.
pub trait SecretStore: Send + Sync {
    fn set(&self, account: &str, secret: &str) -> AppResult<()>;
    #[cfg_attr(not(test), expect(dead_code, reason = "read by SSH auth in Phase 3"))]
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

impl KeyringStore {
    pub fn new(service: &'static str) -> Self {
        Self { service }
    }
}

impl SecretStore for KeyringStore {
    fn set(&self, account: &str, secret: &str) -> AppResult<()> {
        keyring::Entry::new(self.service, account)?.set_password(secret)?;
        Ok(())
    }

    fn get(&self, account: &str) -> AppResult<Option<String>> {
        match keyring::Entry::new(self.service, account)?.get_password() {
            Ok(s) => Ok(Some(s)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn delete(&self, account: &str) -> AppResult<()> {
        match keyring::Entry::new(self.service, account)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.into()),
        }
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
