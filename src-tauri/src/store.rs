//! Saved-host inventory (`hosts.json`). Holds no secrets: see `secrets.rs` and ADR-001.

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Transport {
    Direct,
    CloudflareAccess,
}

/// Display hint for the home-screen card; never used by the SSH layer.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub enum Os {
    #[default]
    Generic,
    Debian,
    Ubuntu,
    RaspberryPiOs,
    Redhat,
    Centos,
    Fedora,
    Opensuse,
    Suse,
    ArchLinux,
    Amazon,
    Freebsd,
    Apple,
    Windows,
    Linux,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Auth {
    Password,
    /// Private key file. The keychain secret, if any, is the key's passphrase.
    Key {
        path: PathBuf,
    },
}

/// Stored on disk and sent to the webview as-is.
/// WHY: one type with no secret field means a secret cannot leak through it by accident.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct Host {
    pub id: Uuid,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub transport: Transport,
    /// Display hint for the home-screen card; never used by the SSH layer.
    #[serde(default)]
    pub os: Os,
    pub auth: Auth,
    /// A password/passphrase is stored in the keychain for this host.
    pub has_secret: bool,
}

/// What the webview may send. Unknown fields (e.g. a stray `password`) are rejected.
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct HostInput {
    pub id: Option<Uuid>,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub transport: Transport,
    #[serde(default)]
    pub os: Os,
    pub auth: Auth,
}

#[derive(Serialize, Deserialize)]
struct FileFormat {
    version: u32,
    hosts: Vec<Host>,
}

const FILE_VERSION: u32 = 1;

fn has_control(s: &str) -> bool {
    s.chars().any(char::is_control)
}

impl HostInput {
    pub fn validate(&self) -> AppResult<()> {
        let name = self.name.trim();
        if name.is_empty() || name.chars().count() > 64 || has_control(name) {
            return Err(AppError::Invalid("name must be 1-64 printable characters"));
        }
        // Hostname, IPv4 or bare IPv6. No scheme, path, userinfo or whitespace.
        let host_ok = !self.host.is_empty()
            && self.host.len() <= 253
            && self
                .host
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':' | '_'));
        if !host_ok {
            return Err(AppError::Invalid("host must be a hostname or IP address"));
        }
        if self.port == 0 {
            return Err(AppError::Invalid("port must be 1-65535"));
        }
        if self.username.is_empty()
            || self.username.len() > 64
            || has_control(&self.username)
            || self.username.chars().any(char::is_whitespace)
        {
            return Err(AppError::Invalid(
                "username must be 1-64 characters without spaces",
            ));
        }
        if let Auth::Key { path } = &self.auth {
            // Existence is checked at connect time; the file may live on a removable drive.
            if !path.is_absolute() || path.as_os_str().len() > 4096 {
                return Err(AppError::Invalid("key path must be an absolute path"));
            }
        }
        Ok(())
    }
}

pub struct HostStore {
    path: PathBuf,
    hosts: Vec<Host>,
}

/// Result of `upsert`: the saved host, and whether its old secret no longer fits the auth kind.
pub struct Saved {
    pub host: Host,
    pub secret_invalidated: bool,
}

