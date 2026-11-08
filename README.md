<center>
  <img src="./assets/fishterm-w.svg" style="background-color: white; padding: 10px; border-radius: 25px" />
  <p align="center">MEO FishTerm: SSH client with Cloudflare Access tunnel support</p>
</center>

> **MEO edition** of [FishTerm](https://github.com/sirilerklab/FishTerm), rebuilt on Tauri v2 + Rust.
> Branch `meo-rust`. Work in progress. See [ADR-001](./docs/adr/ADR-001-rewrite-in-tauri-rust.md) and the [threat model](./docs/threat-model.md).

## Status

| Feature | Upstream (Electron) | MEO (Tauri + Rust) |
| ------- | ------------------- | ------------------ |
| Saved hosts | ✅ hex-encoded passwords in JSON | ✅ secrets in OS keychain, `hosts.json` 0600, atomic writes |
| Remote SSH | ✅ host key **not** verified | ⏳ Phase 3: `russh` + strict `known_hosts` |
| Cloudflare Access SSH | ✅ | ⏳ Phase 5 |
| Monitor (CPU, RAM, disk, net) | ✅ | ⏳ Phase 4 |
| SFTP, X11, port forwarding | ❌ | ❌ |

## Develop

Requires Rust (stable), Bun, and the [Tauri v2 prerequisites](https://v2.tauri.app/start/prerequisites/).

```sh
bun install
bun run tauri dev          # run the app
cd src-tauri && cargo test # core tests
```

On Linux, saving passwords needs a Secret Service (GNOME Keyring or KWallet).

## Credits

MEO FishTerm is a fork of **[FishTerm](https://github.com/sirilerklab/FishTerm)** by
[M-307](https://m307.dev), released under the MIT License. The original idea, UI design,
Cloudflare Access flow and monitor design come from FishTerm.

Upstream special thanks:
- [Maythiwat](https://github.com/maythiwat)
- [nicenathapong](https://github.com/nicenathapong)

## License

[MIT](./LICENSE). The original copyright notice (Copyright (c) 2022 M-307) is kept as required.
