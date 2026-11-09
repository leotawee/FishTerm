## Threat Model: FishTerm (Tauri v2 + Rust SSH client)

See ADR-001.

### 1. Assets

- SSH passwords and private-key passphrases
- Private key files (referenced by path, read by the Rust core only)
- Cloudflare Access app tokens (JWT, grants tunnel access)
- Host trust state (`~/.ssh/known_hosts`)
- Saved host inventory (`hosts.json`: names, hosts, usernames; no secrets)
- Interactive shell session (keystrokes = full remote control)

### 2. Trust Boundaries

- Webview (JS) → Rust core (Tauri IPC commands)
- Rust core → network (TCP to sshd, HTTPS to Cloudflare Access, WSS tunnel)
- Rust core → OS keychain
- Rust core → filesystem (config dir, known_hosts, key files)
- Remote server → terminal output rendered by xterm.js

### 3. Actors

| Actor | Trust Level | Notes |
| ----- | ----------- | ----- |
| Local user | Trusted | Owns the machine and keychain |
| Network attacker (MITM) | Untrusted | Same Wi‑Fi/LAN, rogue DNS |
| Malicious/compromised SSH server | Untrusted | Controls all output and auth prompts |
| Malicious Cloudflare-fronted host | Untrusted | Controls redirects and `cf-access-domain` header |
| Other local OS user | Untrusted | Can read world-readable files |
| Compromised webview (XSS via terminal/UI) | Low trust | Must not reach secrets or arbitrary FS/URL |

### 4. Attack Surface

- Tauri commands: `hosts_list`, `host_save`, `host_delete`, `host_set_secret`, `session_open`, `session_write`, `session_resize`, `session_close`, `hostkey_decide`
- SSH handshake (host key, auth), channel data
- Cloudflare Access: HEAD redirect parsing, `/transfer/<pubkey>` polling, WSS upgrade
- Terminal escape sequences from the remote host into xterm.js
- `hosts.json` parsing

### 5. Threats (STRIDE)

| Threat | STRIDE | Likelihood | Impact | Mitigation |
| ------ | ------ | ---------- | ------ | ---------- |
| MITM impersonates sshd, captures password | S | Med | High | Strict known_hosts check. Unknown key: block until the user approves the SHA256 fingerprint. Changed key: hard fail, no override in UI. |
| Webview reads stored passwords | I | Low | High | Secrets are write-only over IPC. No command returns a secret. Keychain access only in Rust. |
| Another local user reads secrets/tokens | I | Med | High | Secrets and CF tokens live in the OS keychain. `hosts.json` is written with mode 0600 and holds no secrets. |
| Malicious host header writes files outside config dir | T | Low | Med | CF tokens go to the keychain, keyed by a validated hostname. No path built from server data. |
| Server-controlled login URL opens `file://`/custom scheme | E | Med | Med | Implemented (`cfaccess::validate_login_url`, tested): https only, host must equal the Access app domain, exact `/cdn-cgi/access/cli` path, default port, no fragment/userinfo. Rust opens it via the opener plugin; the webview has no open-URL capability. |
| XSS in UI pivots to Rust commands | E | Low | High | Strict CSP (`default-src 'self'`, no `unsafe-eval`). Capabilities limit the webview to app commands. Every command validates its input (lengths, ids, port range). |
| Terminal escape abuse (OSC 52 clipboard write, link spoof) | T | Med | Low | xterm.js defaults: no OSC 52 clipboard handler, and link activation goes through the same validated opener (https only, confirm dialog). |
| Corrupt/hostile `hosts.json` crashes app or wipes data | D/T | Low | Med | Typed serde parse. On error: back up to `hosts.json.bak-<ts>`, surface the error, never overwrite silently. Atomic write (tmp + rename). |
| Session data leaks to logs | I | Low | Med | `tracing` never logs channel data or secrets. Host/user logged at info only. |
| Hung/slow server exhausts resources | D | Low | Low | Connect timeout of 10 s, keepalive, monitor polls skip while the previous poll is in flight. |
| Unsigned auto-update replaced with malware | T/E | Med | High | No auto-updater until release signing exists (Tauri updater needs a signature). |

### 6. Residual Risks

- Headless Linux without Secret Service: `keyring` fails. The app refuses to save the secret and asks each connect. No plaintext fallback.
- TOFU on first connect: the user can approve a malicious key on day one. Shown fingerprint + docs on verifying out-of-band.
- Cloudflare token-transfer protocol is ported from upstream behaviour and `cloudflared`. Breakage is availability, not confidentiality (the `crypto_box` authenticates the payload). The login page the user sees is served by Cloudflare itself; the app only opens an https URL on the app's own domain.
- System webview CVEs depend on OS patching.
- `freezePrototype` is off: xterm.js 6 assigns `toString` on plain objects and throws under a frozen `Object.prototype`. Prototype pollution needs untrusted object merging in the UI, which this app does not do (terminal bytes go to xterm's parser, IPC payloads are typed in Rust).

### 7. Controls Required (before shipping)

- [x] known_hosts strict check + fingerprint approval flow, with tests (unknown / match / mismatch / other-algorithm), mutation-tested
- [x] No IPC command returns a secret (test asserts the response shapes)
- [x] Keychain storage for passwords, passphrases (done) and Cloudflare Access tokens (`cf-access:<domain>:<kid>`, write-only over IPC)
- [x] Login URL validator with tests (https, host == app domain, no userinfo, `/cdn-cgi/access/cli` path only, rejects `file:`, `javascript:`, wrong host/path/port/fragment)
- [x] SSH host-key check enforced over the tunnel: the WSS pipe feeds `russh::connect_stream`, so TOFU/mismatch applies to the Access domain (e2e test runs a handshake through a ws:// bridge)
- [x] `hosts.json` atomic write + 0600 + backup-on-corrupt, with tests
- [x] CSP + minimal capabilities in `tauri.conf.json`
- [x] Input validation on every command (port 1–65535, host length ≤ 253, id = UUID, write ≤ 64 KiB, terminal 1–1000)
- [ ] No secrets or channel data in logs
