# 2026-11-08 — FishTerm → MEO edition: fork + review

## [--:--] Fork upstream

- **Action:** `gh repo fork sirilerklab/FishTerm --clone` → origin `leotawee/FishTerm`, upstream `sirilerklab/FishTerm` (HEAD `79fa6a2`)
- **Why:** User asked to fork. MIT license allows it.
- **Result:** First try failed (`--remote` flag rejected); retry without it succeeded. Fork landed under `leotawee`, not the active gh account `VacTuzX-dot` (gh default owner).
- **Lesson:** `gh repo fork --clone` already adds `upstream`; extra flags can break it. Check which owner gh forks into.

## [--:--] Read-only review (main, preload, renderer sinks, CI)

- **Action:** Read all of `packages/main/src`, `packages/preload/src`, `SSH.vue`, `settings.vue`, `file.vue`, CI + builder config.
- **Why:** Prime Directive 1. In Electron the attack surface is main ↔ preload ↔ IPC, so read those first.
- **Result:** Findings below. No code changed. Deps not installed, so typecheck/lint/test not run.
- **Lesson:** An SSH client's #1 job is host-key verification. Check `verify(true)`-style shortcuts first.

### Findings

Security:
- P0 Host key never verified: `ssh-protocol/index.ts:27`, `cloudflared/websocket.ts:39` call `verify(true)`. MITM gets password.
- P1 Passwords stored as hex ("base16" ≠ encryption) in `~/.fishssh/ssh.json`, default file mode (`config.ts:56`, `utils.ts`).
- P1 Plaintext password sent to renderer (`index.ts:74-79` `ssh.config.info`; `index.ts:45-51` `_value: info`).
- P1 Preload exposes raw `shell.openExternal(url)` (`preload/src/url.ts`). Login URL comes from server header `cf-access-domain`.
- P1 Electron 22 (EOL) + `sandbox: false` (`mainWindow.ts:11`).
- P2 CF token filename built from server header + query `kid` → path traversal into `path.join` (`cloudflared/config.ts:7,30`). Token written with default mode.
- P2 `ssh2-streams` deprecated (merged into `ssh2` v1). `ssh2` is in devDependencies and unused.
- P3 IPC handlers trust renderer input (no schema, `JSON.parse` unchecked, configId as object key).

Correctness:
- `package.json:18` stray backtick `te\`st:preload` → `npm test` broken.
- `monitor.ts:22` `$((NEW_EX))` typo → RX always 0. `eth0` hardcoded.
- `monitor/network.ts:3-4` module-level RX/TX shared by all sessions → wrong numbers with >1 tab.
- Monitors run inside interactive `shell` channel → MOTD/prompt mixed into parser input, chunked data parsed partially. Use `exec` per poll.
- `monitor/disk.ts` field names swapped (Used↔Available); `diskS[8].replace` throws on empty output.
- `config.ts:31-51` `return e` → Error object treated as config; next save writes `{}`-ish garbage = data loss. Non-atomic writes.
- `cloudflared/access.ts:40` emits error but continues; `login()` null token → `token.app_token` TypeError → unhandled rejection in main.
- `index.ts:152,214` `readFileSync(keyPath)` throws inside emitter → uncaught in main. Passphrase keys unsupported.
- `SSH.vue` window `resize` listener + `Terminal` never disposed → leak per tab.
- Dead/duplicate code: `config.load` handler no-op, `preload/config.ts` duplicates `page.ts`.

Repo hygiene:
- Build tools in `dependencies` (vite, eslint parser, vite plugins) → shipped in app. Both `package-lock.json` and `yarn.lock` + `yarn-error.log` committed. `.idea/` tracked.
- CI on Node 16 (EOL), no code signing, autoUpdater on unsigned builds.
- 0 tests (`tests/unit.spec.ts` only; e2e removed).

## [--:--] Decision: scope (mini-ADR)

