# Polaris

A fast, friendly **WinUI 3** desktop client for the [EasyTier](https://github.com/EasyTier/EasyTier)
mesh VPN — with the EasyTier core **embedded directly in the app**, so there is
no separate `easytier-core.exe` daemon to install or manage.

> Polaris is free software, licensed under the **GNU GPL v3.0**.

## Features

- **Embedded EasyTier core** — the mesh-VPN engine runs in-process as a Rust
  library; no external daemon or CLI.
- **Multiple networks at once** — connect to several EasyTier networks
  simultaneously, each with its own profile.
- **VPN or proxy** — full VPN via a TUN adapter when running as administrator;
  otherwise SOCKS5 and port-forward proxies still work, no admin required.
- **P2P mesh** — TCP / UDP / WebSocket transports, WireGuard crypto, subnet
  proxy, a SOCKS5 portal, and magic DNS.
- **Native Windows UX** — system-tray icon, single-instance handling, config
  import/export, light/dark themes and Mica/Acrylic materials.
- **Bundled Wintun driver** — ships the WireGuard-signed `wintun.dll` so the
  TUN adapter loads deterministically instead of a foreign copy found on `PATH`.

## Requirements

- Windows 10 version 2004 (build 19041) or later. **Windows 11** is required for
  the elevated (VPN-capable) MSIX build.
- The **Windows App SDK** runtime (WinUI 3) — install the runtime, or place the
  WinAppSDK DLLs next to the executable.

## Building

Polaris builds with Cargo and a recent Rust toolchain (edition 2024, Rust
1.85+):

```powershell
cargo build --release --bin polaris_et
cargo run --release
```

The build pulls `windows-reactor` and `easytier` from Git (see `Cargo.toml`) and,
via `build.rs`, embeds the app icon and drops the bundled `wintun.dll` next to
the produced executable.

### Administrator / VPN

Creating the TUN adapter needs administrator rights. Polaris never forces
elevation:

- **Without admin** it runs proxy-only (SOCKS5 / port-forward) and won't error
  when connecting.
- Enable **Settings → Always launch as administrator**, or click **Restart as
  administrator now**, to bring up full VPN mode.

## Packaging (MSIX)

Polaris can ship as an MSIX for the Microsoft Store or sideloading, in two
shapes:

| Build | TUN VPN | Proxy |
| --- | --- | --- |
| `cargo build --release` (proxy-only) | No | Yes |
| `cargo build --release --features msix` (VPN-capable, Win 11+) | Yes | Yes |

The VPN-capable build embeds a `highestAvailable` UAC manifest and pairs with
the `allowElevation` capability so the packaged app can elevate. See
**[packaging/PACKAGING.md](packaging/PACKAGING.md)** for the full layout / pack /
sign steps and the Windows 11 and Store-certification caveats.

## Project layout

| Path | What |
| --- | --- |
| `src/main.rs` | App shell, root state, render loop, startup & elevation |
| `src/ui.rs` | WinUI pages (Networks, Settings, About) |
| `src/engine.rs` | EasyTier instance lifecycle |
| `src/config.rs` | Network profiles & persisted settings |
| `src/elevate.rs` | Privilege & MSIX-packaging detection |
| `src/instance.rs` | Single-instance coordination |
| `src/tray.rs` / `src/dialog.rs` | Tray icon, native file dialogs |
| `build.rs` | Icon embed, bundled `wintun.dll`, MSIX UAC manifest |

## License

Polaris is licensed under the **GNU General Public License, version 3** — see
[`LICENSE`](LICENSE). It is distributed in the hope that it will be useful, but
WITHOUT ANY WARRANTY.

Third-party components and their licenses (EasyTier, windows-reactor, the Wintun
driver, and the Fluent UI icon) are listed in [`CREDITS.md`](CREDITS.md).
