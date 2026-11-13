//! SFTP file transfer over the session's shared SSH handle (threat model §7:
//! no secret ever passes through here; the connection is already authenticated).

use crate::commands::AppState;
use crate::error::{AppError, AppResult};
use russh_sftp::client::SftpSession;
use serde::Serialize;
use tauri::ipc::Channel;
use tauri::State;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Transfer chunk / progress-report granularity.
const TRANSFER_CHUNK: u64 = 64 * 1024;
/// Hard cap per transfer; keeps a runaway path from filling the disk silently.
const MAX_TRANSFER: u64 = 8 * 1024 * 1024 * 1024; // 8 GiB
const MAX_PATH_LEN: usize = 4096;

#[derive(Serialize)]
pub struct SftpEntry {
    name: String,
    /// Full remote path, ready to pass back as a new `cwd`.
    path: String,
    is_dir: bool,
    size: u64,
    /// Unix mtime in seconds; `None` when the server omits it.
    modified: Option<i64>,
}

/// Dirs first, then names case-insensitively. Pure fn so it is unit-testable.
fn sort_entries(entries: &mut [SftpEntry]) {
    entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
}

/// Open an SFTP subsystem channel on the session's shared connection.
async fn sftp_for(state: &AppState, id: uuid::Uuid) -> AppResult<SftpSession> {
    let handle = state
        .handle_for(id)
        .ok_or(AppError::NotFound)?
        .clone();
    let channel = handle.channel_open_session().await.map_err(|_| AppError::Ssh)?;
    channel
        .request_subsystem(true, "sftp")
        .await
        .map_err(|_| AppError::Ssh)?;
    SftpSession::new(channel.into_stream())
        .await
        .map_err(|e| {
            tracing::warn!(error = ?e, "sftp subsystem init failed (server may not support it)");
            AppError::Ssh
        })
}

fn check_remote_path(path: &str) -> AppResult<()> {
    if path.is_empty() || path.len() > MAX_PATH_LEN {
        return Err(AppError::Invalid("remote path must be 1-4096 characters"));
    }
    Ok(())
}

fn check_local_path(path: &str) -> AppResult<()> {
    // Trust boundary: only paths produced by the native file dialogs are accepted.
    if !std::path::Path::new(path).is_absolute() || path.len() > MAX_PATH_LEN {
        return Err(AppError::Invalid("local path must be absolute and at most 4096 characters"));
    }
    Ok(())
}

#[derive(Serialize)]
pub struct SftpListing {
    /// Server-canonical directory the entries belong to. The webview must use
    /// this, never a client-side guess — an empty dir otherwise derives "/".
    cwd: String,
    entries: Vec<SftpEntry>,
}

#[tauri::command]
pub async fn sftp_ls(
    state: State<'_, AppState>,
    id: uuid::Uuid,
    path: String,
) -> AppResult<SftpListing> {
    let sftp = sftp_for(&state, id).await?;
    // Empty path = start at the user's home directory.
    let dir = if path.is_empty() {
        sftp.canonicalize(".").await.map_err(|_| AppError::Ssh)?
    } else {
        check_remote_path(&path)?;
        path
    };
    let mut entries: Vec<SftpEntry> = sftp
        .read_dir(&dir)
        .await
        .map_err(|_| AppError::Ssh)?
        .map(|e| {
            let meta = e.metadata();
            SftpEntry {
                name: e.file_name(),
                path: e.path(),
                is_dir: meta.is_dir(),
                size: meta.len(),
                modified: meta.modified().ok().map(|t| {
                    t.duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs() as i64)
                        .unwrap_or(0)
                }),
            }
        })
        .collect();
    sort_entries(&mut entries);
    Ok(SftpListing { cwd: dir, entries })
}

/// Progress payload sent to the webview during a transfer.
#[derive(Clone, Serialize)]
pub struct SftpProgress {
    pub bytes: u64,
    pub total: u64,
}

async fn copy_with_progress<R: tokio::io::AsyncRead + Unpin, W: tokio::io::AsyncWrite + Unpin>(
    mut reader: R,
    mut writer: W,
    total: u64,
    progress: Channel<SftpProgress>,
) -> AppResult<u64> {
    let mut buf = vec![0u8; TRANSFER_CHUNK as usize];
    let mut done: u64 = 0;
    loop {
        let n = reader.read(&mut buf).await.map_err(AppError::Io)?;
        if n == 0 {
            break;
        }
        done += n as u64;
        if done > MAX_TRANSFER {
            return Err(AppError::Invalid("transfer exceeds the 8 GiB limit"));
        }
        writer
            .write_all(&buf[..n])
            .await
            .map_err(AppError::Io)?;
        let _ = progress.send(SftpProgress { bytes: done, total });
    }
    writer.flush().await.map_err(AppError::Io)?;
    Ok(done)
}

#[tauri::command]
pub async fn sftp_download(
    state: State<'_, AppState>,
    id: uuid::Uuid,
    remote_path: String,
    local_path: String,
    progress: Channel<SftpProgress>,
) -> AppResult<u64> {
    check_remote_path(&remote_path)?;
    check_local_path(&local_path)?;
    let sftp = sftp_for(&state, id).await?;
    let total = sftp
        .metadata(&remote_path)
        .await
        .map_err(|_| AppError::Ssh)?
        .len();
    let mut remote = sftp.open(&remote_path).await.map_err(|_| AppError::Ssh)?;
    let local = tokio::fs::File::create(&local_path).await.map_err(AppError::Io)?;
    let n = copy_with_progress(&mut remote, local, total, progress).await?;
    tracing::info!(bytes = n, "sftp download finished");
    Ok(n)
}

#[tauri::command]
pub async fn sftp_upload(
    state: State<'_, AppState>,
    id: uuid::Uuid,
    local_path: String,
    remote_path: String,
    progress: Channel<SftpProgress>,
) -> AppResult<u64> {
    check_local_path(&local_path)?;
    check_remote_path(&remote_path)?;
    let sftp = sftp_for(&state, id).await?;
    let local = tokio::fs::File::open(&local_path).await.map_err(AppError::Io)?;
    let total = local.metadata().await.map_err(AppError::Io)?.len();
    // create=true, truncate=true, write=true — overwrite an existing remote file.
    let remote = sftp
        .open_with_flags(
            &remote_path,
            russh_sftp::protocol::OpenFlags::CREATE
                | russh_sftp::protocol::OpenFlags::TRUNCATE
                | russh_sftp::protocol::OpenFlags::WRITE,
        )
        .await
        .map_err(|_| AppError::Ssh)?;
    let n = copy_with_progress(local, remote, total, progress).await?;
    tracing::info!(bytes = n, "sftp upload finished");
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, is_dir: bool) -> SftpEntry {
        SftpEntry {
            name: name.into(),
            path: format!("/x/{name}"),
            is_dir,
            size: 0,
            modified: None,
        }
    }

    #[test]
    fn dirs_sort_first_then_case_insensitive_names() {
        let mut v = vec![
            entry("zebra", false),
            entry("sub", true),
            entry("Apple", false),
            entry("apple2", false),
        ];
        sort_entries(&mut v);
        let names: Vec<_> = v.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["sub", "Apple", "apple2", "zebra"]);
    }

    #[test]
    fn local_paths_must_be_absolute() {
        assert!(check_local_path("/tmp/a.bin").is_ok());
        assert!(check_local_path("relative/a.bin").is_err());
        assert!(check_local_path("").is_err());
    }
}
