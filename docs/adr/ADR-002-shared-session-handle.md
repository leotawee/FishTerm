# ADR-002: Share the SSH session handle for SFTP (and future channels)

**Status:** Accepted
**Date:** 2026-11-14

## Context

ADR-001 gave each session a dedicated `run_shell` task that owned the only
`russh::client::Handle` and an mpsc `Cmd` channel. That design could not open
new SSH channels (SFTP subsystem, forwarded TCP) from outside the shell task:
`Handle` is not `Clone` — it owns the reply receiver — and the shell task never
exposes it.

The SFTP spec (`docs/spec/sftp.md`) needs a subsystem channel on the *same*
authenticated connection as the terminal. Alternatives:

1. Open a second SSH connection per SFTP panel — re-does auth, host-key
   checking and keychain reads, doubles server load, and can desync from the
   terminal session.
2. Route SFTP through the shell's `Cmd` enum with oneshot replies — serializes
   every SFTP byte through the PTY task and complicates the command type.
3. Share the handle as `Arc<russh::client::Handle>`; every channel user opens
   its own channel from its own task.

## Decision

Option 3. `run_shell` takes `Arc<Handle<Client>>`; `AppState` keeps a
`handles: HashMap<Uuid, Arc<Handle>>` alongside the existing command-sender
map. SFTP (`src-tauri/src/sftp.rs`) clones the handle per call and opens its
own subsystem channel. The shell loop still owns the connection pump; nothing
else writes to the PTY channel.

## Consequences

- SFTP lives and dies with the terminal session, like `ssh -L` — closing the
  tab ends the SFTP channel. That is the intended model.
- `Cmd::Exec` (monitor polls) is unchanged in behaviour but now documents that
  exec runs on a *separate channel* of the shared handle — still compliant
  with ADR-001's "one exec channel per poll, no shared interactive shell".
- Parallel per-session maps (senders / handles / samples) must be inserted and
  removed in lockstep (`commands.rs` TECH DEBT note); merge into one
  `HashMap<Uuid, Session>` struct when a fourth field appears.
- Future channel users (port forwarding) reuse the same pattern without
  further architecture change.