impl HostStore {
    /// Load `path`. A missing file is an empty list. An unparseable file is moved aside
    /// (never overwritten) and reported as `CorruptConfig`.
    pub fn open(path: PathBuf) -> AppResult<Self> {
        let raw = match fs::read(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self {
                    path,
                    hosts: Vec::new(),
                });
            }
            Err(e) => return Err(e.into()),
        };
        match serde_json::from_slice::<FileFormat>(&raw) {
            Ok(f) if f.version == FILE_VERSION => Ok(Self {
                path,
                hosts: f.hosts,
            }),
            Ok(_) | Err(_) => {
                let ts = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                let backup = path.with_extension(format!("json.corrupt-{ts}"));
                fs::rename(&path, &backup)?;
                tracing::warn!(backup = %backup.display(), "hosts file unreadable, moved aside");
                Err(AppError::CorruptConfig(backup.display().to_string()))
            }
        }
    }

    /// `open`, but a corrupt file yields an empty store plus a warning for the UI.
    pub fn open_or_recover(path: PathBuf) -> AppResult<(Self, Vec<String>)> {
        match Self::open(path.clone()) {
            Ok(s) => Ok((s, Vec::new())),
            Err(e @ AppError::CorruptConfig(_)) => Ok((
                Self {
                    path,
                    hosts: Vec::new(),
                },
                vec![e.to_string()],
            )),
            Err(e) => Err(e),
        }
    }

    pub fn list(&self) -> &[Host] {
        &self.hosts
    }

    pub fn get(&self, id: &Uuid) -> Option<&Host> {
        self.hosts.iter().find(|h| h.id == *id)
    }

    pub fn upsert(&mut self, input: HostInput) -> AppResult<Saved> {
        input.validate()?;
        let mut next = self.hosts.clone();
        let (host, secret_invalidated) = match input.id {
            Some(id) => {
                let slot = next
                    .iter_mut()
                    .find(|h| h.id == id)
                    .ok_or(AppError::NotFound)?;
                // A password is not a key passphrase: drop the secret when the auth kind changes.
                let invalidated = slot.has_secret
                    && std::mem::discriminant(&slot.auth) != std::mem::discriminant(&input.auth);
                *slot = Host {
                    id,
                    name: input.name.trim().to_owned(),
                    host: input.host,
                    port: input.port,
                    username: input.username,
                    transport: input.transport,
                    os: input.os,
                    auth: input.auth,
                    has_secret: slot.has_secret && !invalidated,
                };
                (slot.clone(), invalidated)
            }
            None => {
                let h = Host {
                    id: Uuid::new_v4(),
                    name: input.name.trim().to_owned(),
                    host: input.host,
                    port: input.port,
                    username: input.username,
                    transport: input.transport,
                    os: input.os,
                    auth: input.auth,
                    has_secret: false,
                };
                next.push(h.clone());
                (h, false)
            }
        };
        self.commit(next)?;
        Ok(Saved {
            host,
            secret_invalidated,
        })
    }

    pub fn delete(&mut self, id: &Uuid) -> AppResult<()> {
        let next: Vec<Host> = self.hosts.iter().filter(|h| h.id != *id).cloned().collect();
        if next.len() == self.hosts.len() {
            return Err(AppError::NotFound);
        }
        self.commit(next)
    }

    pub fn set_has_secret(&mut self, id: &Uuid, value: bool) -> AppResult<()> {
        let mut next = self.hosts.clone();
        next.iter_mut()
            .find(|h| h.id == *id)
            .ok_or(AppError::NotFound)?
            .has_secret = value;
        self.commit(next)
    }

    /// Write first, then swap memory: memory never holds state the disk does not.
    fn commit(&mut self, next: Vec<Host>) -> AppResult<()> {
        let body = serde_json::to_vec_pretty(&FileFormat {
            version: FILE_VERSION,
            hosts: next.clone(),
        })?;
        write_atomic(&self.path, &body)?;
        self.hosts = next;
        Ok(())
    }
}

