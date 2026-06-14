// SPDX-License-Identifier: GPL-3.0-only
//! Polaris — a WinUI 3 client for the embedded EasyTier mesh VPN.
//!
//! `windows_subsystem = "windows"` keeps the app from spawning a console.
//!
//! `root` owns the app shell, all persistent state, *and* the single 1 Hz
//! repaint timer. The host only ever re-renders from the root and skips any
//! subtree whose props are unchanged, so every piece of state that must drive
//! a re-render — the active page, the network-editor sub-tab, and a `tick`
//! counter for live data — has to live here and flow down through props.
//! (An earlier design kept the timer and sub-tab state inside the child
//! components; because their props never changed, the reconciler skipped them
//! and they silently never updated.)
#![windows_subsystem = "windows"]

mod config;
mod dialog;
mod elevate;
mod engine;
mod i18n;
mod instance;
mod logging;
mod tray;
mod ui;

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use windows_reactor::*;

use config::{LogLevel, Material, Store, Theme};
use engine::Engine;
use ui::{BodyProps, Handles};

fn root(cx: &mut RenderCx, engine: &Engine) -> Element {
    // The engine is created in `main` and shared with the tray thread; memoize
    // a stable clone here so every render sees the same instance.
    let engine = cx.use_memo((), {
        let engine = engine.clone();
        move || engine
    });
    let initial = cx.use_memo((), Store::load);

    let (store, set_store) = cx.use_state(initial);

    // Apply the UI language synchronously, before any child renders this pass.
    // The host re-renders from the root and rebuilds the whole tree, so setting
    // the global here means every `i18n::t()` below already sees the new
    // language; the re-render is driven by `store` changing like any setting.
    let language = store.settings.language;
    i18n::set(language);

    let (tab, set_tab) = cx.use_state(String::from("home"));
    let (pane_open, set_pane_open) = cx.use_state(true);
    // Network-editor sub-tab. Lives here (not in `body_view`) so that clicking
    // a sub-tab actually re-renders the body — see the module docs.
    let (sub_tab, set_sub_tab) = cx.use_state(String::from("general"));

    // 1 Hz repaint clock. Bumping `tick` re-renders the body each second,
    // refreshing peers / stats / connection status.
    let (tick, set_tick) = cx.use_state(0u64);
    let tick_timer = cx.use_ref(None::<DispatcherTimer>);
    {
        let tick_timer = tick_timer.clone();
        cx.use_effect((), move || {
            let counter = Rc::new(Cell::new(0u64));
            let t = DispatcherTimer::new(Duration::from_millis(1000), move || {
                let n = counter.get().wrapping_add(1);
                counter.set(n);
                set_tick.call(n);
            })
            .ok();
            *tick_timer.borrow_mut() = t;
        });
    }

    // Apply theme + backdrop whenever the preference changes.
    let theme = store.settings.theme;
    let material = store.settings.material;
    cx.use_effect((theme, material), move || {
        set_requested_theme(match theme {
            Theme::System => RequestedTheme::Default,
            Theme::Light => RequestedTheme::Light,
            Theme::Dark => RequestedTheme::Dark,
        });
        set_backdrop(match material {
            Material::Mica => Some(Backdrop::Mica),
            Material::MicaAlt => Some(Backdrop::MicaAlt),
            Material::Acrylic => Some(Backdrop::Acrylic),
            Material::Solid => None,
        });
    });

    // Apply the diagnostics log level whenever it (or the enable toggle) changes.
    let diag_enabled = store.settings.diagnostics_enabled;
    let log_level = store.settings.log_level;
    cx.use_effect((diag_enabled, log_level), move || {
        logging::set_level(if diag_enabled {
            log_level
        } else {
            LogLevel::Off
        });
    });

    // Optionally connect the selected network on launch.
    {
        let engine = engine.clone();
        let auto = store.settings.auto_connect;
        // No networks → nothing to auto-connect.
        let conn = store
            .current()
            .map(|p| (p.id.clone(), p.to_network_config()));
        cx.use_effect((), move || {
            if auto && let Some((id, cfg)) = conn {
                engine.start(id, cfg);
            }
        });
    }

    // Install the close/minimize-to-tray window hook once the main window
    // exists. Retried on each repaint tick until it succeeds (the window may
    // not exist on the very first render), then a cheap no-op.
    cx.use_effect(tick, tray::ensure_window_hook);

    // Keep the tray's close/minimize behavior in sync with the settings.
    let close_to_tray = store.settings.close_to_tray;
    let minimize_to_tray = store.settings.minimize_to_tray;
    cx.use_effect((close_to_tray, minimize_to_tray), move || {
        tray::set_close_to_tray(close_to_tray);
        tray::set_minimize_to_tray(minimize_to_tray);
    });

    let handles = Handles {
        engine: engine.clone(),
        set_store: set_store.clone(),
        set_tab: set_tab.clone(),
        set_sub_tab: set_sub_tab.clone(),
    };

    // Stable nav handler + items so the shell never churns the NavigationView.
    let nav_select = cx.use_callback((), {
        let set_tab = set_tab.clone();
        move |t: String| {
            // Item tags carry a language suffix (see `items`); strip it back to
            // the bare tag the page dispatch in `ui::body_view` expects.
            let base = t.split('@').next().unwrap_or("").to_string();
            if !base.is_empty() {
                set_tab.call(base);
            }
        }
    });
    // Keyed on `language` too, so the sidebar labels rebuild on a language switch.
    let items = cx.use_memo((diag_enabled, language), move || {
        // Suffix each tag with the effective language. Relabeling rebuilds the
        // NavigationView's item list, which clears WinUI's selection; the
        // reconciler then skips the *unchanged* selected_tag and the sidebar is
        // left with nothing selected. Folding the language into the tag makes the
        // selected_tag value change too, so it is re-applied and re-matches. The
        // suffix is stripped again in `nav_select`.
        let sfx = if crate::i18n::is_zh() { "zh" } else { "en" };
        let tag = |t: &str| format!("{t}@{sfx}");
        let mut v = vec![
            NavViewItem::new(i18n::t("nav.home"))
                .tag(tag("home"))
                .icon(SymbolGlyph::Home),
            NavViewItem::new(i18n::t("nav.network"))
                .tag(tag("network"))
                .icon(SymbolGlyph::Edit),
            NavViewItem::new(i18n::t("nav.peers"))
                .tag(tag("peers"))
                .icon(SymbolGlyph::People),
            NavViewItem::new(i18n::t("nav.activity"))
                .tag(tag("logs"))
                .icon(SymbolGlyph::Sync),
            NavViewItem::new(i18n::t("nav.settings"))
                .tag(tag("settings"))
                .icon(SymbolGlyph::Setting),
            NavViewItem::new(i18n::t("nav.about"))
                .tag(tag("about"))
                .icon(SymbolGlyph::Help),
        ];
        // Diagnostics page appears between Settings and About when enabled.
        if diag_enabled {
            v.insert(
                5,
                NavViewItem::new(i18n::t("nav.diagnostics"))
                    .tag(tag("diagnostics"))
                    .icon(SymbolGlyph::Find),
            );
        }
        v
    });

    // Dispatch directly to the current page component (see `ui::body_view`).
    // Calling it as a plain function — rather than `component(ui::body_view, …)`
    // — keeps the page component a direct child of the NavigationView widget, so
    // it isn't the bare output of another component (which would remount it, and
    // reset scroll, every tick).
    let body = ui::body_view(&BodyProps {
        handles: handles.clone(),
        store: store.clone(),
        tab: tab.clone(),
        sub_tab: sub_tab.clone(),
        tick,
    });

    // Tall title bar. Connection status + actions now live in the system tray
    // (see `tray`); theme is set on the Settings page.
    //
    // `tall(true)` only sets the window's caption-button strip to 48 px — it
    // does NOT size the TitleBar control. Without custom content the control is
    // shorter than 48 px, so the caption buttons would overhang the page below
    // it; pin the row to 48 px so the page starts cleanly beneath them.
    let title_bar = TitleBar::new("Polaris")
        .subtitle("EasyTier")
        .pane_toggle_button_visible(true)
        .on_pane_toggle_requested({
            let set_pane_open = set_pane_open.clone();
            move || set_pane_open.call(!pane_open)
        })
        .tall(true)
        .min_height(48.0);

    // Must match the language-suffixed item tags built above.
    let nav_suffix = if i18n::is_zh() { "zh" } else { "en" };
    let nav = NavigationView::new(items, body)
        .selected_tag(format!("{}@{}", &*tab, nav_suffix))
        .on_selection_changed(nav_select)
        .pane_display_mode(NavViewPaneDisplayMode::Left)
        .pane_open(pane_open)
        .pane_toggle_button_visible(false)
        .back_button_visible(false)
        .settings_visible(false);

    grid((title_bar.grid_row(0), nav.grid_row(1)))
        .rows([GridLength::Auto, GridLength::Star(1.0)])
        .columns([GridLength::Star(1.0)])
        .into()
}