- **Action:** Asked Leo via 2 questions.
- **Why:** Rewrite vs harden is expensive to reverse (Prime Directive 4: resolve ambiguity first).
- **Result:** Leo: no Electron, Rust. Tauri v2 (Rust core + Vue/xterm.js UI), branch `meo-rust` in the fork. Deps change approved.
- **Rejected:** harden the Electron fork (Leo ruled out Electron); pure egui GUI (terminal from scratch = weeks); ratatui TUI (no window); separate repo (loses upstream history).
- **Lesson:** Present 2–3 concrete options with a preview; a free-text answer ("ไม่เอา electron build rust") can still override them, so re-ask the narrower follow-up.

## [--:--] Branch + ADR + threat model

- **Action:** `git switch -c meo-rust`; wrote `docs/adr/ADR-001-rewrite-in-tauri-rust.md`, `docs/threat-model.md`.
- **Why:** CLAUDE.md requires an ADR for an architecture change and a threat model for a new auth/external-integration surface. Templates from `~/.claude/templates/` (repo has none).
- **Result:** Both docs written. Status Proposed, waiting for Leo's go on the plan + crate list.
- **Lesson:** Write the controls list before the code; it becomes the test checklist.

## [--:--] Phase 1 — remove legacy + frontend scaffold

- **Action:** `git rm -r packages yarn.lock yarn-error.log .idea package-lock.json` (approved). Rewrote `package.json`. `bun add --exact` runtime + dev deps. Wrote `vite.config.ts`, `ui/{index.html,tsconfig.json,src/main.ts,src/App.vue,src/api.ts,src/env.d.ts}`.
- **Why:** Phase 1 of plan. `bun add --exact` resolves real latest versions and pins them (supply-chain rule) instead of guessing numbers.
- **Result:** `bun run build` OK (vue-tsc + vite 8, 64 kB JS). Two failures on the way:
  1. `typescript@7.0.2` → vue-tsc `ERR_PACKAGE_PATH_NOT_EXPORTED` (TS 7 is the native compiler, no `lib/tsc` JS API). Pinned `typescript@6.0.3`.
  2. Vite loaded leftover root `postcss.config.js` (CJS under `"type":"module"`). Set `css.postcss: {}` inline instead of deleting an unapproved file.
- **Lesson:** "latest" of a toolchain can break its plugins. Pin, then verify the build right away. 📌 GENERAL: TS 7 (tsgo) breaks tools that `require('typescript/lib/tsc')`.
- **Pending (needs approval to delete):** Electron leftovers `.electron-builder.config.js .eslintignore .eslintrc.json .nano-staged.mjs .simple-git-hooks.json postcss.config.js tailwind.config.js vitest.config.js scripts/ types/ version/ .github/workflows/*`.

## [--:--] Phase 1/2 — Rust core scaffold + host store + keychain

- **Action:** `src-tauri/` (Cargo.toml, build.rs, tauri.conf.json, capabilities/default.json, icons via `tauri icon`), `src/{lib,main,error,secrets,store,commands}.rs`. Crates via `cargo add`: tauri 2, tauri-build 2, serde, serde_json, thiserror, tracing(+subscriber), uuid, keyring 4.2.0; dev: tempfile.
- **Why:** Interface first: `Host` has no secret field (cannot leak by type), `HostInput` is `deny_unknown_fields`, secrets are write-only over IPC, errors serialize to a generic message. Logic sits in `AppState` methods so tests run without Tauri.
- **Result:** `cargo test` 17/17, `cargo clippy --all-targets -D warnings` clean, `cargo fmt` clean, `tauri build --debug --no-bundle` OK, app stayed up 8 s with no errors. UI not inspected visually.
- **Dead ends:**
  1. `cargo add` before `src/lib.rs` existed → "can't find library". Wrote stubs first.
  2. keyring is v4 now (keyring-core split). Read `v1.rs` source before coding: the `v1` default feature keeps `Entry::new/set_password/get_password/delete_credential`; `keyring::Error::NoEntry` still exists.
  3. clippy: `#[expect(dead_code)]` on `SecretStore::get` went unfulfilled in the test build (tests call `get`) → `cfg_attr(not(test), expect(...))`. `type_complexity` on a test table → `type Mutator`.
