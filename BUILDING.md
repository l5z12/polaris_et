# Building Polaris

Polaris is a Windows-only WinUI 3 app. It builds with Cargo, but the build pulls
two fast-moving Git dependencies (`windows-reactor` from `microsoft/windows-rs`,
`easytier` from `EasyTier/EasyTier`) and several build scripts that need `protoc`
and the Windows App SDK. This file covers the local setup; for end-user notes see
[`README.md`](README.md), and for MSIX packaging see
[`packaging/PACKAGING.md`](packaging/PACKAGING.md).

## Prerequisites

- **Windows 10 2004 (build 19041)+ / Windows 11**, x64.
- **Rust** with the `x86_64-pc-windows-msvc` toolchain — edition 2024, so
  **Rust 1.85+**. (`rustup default stable`.)
- **MSVC build tools + Windows SDK** (Visual Studio Build Tools "Desktop
  development with C++"). Required for linking and for the C deps (`ring`, etc.).
- **Git** on `PATH` (the build fetches Git dependencies via the Git CLI).
- **protoc** (Protocol Buffers compiler). EasyTier's `prost-*` build scripts need
  it; a copy is vendored at `tools/protoc/bin/protoc.exe` (the `tools/` dir is
  git-ignored — drop a `protoc` build there or install one and point `PROTOC` at
  it).
- **Windows App SDK runtime** to *run* the app (the WinUI 3 dependency). The
  build downloads the WinAppSDK *build-time* payloads itself (see below).
- **Network access** while building: Cargo fetches the Git deps, and
  `windows-reactor`'s build script downloads WinAppSDK nupkgs. A proxy can be
  configured in `.cargo/config.toml`.

## Required: `.cargo/config.toml`

This file is **git-ignored** (it contains machine-specific absolute paths and a
local proxy) but is **required** to build. Create `.cargo/config.toml` at the
repo root:

```toml
[env]
# windows-reactor's build.rs locates the Windows App SDK nupkgs relative to
# CARGO_WORKSPACE_DIR, a var the windows-rs workspace normally injects. As a Git
# dependency that injection is gone, so point it at THIS repo's root (absolute,
# trailing slash, forward slashes).
CARGO_WORKSPACE_DIR = "C:/path/to/polaris_et/"

# prost-build (an EasyTier dependency) needs protoc on PATH or via PROTOC.
PROTOC = "C:/path/to/polaris_et/tools/protoc/bin/protoc.exe"

# Optional — only if you build behind a proxy. curl reads the lowercase names;
# reqwest/cargo read the uppercase ones.
# HTTP_PROXY  = "http://127.0.0.1:10808"
# HTTPS_PROXY = "http://127.0.0.1:10808"
# http_proxy  = "http://127.0.0.1:10808"
# https_proxy = "http://127.0.0.1:10808"

# Optional — proxy for crates.io/registry over HTTP.
# [http]
# proxy = "http://127.0.0.1:10808"

[net]
# Fetch Git dependencies with the Git CLI (more reliable behind proxies/auth).
git-fetch-with-cli = true
```

Replace `C:/path/to/polaris_et/` with this checkout's absolute path. Drop the
proxy lines if you have direct internet access.

## Build & run

```powershell
# Debug
cargo build --bin polaris_et
cargo run

# Release
cargo build --release --bin polaris_et
```

For the VPN-capable MSIX build, add `--features msix` — see
[`packaging/PACKAGING.md`](packaging/PACKAGING.md).

The first build is slow: it compiles the EasyTier core and the windows-rs crates
from Git, and downloads the WinAppSDK payloads.

## What the build does (`build.rs`)

- Embeds the app icon (`assets/polaris.ico`, resource id 1) and version info
  (`FileDescription` = "Polaris", so the taskbar shows the friendly name).
- Copies the bundled, WireGuard-signed `wintun.dll` (`wintun/bin/<arch>/`) next
  to the produced exe, so EasyTier loads *ours* rather than a stray `wintun.dll`
  from `PATH`.
- With `--features msix`, embeds a `highestAvailable` UAC manifest.

`windows-reactor`'s build script downloads the Windows App SDK nupkgs into
git-ignored staging dirs (`temp/`, `winmd/`, `*.nupkg`).

## Dependency pinning

`windows-reactor` and `easytier` are Git deps pinned by `rev` in `Cargo.toml`
(not just `Cargo.lock`) because `microsoft/windows-rs` `main` and EasyTier move
fast and have unstable APIs. Bump a `rev` deliberately and migrate call sites;
don't switch them to unpinned `git = "…"`.

## Troubleshooting

- **`protoc` not found / `prost-build` errors** — set `PROTOC` in
  `.cargo/config.toml` to an existing `protoc.exe`.
- **WinAppSDK download / `Class not registered` (`REGDB_E_CLASSNOTREG`) at
  startup** — the Windows App SDK runtime is missing or the build couldn't fetch
  its payloads; check `CARGO_WORKSPACE_DIR` and network/proxy.
- **Build "worked yesterday" but now fails with renamed APIs** — a Git dep
  floated. Make sure `Cargo.toml` pins both `windows-reactor` and `easytier` by
  `rev`.
- **`Access is denied` removing `target\…\polaris_et.exe`** — an instance is
  running; close it (or `Stop-Process polaris_et`) before rebuilding.