fn main() -> Result<()> {
    // Give the process a stable Application User Model ID before any window is
    // created. Without it, the taskbar derives an implicit id from the exe path
    // and labels the jump list "polaris_et.exe"; with an explicit id it uses the
    // exe's FileDescription ("Polaris", set in build.rs) instead.
    unsafe {
        let _ = windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID(
            windows::core::w!("Polaris.EasyTier"),
        );
    }

    elevate::init();

    // Single instance: if another Polaris is already running, ask it to surface
    // its window (it may be hidden in the tray) and exit. Peeked *before* any
    // elevation step so re-launching to surface the window never triggers a UAC
    // prompt, even with "always launch as administrator" on.
    //
    // Exception: a process we relaunched to elevate (via `relaunch_elevated`)
    // carries a takeover flag. Its predecessor is on its way out, so instead of
    // bowing out as a duplicate it waits for the single-instance mutex to drop
    // and claims it below — otherwise the old instance exits and nothing is left.
    if elevate::is_relaunch() {
        instance::wait_until_free(Duration::from_secs(5));
    } else if instance::exists() {
        instance::broadcast_show();
        return Ok(());
    }

    // TUN needs administrator rights. We don't force elevation — but if the user
    // opted into "Always launch as administrator" and we're a plain (unpackaged)
    // exe that isn't elevated yet, relaunch via UAC and let that instance take
    // over. A packaged (MSIX) build elevates via its manifest at launch instead,
    // so the runtime relaunch is skipped there.
    if !elevate::is_elevated()
        && !elevate::is_packaged()
        && Store::load().settings.always_admin
        && elevate::relaunch_elevated()
    {
        return Ok(());
    }

    // Claim the single-instance lock in the surviving process. If we lost a
    // launch race, defer to the winner and exit.
    if !instance::acquire() {
        instance::broadcast_show();
        return Ok(());
    }

    // Diagnostics logging: install the global subscriber before the engine so
    // EasyTier's events are captured too. Off unless the user enabled
    // diagnostics; the live level is (re)applied from `root` via a render effect.
    let dprefs = Store::load().settings;
    // Apply the saved language before the tray spawns so its first tooltip/menu
    // are already localized; `root` re-applies it on every render.
    i18n::set(dprefs.language);
    logging::init(if dprefs.diagnostics_enabled {
        dprefs.log_level
    } else {
        LogLevel::Off
    });
    if dprefs.diagnostics_enabled {
        logging::cleanup(dprefs.log_retention_days);
    }

    // The engine owns a background worker thread and is shared (cheap clones,
    // thread-safe) between the UI and the system-tray icon.
    let engine = Engine::new();
    tray::spawn(engine.clone());

    App::new()
        .title("Polaris — EasyTier")
        .inner_size(1200.0, 800.0)
        .inner_constraints(InnerConstraints {
            min_width: Some(860.0),
            min_height: Some(560.0),
            max_width: None,
            max_height: None,
        })
        .backdrop(Backdrop::Mica)
        .render(move |cx| root(cx, &engine))
}
