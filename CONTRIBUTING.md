# Contributing to Polaris

Thanks for your interest! Polaris is a Windows-only WinUI 3 client for the
EasyTier mesh VPN. This guide covers how to get set up, the conventions to
follow, and a few hard-won gotchas about the `windows-reactor` UI framework that
will save you time.

By contributing you agree your work is licensed under the project's
[GPL-3.0](LICENSE).

## Getting started

1. Set up the toolchain and the **required** `.cargo/config.toml` — see
   [`BUILDING.md`](BUILDING.md). The project won't build without it.
2. Build and run:
   ```powershell
   cargo run
   ```
3. Before opening a PR, make sure it's clean:
   ```powershell
   cargo fmt --all
   cargo clippy --all-targets        # no warnings
   cargo build --release             # and --features msix if you touched packaging
   cargo run                         # smoke-test the actual app
   ```

## Project layout

| Path | What |
| --- | --- |
| `src/main.rs` | App shell, root state, the 1 Hz repaint timer, startup & elevation |
| `src/ui.rs` | All WinUI pages and shared view helpers |
| `src/engine.rs` | EasyTier instance lifecycle + the snapshot the UI renders |
| `src/config.rs` | Profiles, settings, and conversion to EasyTier's `NetworkConfig` |
| `src/elevate.rs` | Privilege / MSIX-packaging detection, UAC relaunch |
| `src/instance.rs` | Single-instance coordination |
| `src/tray.rs` | Tray icon, window icon, taskbar identity, close/minimize-to-tray |
| `src/logging.rs` | Diagnostics: global tracing subscriber → file + in-app panel |
| `src/dialog.rs` | Native file dialogs + clipboard |
| `build.rs` | Icon/version-info embed, bundled `wintun.dll`, MSIX manifest |

## Code style

- **Format with `rustfmt`** (`cargo fmt --all`) — CI-style: no manual deviations.
- **Zero clippy warnings** (`cargo clippy --all-targets`). Prefer fixing over
  `#[allow(...)]`; if an allow is genuinely needed, scope it tightly and comment
  why.
- **Match the surrounding code**: comment density, naming, and idiom. Comments
  explain *why* (especially the non-obvious Win32 / reactor workarounds), not
  *what*.
- New `.rs` files start with `// SPDX-License-Identifier: GPL-3.0-only`.
- Unsafe Win32/COM: keep it localized, document the invariant, and free what you
  allocate (e.g. `PropVariantClear`).

## windows-reactor: read this before touching the UI

The UI framework is `microsoft/windows-rs`'s experimental `windows-reactor`
crate. It's powerful but unstable and under-documented. Key things that will bite
you (all are reflected in code comments — search for them):

- **It's pinned by `rev` in `Cargo.toml` for a reason.** `windows-rs` `main` moves
  fast and renames/reworks the reactor API constantly. Don't switch it (or
  `easytier`) to an unpinned `git = "…"` — a re-resolve will float to HEAD and
  silently break the build. Bump the `rev` deliberately and migrate call sites;
  diff the vendored source under
  `~/.cargo/git/checkouts/windows-rs-*/<rev>/crates/libs/reactor` to find new
  names.
- **It re-renders only from the root** and skips subtrees whose props are
  unchanged. State that must drive a re-render has to live in `main::root` and
  flow down via props (that's why `tab`, `sub_tab`, and the `tick` counter live
  there). Don't put such state in a child component.
- **Force a remount with a distinct component *type*, not a key.** Keys don't
  remount a single child; the reconciler remounts only when `component_type_id`
  changes. The page dispatch in `ui::body_view` uses one `fn *_view` per tab for
  exactly this.
- **A component must never be the bare sole output of another component** — host
  it under a real widget. Otherwise their roots collide on one control id and the
  inner one remounts every render (this caused a scroll-reset bug). `body_view` is
  a plain function for this reason.
- **Only buttons are clickable.** For click-to-copy on plain text use the
  universal `on_tapped` modifier — buttons center/pad their content and won't
  align in tables.
- **Some props are no-ops in the pinned rev.** e.g. the modern `scroll_view`'s
  `content_orientation` is never sent to the backend, so horizontal scrolling
  needs the legacy `scroll_viewer`. When a modifier "does nothing," check whether
  the widget's `bindings()` actually emit it before assuming a logic bug.

## EasyTier specifics

- `config::Profile::to_network_config` maps Polaris settings to EasyTier's
  `NetworkConfig`. Keep it aligned with EasyTier's defaults; mismatches cause
  subtle connectivity problems (e.g. disabling encryption breaks P2P).
- The build curates EasyTier's feature set (see `Cargo.toml`); `kcp` is omitted
  to avoid the `kcp-sys` C-toolchain dependency.

## Diagnostics when debugging connectivity

Enable **Settings → Diagnostics**, set the level to Debug, and use the
Diagnostics panel (or `%APPDATA%\Polaris\logs\`) — EasyTier's hole-punch and
peer-connection logs flow there.

## Submitting changes

- Branch off `main`; keep PRs focused.
- Write clear commit messages explaining the *why*.
- Confirm `cargo fmt`/`cargo clippy`/`cargo build` are clean and the app runs.
- Note any platform/runtime caveats you couldn't verify (this is a GUI app;
  reviewers may need to eyeball visual changes).
