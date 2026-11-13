# Spec: SFTP file transfer

Approved scope from the scrutinize review (2026-11-14): SFTP only. X11 rejected
(webview cannot render X11 apps; no X server on macOS). Port forwarding deferred.

## Goal

Browse the remote filesystem of a connected SSH host and transfer files
(download from remote, upload to remote) with progress feedback.

## Design

- **Transport:** `russh-sftp` client over a session subsystem channel on the
  same SSH connection the terminal uses. SFTP lives and dies with the session,
  like `ssh -L` does — no separate connection, no extra credentials handling.
- **Prerequisite:** the SSH `Handle` must be shareable. It is restructured into
  `Arc<Handle>` (methods take `&self`), so the SFTP task can open channels
  while the shell loop owns the connection pump. (T1)
- **Backend:** `src-tauri/src/sftp.rs` — normalize/resolve the remote home dir
  once, then `list`, `download`, `upload`. Progress is streamed to the webview
  via a Tauri `Channel` (same mechanism as session events). (T2)
- **UI:** a "Files" button in the terminal footer opens an SFTP dialog:
  directory listing (name/size/mtime, directories first), navigate by click +
  breadcrumb path, download (native save dialog → transfer), upload (native
  open dialog → transfer). Progress bar per active transfer. (T3)

## Security notes

- Remote paths come from the user's own server; they are never checked against
  the local filesystem — only local paths from `plugin-dialog` are used as
  destinations/sources.
- Download/upload size and path length bounded; transfers are sequential per
  session to keep channel windowing predictable.
- No secrets touch the SFTP layer; it reuses the already-authenticated handle.

## Tickets

| Ticket | Depends on | Scope |
| --- | --- | --- |
| T1 `sftp-t1-arc-handle` | — | `Arc<Handle>` session restructure, behavior unchanged |
| T2 `sftp-t2-backend` | T1 | `sftp.rs` + IPC commands `sftp_ls/sftp_download/sftp_upload`, unit tests for pure logic |
| T3 `sftp-t3-ui` | T2 | Files dialog in `SSH.vue`, listing + transfer with progress |

## Out of scope

Delete/rename/mkdir remote operations, drag-and-drop, concurrent transfers,
recursive directory transfer, remote-forward local browsing.
