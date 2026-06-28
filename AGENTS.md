# AGENTS.md

Guidance for AI coding agents working in this repo. The authoritative docs are
[CONTRIBUTING.md](CONTRIBUTING.md) and [BUILDING.md](BUILDING.md); this file
distills the must-knows and adds a few agent-specific conventions.

## What this is

Polaris is a **Windows-only** WinUI 3 desktop client for the EasyTier mesh VPN,
written in Rust on Microsoft's experimental `windows-reactor` UI crate.

## Setup & build

The project will **not** build without `.cargo/config.toml` — set it up per
[BUILDING.md](BUILDING.md) first.

```powershell
cargo run                      # build + run the app
cargo fmt --all                # format (required; no manual deviations)
cargo clippy --all-targets     # must be warning-free
cargo build --release          # add --features msix if you touched packaging/
```

After a change, verify what you can — at minimum `cargo fmt --all`, `cargo clippy
--all-targets`, and a build. This is a GUI app, so a headless agent usually
cannot observe visual or runtime behavior: **state plainly what you verified vs.
couldn't**, and call out any platform/runtime caveats (e.g. behavior that only
differs between the unpackaged and MSIX builds).

## Project layout

Full table in [CONTRIBUTING.md](CONTRIBUTING.md#project-layout). In short:
`main.rs` (app shell + root state + 1 Hz repaint), `ui.rs` (all pages),
`engine.rs` (EasyTier lifecycle + snapshot), `config.rs` (profiles/settings +
`NetworkConfig` mapping), `tray.rs`, `elevate.rs`, `autostart.rs`,
`instance.rs`, `logging.rs`, `dialog.rs`, `build.rs`.

## Code style

- New `.rs` files start with `// SPDX-License-Identifier: GPL-3.0-only`.
- `rustfmt`-clean and zero clippy warnings; prefer fixing over `#[allow(...)]`.
- Match the surrounding code (comment density, naming, idiom). Comments explain
  *why* — especially the non-obvious Win32 / reactor / COM workarounds.
- Keep `unsafe` localized, document its invariant, and free what you allocate.

## windows-reactor gotchas (read before touching the UI)

These will bite you; full detail in
[CONTRIBUTING.md](CONTRIBUTING.md#windows-reactor-read-this-before-touching-the-ui).

- **Pinned by `rev`** in `Cargo.toml` on purpose — don't unpin `windows-reactor`
  or `easytier`; an unpinned re-resolve floats to HEAD and breaks the build.
- **Re-renders only from the root** and skips unchanged-prop subtrees — state
  that must drive a re-render lives in `main::root` and flows down via props.
- **Force a remount with a distinct component _type_, not a key.**
- **A component must never be the bare sole output of another component** — host
  it under a real widget.

## i18n

UI strings are namespaced keys in `locales/en.json` (source of truth, and the
fallback) and `locales/zh.json`, embedded at build time. Add new keys to both.

## Commits & PRs

- Branch off `main`; keep each commit and PR focused on one logical change.
- Write commit messages that explain the *why*.
- **Do not** add a `Co-Authored-By` trailer.
- Confirm `cargo fmt` / `cargo clippy` / `cargo build` are clean before you commit.
