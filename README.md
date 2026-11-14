<center>
  <img src="./assets/fishterm-w.svg" style="background-color: white; padding: 10px; border-radius: 25px" />
  <p align="center">A open-source Terminal remote access</p>
</center>

> Rust rewrite of [FishTerm](https://github.com/sirilerklab/FishTerm) on Tauri v2. Branch `tauri-rust`. Work in progress. See [ADR-001](./docs/adr/ADR-001-rewrite-in-tauri-rust.md) and the [threat model](./docs/threat-model.md).

## Status

| Feature | Upstream (Electron) | This fork (Tauri + Rust) |
| --- | --- | --- |
| Saved hosts | ✅ hex-encoded passwords in JSON | ✅ secrets in OS keychain, `hosts.json` 0600, atomic writes |
| Remote SSH | ✅ host key **not** verified | ✅ `russh`, strict `~/.ssh/known_hosts`, password / key + passphrase |
| Cloudflare Access SSH | ✅ | ✅ token-transfer login in Rust, token in keychain, SSH over the WSS tunnel with strict host-key check |
| Monitor (CPU, RAM, disk, net) | ✅ | ✅ live via SSH exec (Linux hosts, others show `--`) |
| SFTP | ❌ | ✅ file browser tied to the session |
| Port forwarding | ❌ | ✅ local forwards (`-L`) tied to the session |
| X11 | ❌ | ❌ (webview cannot render X11 apps) |

## Develop

Requires Rust (stable), Bun, and the [Tauri v2 prerequisites](https://v2.tauri.app/start/prerequisites/).

```sh
bun install
bun run tauri dev          # run the app
cd src-tauri && cargo test # core tests
```

On Linux, saving passwords needs a Secret Service (GNOME Keyring or KWallet).

## Clipboard over SSH (OSC 52)

Copy text from inside vim/micro straight to your system clipboard (paste it anywhere,
including another FishTerm tab). FishTerm writes the clipboard; the remote can never read it.

- **micro** ≥ 2.0.11 — works out of the box (yank as usual).
- **neovim 0.10+** — enable the built-in OSC 52 provider:
  ```lua
  vim.g.clipboard = {
    name = 'OSC 52',
    copy = { ['+'] = require('vim.ui.clipboard.osc52').copy, ['*'] = require('vim.ui.clipboard.osc52').copy },
    paste = { ['+'] = require('vim.ui.clipboard.osc52').paste, ['*'] = require('vim.ui.clipboard.osc52').paste },
  }
  ```
- First use per session asks once (Allow / Deny). `Cmd+C` copies the visible selection,
  `Cmd+V` pastes into the focused tab.


## Credits

This repository is a fork of **[FishTerm](https://github.com/sirilerklab/FishTerm)** by [M-307](https://m307.dev), released under the MIT License. The original idea, UI design, Cloudflare Access flow and monitor design come from FishTerm.

Upstream special thanks:

- [Maythiwat](https://github.com/maythiwat)
- [nicenathapong](https://github.com/nicenathapong)

## License

[MIT](./LICENSE). The original copyright notice (Copyright (c) 2022 M-307) is kept as required.