- **Lesson:** `#[expect]` beats `#[allow]` for "used later" code: it errors once the code is used, so the suppression cannot rot. 📌 GENERAL: lint attributes behave differently per cfg (test vs lib).
- **Security items (Lesson = attack prevented):**
  - Keychain secrets: another local user or a backup tool reading `hosts.json` gets no passwords.
  - 0600 + atomic rename: no world-readable window, and a crash mid-write cannot truncate the host list.
  - Corrupt file moved aside: a bad edit or disk error no longer silently wipes every saved host (upstream `return e` bug).
  - Generic error strings: the webview never learns filesystem paths or keychain internals.
  - CSP + capability `core:event:default` only: XSS in the UI cannot reach fs/shell/opener.

## [--:--] README credits

- **Action:** Rewrote `README.md`: MEO status table, dev steps, Credits (FishTerm by M-307, upstream thanks), kept the original copyright line in `LICENSE` untouched.
- **Why:** MIT requires the notice to stay. Leo asked for good credit.
- **Result:** Done.

## Summary (Phase 1–2)

- Changed: removed Electron `packages/`, yarn/npm lockfiles, `.idea/`; new `package.json` + `bun.lock`, `vite.config.ts`, `ui/`, `src-tauri/`, `README.md`, docs (ADR-001, threat model, this log).
- Unchanged: `main` branch, `LICENSE`, `upstream` remote, `assets/`, `buildResources/`.
- Remaining: Phase 3 (russh + known_hosts, P0), 4 (monitor), 5 (Cloudflare Access), 6 (UI port), 7 (CI). Delete Electron leftovers (needs approval). Nothing committed or pushed.
- Key decisions: ADR-001; `Host` single type without secrets; store lock held across keychain calls.
- Top 3 lessons:
  1. 📌 GENERAL: Pin toolchain versions and build right away; "latest" TS 7 broke vue-tsc.
  2. 📌 GENERAL: Make the secret impossible to serialize (type has no field) instead of remembering to strip it.
  3. Read a dependency's source when its major version jumped; don't code from memory.
- Mistakes & dead ends: `gh repo fork --remote` flag; `ls` alias printed nothing (used `/bin/ls`); `echo ===` breaks in zsh; TS 7; postcss leftover; cargo add without lib.rs; clippy expect/test cfg.

## [--:--] Commit phase 1-2 + remove Electron leftovers

- **Action:** commit `2405c3d`. `git rm -r` 18 leftover files (builder config, eslint/prettier hooks, postcss/tailwind/vitest configs, `scripts/ types/ version/ .github/workflows`). Removed the `css.postcss` workaround from `vite.config.ts`.
- **Why:** Leo approved (1, 2). Old CI would run `npm ci` against a lockfile that no longer exists. The workaround only existed because of `postcss.config.js`.
- **Result:** `bun run build` OK after removal. First `git rm` failed: zsh does not word-split `$T`; used `${=T}`.
- **Lesson:** 📌 GENERAL: in zsh, `$VAR` with spaces stays one word. Use an array or `${=VAR}`. Delete workarounds when their cause goes.

## [--:--] Phase 3 — russh client, strict known_hosts, sessions (closes P0)

- **Action:** `cargo add russh@0.65.1 tokio`. New `src/ssh.rs`; `commands.rs` gains `target_for`, `session_open/write/resize/close`, `hostkey_decide`; `error.rs` gains host-key/auth/timeout variants + `From<russh::Error>`; `lib.rs` wires `~/.ssh/known_hosts`. Test-only keys in `tests/fixtures/` (generated with ssh-keygen, README says so).
- **Why:** P0: upstream `verify(true)` let any MITM capture the password. Read russh source first (0.65 API: async `Handler::check_server_key`, `known_hosts` module, `ChannelOpenHandle`).
- **Decision (mini-ADR):** own `host_key_status` instead of `russh::keys::check_known_hosts_path`. Rejected the russh function because it returns "unknown" when the recorded key has a different algorithm, so an attacker offering another key type gets a TOFU prompt instead of a hard fail.
- **Decision:** terminal bytes go over `tauri::ipc::Channel` as `InvokeResponseBody::Raw` (no JSON/base64, no UTF-8 assumption); control events are JSON on the same channel. Rejected global `emit` events (needs wider capability and per-session event names like upstream).
- **Result:** `cargo test` 27/27, including real handshakes against an in-process russh server: reject → nothing recorded; accept → learned, then trusted without prompt + echo through PTY shell; changed key → `HostKeyChanged` with no prompt; wrong password → `AuthFailed`; encrypted key passphrase cases. Mutation test: forcing Mismatch → `Ok(true)` makes `changed_key_is_refused_without_prompt` fail; restored. `clippy -D warnings` clean; debug app launches.
- **Dead end:** test helper `s.data(ch, data.to_vec().into())` → E0283 (generic param); dropped `.into()`.
- **Lesson:** Security on a library's "check" helper means reading its edge cases; `check_known_hosts_path` is not strict. 📌 GENERAL: mutation-test the one assertion that guards a P0. A test you never saw fail proves nothing.
- **Security Lesson (attack prevented):** strict host-key check stops a rogue Wi-Fi/ARP-spoofing attacker from posing as the server and collecting the password.
- **Remaining:** UI for sessions/prompt (Phase 6); `shortcut:` unanswered prompt senders stay in a map until exit; host certificates refused; zeroize still TECH DEBT.

