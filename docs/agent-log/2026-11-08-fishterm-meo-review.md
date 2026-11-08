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
