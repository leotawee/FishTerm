# ADR-001: Rewrite FishTerm on Tauri v2 + Rust

**Status:** Proposed
**Date:** 2026-11-08

## Context

Upstream FishTerm (`79fa6a2`) is an Electron 22 + Vue 3 SSH client with Cloudflare Access tunnel support.
The review (`docs/agent-log/2026-11-08-fishterm-rust-rewrite.md`) found:

- The SSH host key is never verified (`verify(true)`): a MITM can capture the password.
- Passwords are stored hex-encoded in `~/.fishssh/ssh.json`, and plaintext passwords are sent to the renderer.
- Electron 22 is EOL, `sandbox: false`, and the preload exposes a raw `shell.openExternal`.
- `ssh2-streams` is deprecated. The monitor and config layers have data-loss and parsing bugs. There are no tests.

Leo chose: no Electron, Rust core, Tauri v2 UI, on branch `tauri-rust` in the fork.

Constraints:
- Must keep the features: saved hosts, password or key auth, SSH over Cloudflare Access (WebSocket), live CPU/RAM/disk/net monitor, multi-tab terminal.
- Single-user desktop app on macOS, Linux and Windows. There is no server component.
- MIT attribution to upstream must stay.

## Decision

We will rebuild FishTerm, keeping its name. Tauri v2 hosts the UI and a Rust core owns every secret and every socket.

- **SSH:** `russh` (pure Rust, maintained). Strict host-key checking against `~/.ssh/known_hosts`. Unknown keys need explicit user approval of the SHA256 fingerprint (TOFU). A changed key fails hard.
- **Secrets:** OS keychain through `keyring` (macOS Keychain, Windows Credential Manager, Secret Service). `hosts.json` holds no secret, only a keychain reference.
- **Write-only secrets at the IPC boundary:** the webview can set or replace a password but never read it back.
- **Cloudflare Access:** port the `cloudflared` token-transfer flow to Rust (`reqwest` + `crypto_box`, NaCl-compatible). Store the app token in the keychain. Rust validates the login URL (https only, host must match the configured app domain) and opens it itself. The webview never gets an "open URL" primitive.
- **Monitor:** one `exec` channel per poll (no shared interactive shell). Per-session state. Linux targets only; elsewhere the monitor reports "unsupported".
- **UI:** reuse the Vue 3 screens. `@xterm/xterm` + fit addon. Strict CSP. Tauri capabilities allow only the app's own commands.
- **Tooling:** Bun for the frontend (house default), Cargo for the core. A single lockfile per side (`bun.lock`, `Cargo.lock`).

## Consequences

### Positive

- Closes every P0/P1 at the architecture level, not one patch at a time. Secrets never enter the JS heap after save.
- Binary is roughly 10–15 MB vs about 150 MB. No bundled Chromium to keep patched; the OS webview gets OS updates.
- Rust types at the IPC boundary replace `any` + `JSON.parse`.
- `known_hosts` is shared with OpenSSH, so hosts trusted in one client are trusted in the other.

### Negative / Trade-offs

- Full rewrite of the main process: about 1.2k lines of TS become about 1.5–2k lines of Rust. The Vue UI is ported, not reused as-is.
- System webviews differ (WebKit on macOS/Linux, WebView2 on Windows), so xterm.js rendering needs testing on each.
- The Cloudflare token-transfer protocol is undocumented. It is ported from upstream behaviour and the `cloudflared` source, and may break if Cloudflare changes it.
- On Linux, `keyring` needs a Secret Service (gnome-keyring/KWallet). Headless Linux has no fallback (see threat model, residual risks).
- Upstream fixes can no longer be merged; this branch is a fork in spirit.

## Alternatives Considered

| Option | Why rejected |
| ------ | ------------ |
| Harden the Electron fork | Leo ruled out Electron. Also keeps a 150 MB Chromium to patch forever. |
| Pure native Rust GUI (egui + alacritty_terminal) | Terminal rendering and input from scratch costs weeks; the Vue UI is thrown away. |
| ratatui TUI | No GUI window, which drops the product's main form. |
| `ssh2` crate (libssh2 bindings) | C dependency, harder cross-compile, weaker async story than `russh`. |
| Encrypted local file + master password | Adds a custom KDF/UX. The OS keychain is audited and already unlocked with the user session. |