## [--:--] Correction: drop "MEO" branding, keep the FishTerm name

- **Action:** Renamed product/title to `FishTerm`, crate `meo-fishterm` → `fishterm` (lib `fishterm_lib`), identifier + keychain service `dev.meo.fishterm` → `io.github.leotawee.fishterm`, package.json name, README/ADR/threat-model text. `git mv` this log to `2026-11-08-fishterm-rust-rewrite.md`. Branch `meo-rust` → `tauri-rust` (local only).
- **Why:** Leo: keep the name FishTerm, no MEO. Identifier uses the fork's GitHub namespace instead of a domain nobody here owns. No secrets were ever saved under the old keychain service, so nothing is orphaned.
- **Result:** `cargo test` 27/27, clippy clean. Earlier entries in this log still say "MEO" (append-only rule: not edited). Commit `2405c3d` message also says "MEO edition" (history not rewritten).
- **Lesson:** Ask about naming/branding before scaffolding; renames touch identifiers that are costly to change after release (keychain service, app data dir).

## [--:--] Port the original FishTerm UI (Phase 6 pulled forward)

- **Action:** Leo: "UI ตาม fishterm เดิมทั้งหมด". Approved deps: tailwindcss 3.4.19 + postcss (dev), FontAwesome 7.3.1 (svg-core, solid, brands, vue-fontawesome 3.3.3), tauri-plugin-dialog 2.8.4 (Rust + JS). Copied original components verbatim from `main` (Button, Card, Container, Title, Dialog, text, select, useClickOutside), logo + FiraCode fonts. Rewrote data layer of App/Home/settings/SSH/file/info onto Rust commands. Backend: `AppError` now serializes `{code, message}`; `closed` event carries `error`; dialog plugin + `dialog:allow-open` capability.
- **Why:** Same look as upstream; UI switches screens by error `code` instead of parsing messages.
- **Deliberate deviations (security / bugs):** saved password is never loaded back into the edit form (upstream loaded plaintext, P1); added passphrase field; key file uses the native picker (Electron `File.path` doesn't exist in Tauri); host-key trust dialog and generic error screen are new; sweetalert2 replaced by the existing Dialog; monitor footer shows `--` until Phase 4; fixed upstream bugs: `fa-soild` icon typo, missing `faTimes` registration, Cancel button `type="submit"`, leaked resize/click listeners, xterm never disposed.
- **Result:** `vue-tsc` + vite build OK first try (546 kB JS, warning only). `cargo test` 27/27 + new error-shape assertion; clippy clean.
- **Debug (dead end → fix):** app logged `TypeError: Attempted to assign to readonly property.` at `@xterm_xterm.js:1761 o.toString = s`. Hypotheses: (1) `freezePrototype: true` override mistake, (2) CSP block, (3) Vue/FontAwesome. Stack pointed at xterm, CSP errors look different → tested (1): removed `freezePrototype`, app rebuilt, error gone. Recorded as residual risk in the threat model.
- **Not verified:** visual check. `screencapture` returned a black image (no Screen Recording permission); did not pursue further.
- **Lesson:** 📌 GENERAL: freezing built-in prototypes breaks libraries that assign `toString`/`constructor` on plain objects (the "override mistake"); test hardening flags against every dependency at runtime, not only at build.

## [--:--] Debug: UI breaks on window resize

- **Repro:** Leo reported resize UX bugs (no detail). Screen capture of the Tauri window is blocked (no Screen Recording permission), so the same UI ran from the Vite dev server in Chrome with a mocked `window.__TAURI_INTERNALS__` (fake `hosts_list`/`session_open`/`session_resize`), inside an iframe sized like the Tauri window (Chrome window was full-screen and would not shrink). Measured with `getBoundingClientRect`, not by eye.
- **Fail paths found:**
  - A. Home list `h-[800px]`: at 816×456 the list ended at 936px, so hosts below the fold were unreachable.
  - B. Dialog `pt-24 … sm:max-h-screen`: bottom 10px past the viewport at 816×456 (worse below `sm`).
  - C. Terminal: the first resize after connect always overflowed (34 rows / 646px in a 620px box, hidden behind the footer); later resizes were correct.
  - D. Tabs fixed at 250px wrapped onto a second row on narrow windows; the newest tab could sit off-screen.
- **Hypotheses for C (ledger):** (1) `fit()` before layout settles: disproved, a single-step resize + re-fired event gave correct rows. (2) web font: first test (measure right after connect) showed no overflow → looked disproved, but the exact repro loop showed only the first resize overflows with an unchanged 19px row height → xterm measured cells at `open()` with the fallback font; FitAddon used stale metrics once Fira Code loaded. (3) footer height change: footer stayed 32px.
- **Fix:** `await document.fonts.load('15px "Fira Code"')` before `term.open()`; `ResizeObserver` on the terminal box instead of `window.resize`, one fit per animation frame, `session_resize` only when cols/rows change (was sent twice); Home list `flex-1 min-h-0` inside a full-height Container; Dialog `p-4` + `max-h-[calc(100vh-2rem)]`; tab strip `overflow-x-auto` + `shrink-0`, active tab `scrollIntoView`.
- **Verified (same harness):** list ends at 420px of 456 and scrolls to the last host; dialog 16→440 of 456; terminal never overflows across 1100×700 → 700×420 → 1300×800 → 640×400 → 1100×700; nav stays 44px with 4 tabs and the newest tab is visible. `vue-tsc` + build OK.
- **Lesson:** 📌 GENERAL: when a bug only shows "the first time", re-run the exact original sequence; a simplified repro can falsely disprove the right hypothesis. 📌 GENERAL: size xterm.js after its web font has loaded, and observe the container, not the window.

## [--:--] Release prep (macOS) — blocked on an unexplained exit

- **Action:** checks green (cargo test 27/27, clippy, fmt, build); commit `0ed9f39`; `bun run tauri build` → `FishTerm.app` 9.86 MiB, `FishTerm_0.1.0_aarch64.dmg` 4.73 MiB, sha256 `5b579d33…7220`, ad-hoc signed (no Developer ID).
- **Problem:** smoke launches of the bundle "EXITED" within 6–8 s on 2 of 5 runs (once logging `web content process terminated`); the raw `target/release/fishterm` stayed up; later bundle runs stayed up.
- **Confounder found:** a run's log shows `session opened` → `session closed` 1 s later for a saved host. The app never auto-connects, so a person was using the windows I launched. "EXITED" may be the window being closed by hand. Stopped launching windows to avoid interfering.
- **Open question:** that session closed cleanly 1 s after opening, preceded by `WARN ssh error=Disconnect`. Could be the server ending the shell or a client bug; needs Leo's account of what happened.
- **Lesson:** smoke tests that spawn GUI windows on the user's desktop are not isolated; a human can be part of the result. Ask before interpreting.
- **Resolved:** Leo confirmed he clicked/closed those windows. The exits were manual, not crashes. The `WARN ssh error=Disconnect` before that session is still unexplained (session itself opened fine); tracked as a follow-up.
- **Decision:** Leo approved `git push origin tauri-rust` + GitHub pre-release `v0.1.0` with the aarch64 dmg.

## [--:--] Phase 5 — Cloudflare Access: token-transfer login + SSH over the WSS tunnel

- **Action:** New `src/cfaccess.rs` (probe → cached-token check → NaCl-box login → transfer polling → `WsPipe`), `ssh.rs` gains `connect_on` over any stream, `commands.rs` routes `CloudflareAccess` hosts through it, `lib.rs` registers `tauri-plugin-opener` (Rust-side only). UI: `cf-login` progress events on the session channel. Deps pinned: reqwest 0.12 (rustls), tokio-tungstenite 0.28 (rustls + handshake), crypto_box 0.9, base64 0.22, url 2, http 1, futures-util 0.3; dev: wiremock.
- **Why:** Phase 5 of the plan. Leo asked to connect via Cloudflare first. The protocol is the upstream Electron flow (read from `upstream/main` git history): HEAD with `cloudflared/2022.10.0` UA → `kid` in final query + `cf-access-domain` header; box keypair; `/cdn-cgi/access/cli` login URL; poll `login.cloudflareaccess.org/transfer/<pubkey>`; decrypt with the `service-public-key`; `wss://<domain>` with `Cf-Access-Token` as a raw byte pipe.
- **Result:** `cargo test` 45/45 (was 27), clippy clean on all touched files, `cargo fmt` clean, `vue-tsc` + vite build OK. Key tests: login-URL validator (file/javascript/http/userinfo/host/path/port/fragment); crypto interop with a simulated transfer service (nonce-first blob, padded URL-safe keys, JSON envelope); JWT `exp` cache rules incl. unpadded base64; probe redirect parsing on wiremock; dynamic transfer mock that encrypts to whatever key the client generated; **end-to-end SSH handshake + password auth + echo through a real ws:// bridge into russh, with TOFU learn/no-reprompt**.
- **Security deltas vs upstream (each closed a finding from the review):** host keys are no longer accepted blindly (`verify(true)` P0) — the strict known_hosts check runs over the tunnel, keyed on the Access domain; the token lives in the OS keychain, not `~/.cloudflared` 0644; the login URL is validated (https, exact host, no userinfo) and opened by Rust via the opener plugin — the webview has no open-URL primitive; upstream's two crash paths (`kid` type confusion, `null.app_token`) are typed errors (`CfProbe`/`CfTransfer`).
- **Dead ends:**
  1. `git checkout Cargo.toml` while re-pinning deps silently reverted the first `cargo add` batch — re-added from scratch. 📌 GENERAL: don't reset a manifest between batched `cargo add` calls.
  2. wiremock matches mocks **in mount order**: an unqualified `method("HEAD")` mock swallowed the later `/login` mock and caused an infinite redirect in the probe test. Scope every mock with a path matcher.
  3. Pre-built transfer mocks can't work for the full flow: `ensure_token_on` generates its own keypair, so the mock must be a dynamic responder that encrypts to the key from the request path — and it must return the JSON envelope `{"app_token":..}` (encrypting bare text fails serde) — and a well-formed JWT with future `exp`, or the cache path rejects it.
  4. `probe_url`'s https-final-URL check broke http wiremock tests; the check now applies only when the configured base is https (production always is; `validate_app_host` rejects schemes from user input).
  5. `&dyn Fn` params made `open_session`'s future `!Send` (Tauri needs Send); fixed with `&(dyn Fn(..) + Send + Sync)`. 📌 GENERAL: trait-object callbacks crossing an await must carry Send + Sync.
  6. Passing `Target` by value into a shared handshake tail after building the handler from its fields → partial-move errors; handler now clones fields, `Target`/`Credential` derive `Clone` (core-internal only, never serialized over IPC).
- **Lesson:** 📌 GENERAL: read the dependency's source *and* the upstream's original code before porting a protocol — the base64 padding choice, the nonce-first blob layout, and the JSON envelope all came from reading `encrypt.ts`, not from guessing. New clippy on this machine flags pre-existing lints in `stats.rs`/`store.rs` (fresh toolchain, not this change).
- **Not verified:** a live sign-in against a real Cloudflare tunnel (needs Leo's tunnel + Access app). The offline suite covers the protocol pieces individually; first real connect should be watched once.
- **Remaining:** zeroize (pre-existing TECH DEBT), `cf-access` token deletion when a host is deleted (tokens are keyed by domain, shared across hosts — deliberate: same tunnel reused), UI for token status on the host card.