/// tmp file (0600, same dir) + fsync + rename. A crash leaves either the old or the new file.
fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let dir = path.parent().ok_or(std::io::ErrorKind::InvalidInput)?;
    create_private_dir(dir)?;
    let file_name = path
        .file_name()
        .ok_or(std::io::ErrorKind::InvalidInput)?
        .to_string_lossy();
    let tmp = dir.join(format!(".{file_name}.tmp-{}", Uuid::new_v4()));

    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600);

    let result = (|| {
        let mut f = opts.open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result?;
    #[cfg(unix)]
    {
        // Persist the rename itself. Best effort: some filesystems refuse dir fsync.
        if let Ok(d) = fs::File::open(dir) {
            let _ = d.sync_all();
        }
    }
    Ok(())
}

pub(crate) fn create_private_dir(dir: &Path) -> std::io::Result<()> {
    if dir.is_dir() {
        return Ok(());
    }
    let mut b = fs::DirBuilder::new();
    b.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut b, 0o700);
    // shortcut: on Windows the dir inherits the user-profile ACL (AppData), which is private by default.
    b.create(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(name: &str) -> HostInput {
        HostInput {
            id: None,
            name: name.into(),
            host: "example.com".into(),
            port: 22,
            username: "leo".into(),
            transport: Transport::Direct,
            os: Os::Generic,
            auth: Auth::Password,
        }
    }

    fn store_in(dir: &tempfile::TempDir) -> HostStore {
        HostStore::open(dir.path().join("cfg/hosts.json")).unwrap()
    }

    #[test]
    fn missing_file_is_empty_store() {
        let d = tempfile::tempdir().unwrap();
        assert!(store_in(&d).list().is_empty());
    }

    #[test]
    fn save_round_trips_through_disk() {
        let d = tempfile::tempdir().unwrap();
        let mut s = store_in(&d);
        let saved = s.upsert(input("box")).unwrap().host;
        let reopened = store_in(&d);
        assert_eq!(reopened.list(), &[saved]);
    }

    #[cfg(unix)]
    #[test]
    fn file_and_dir_are_private() {
        use std::os::unix::fs::PermissionsExt;
        let d = tempfile::tempdir().unwrap();
        store_in(&d).upsert(input("box")).unwrap();
        let file = fs::metadata(d.path().join("cfg/hosts.json"))
            .unwrap()
            .permissions()
            .mode();
        let dir = fs::metadata(d.path().join("cfg"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(file & 0o777, 0o600);
        assert_eq!(dir & 0o777, 0o700);
    }

    #[test]
    fn corrupt_file_is_moved_aside_not_overwritten() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("hosts.json");
        fs::write(&path, b"{not json").unwrap();

        let (mut s, warnings) = HostStore::open_or_recover(path.clone()).unwrap();
        assert_eq!(warnings.len(), 1);
        assert!(s.list().is_empty());

        let backups: Vec<_> = fs::read_dir(d.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".corrupt-"))
            .collect();
        assert_eq!(backups.len(), 1);
        assert_eq!(fs::read(backups[0].path()).unwrap(), b"{not json");

        // Saving afterwards must not touch the backup.
        s.upsert(input("new")).unwrap();
        assert_eq!(fs::read(backups[0].path()).unwrap(), b"{not json");
    }

    #[test]
    fn unknown_version_is_treated_as_corrupt() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("hosts.json");
        fs::write(&path, br#"{"version":99,"hosts":[]}"#).unwrap();
        assert!(matches!(
            HostStore::open(path),
            Err(AppError::CorruptConfig(_))
        ));
    }

    #[test]
    fn no_tmp_files_left_behind() {
        let d = tempfile::tempdir().unwrap();
        let mut s = store_in(&d);
        s.upsert(input("a")).unwrap();
        s.upsert(input("b")).unwrap();
        let names: Vec<String> = fs::read_dir(d.path().join("cfg"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["hosts.json".to_string()]);
    }

    #[test]
    fn update_unknown_id_is_not_found() {
        let d = tempfile::tempdir().unwrap();
        let mut s = store_in(&d);
        let mut i = input("x");
        i.id = Some(Uuid::new_v4());
        assert!(matches!(s.upsert(i), Err(AppError::NotFound)));
        assert!(s.list().is_empty());
    }

    #[test]
    fn auth_kind_change_invalidates_secret() {
        let d = tempfile::tempdir().unwrap();
        let mut s = store_in(&d);
        let h = s.upsert(input("x")).unwrap().host;
        s.set_has_secret(&h.id, true).unwrap();

        let mut same = input("renamed");
        same.id = Some(h.id);
        let r = s.upsert(same).unwrap();
        assert!(!r.secret_invalidated && r.host.has_secret);

        let mut key = input("renamed");
        key.id = Some(h.id);
        key.auth = Auth::Key {
            path: std::env::temp_dir().join("id_ed25519"),
        };
        let r = s.upsert(key).unwrap();
        assert!(r.secret_invalidated && !r.host.has_secret);
    }

    #[test]
    fn validation_rejects_bad_input() {
        type Mutator = fn(&mut HostInput);
        let cases: Vec<(&str, Mutator)> = vec![
            ("empty name", |i| i.name = "  ".into()),
            ("long name", |i| i.name = "a".repeat(65)),
            ("control in name", |i| i.name = "a\u{7}".into()),
            ("scheme in host", |i| i.host = "ssh://x".into()),
            ("space in host", |i| i.host = "a b".into()),
            ("userinfo in host", |i| i.host = "u@x".into()),
            ("long host", |i| i.host = "a".repeat(254)),
            ("port zero", |i| i.port = 0),
            ("space in user", |i| i.username = "a b".into()),
            ("relative key", |i| {
                i.auth = Auth::Key {
                    path: "id_rsa".into(),
                }
            }),
        ];
        for (label, mutate) in cases {
            let mut i = input("ok");
            mutate(&mut i);
            assert!(
                matches!(i.validate(), Err(AppError::Invalid(_))),
                "{label} should be rejected"
            );
        }
        assert!(input("ok").validate().is_ok());
    }

    #[test]
    fn input_with_stray_secret_field_is_rejected() {
        let json = r#"{"id":null,"name":"x","host":"h","port":22,"username":"u",
            "transport":"direct","auth":{"kind":"password"},"password":"hunter2"}"#;
        assert!(serde_json::from_str::<HostInput>(json).is_err());
    }

    #[test]
    fn host_json_has_no_secret_field() {
        let d = tempfile::tempdir().unwrap();
        let h = store_in(&d).upsert(input("x")).unwrap().host;
        let v = serde_json::to_value(&h).unwrap();
        let mut keys: Vec<_> = v.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        assert_eq!(
            keys,
            [
                "auth",
                "has_secret",
                "host",
                "id",
                "name",
                "os",
                "port",
                "transport",
                "username"
            ]
        );
    }
}
