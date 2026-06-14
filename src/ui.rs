//! All pages and shared view helpers for the Polaris UI.
//!
//! The shell (title bar + NavigationView) lives in `main.rs`. This file owns
//! every page. The network editor is a [`Pivot`] with five tabs organised by
//! *purpose* rather than EasyTier's flat flag list:
//!
//! 1. **General**   — identity, device, connection, addressing.
//! 2. **Security**  — encryption, magic DNS, private mode, relay whitelist.
//! 3. **Routing**   — latency preference, subnet proxy, manual routes, exit nodes.
//! 4. **Proxies**   — SOCKS5, VPN portal (with live WireGuard config viewer),
//!    port-forward table.
//! 5. **Advanced**  — transport / P2P / NAT toggles, listeners, MTU / BPS limit.
//!
//! Dense flag grids use checkboxes + `(?)` tooltip icons (three-up grid). The
//! few headline switches use single-column rows where the help text gets the
//! full card width — so nothing truncates.

use windows_reactor::*;

use crate::config::*;
use crate::engine::*;
use crate::i18n::{Language, t, tn};

// ─────────────────────────── Context & body view ──────────────────────────

/// Everything a page needs.
pub struct PageCtx {
    pub store: Store,
    pub set_store: SetState<Store>,
    pub set_tab: SetState<String>,
    /// Current network-editor sub-tab. This state lives in the root component
    /// (see `main::root`), *not* in `body_view`: the host only ever re-renders
    /// from the root and skips any subtree whose props are unchanged, so state
    /// that must drive a re-render of this body has to travel down via props —
    /// otherwise the outer `NavigationView` compares equal and the body is
    /// skipped (which is why the sub-tabs silently did nothing before).
    pub sub_tab: String,
    /// Stable selection handler for the editor's sub-tab `NavigationView`.
    pub on_sub_change: Callback<String>,
    pub engine: Engine,
    pub snap: Snapshot,
}

/// Long-lived handles shared with child components.
#[derive(Clone)]
pub struct Handles {
    pub engine: Engine,
    pub set_store: SetState<Store>,
    pub set_tab: SetState<String>,
    pub set_sub_tab: SetState<String>,
}

impl PartialEq for Handles {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

/// Props for the page body component. `tick` is bumped once per second by the
/// root's repaint timer; including it here is what makes live data (peers,
/// stats, connection status) actually refresh — without a prop that changes,
/// the reconciler skips this entire subtree.
#[derive(Clone, PartialEq)]
pub struct BodyProps {
    pub handles: Handles,
    pub store: Store,
    pub tab: String,
    pub sub_tab: String,
    pub tick: u64,
}

/// The page body. Re-renders whenever the root re-renders with new props —
/// once per second (via `tick`) and immediately on any navigation or edit.
/// Dispatch the current tab to its page component.
///
/// Called as a PLAIN FUNCTION from `main::root` (not wrapped in `component(...)`)
/// so the returned page component is hosted directly by the NavigationView
/// *widget*. A component must never be the bare sole output of another
/// component: their roots collide on a single control id, and the reactor then
/// remounts the inner one on EVERY render — which destroyed and recreated the
/// page each 1 Hz tick, resetting scroll position.
///
/// Each tab maps to a DISTINCT component type so the reconciler does a clean
/// remount on tab switch (it diffs same-type children in place, which otherwise
/// leaks stale controls across structurally different pages — e.g. a Settings
/// toggle bleeding into About). Keys can't substitute here: the reactor exposes
/// no key setter for widgets and drops Group keys on flatten.
pub fn body_view(props: &BodyProps) -> Element {
    // Settings and Network host translated ComboBoxes. WinUI clears a combo's
    // selection when its item list is replaced, and the reconciler does not
    // re-apply an unchanged SelectedIndex — so relabeling them on a language
    // switch would leave them blank. Encode the language in the component *type*
    // (a distinct monomorphization → distinct `component_type_id`) so those two
    // pages REMOUNT with fresh combos when it flips; the rest just re-render.
    let zh = crate::i18n::is_zh();
    match props.tab.as_str() {
        "network" if zh => component(network_view::<true>, props.clone()),
        "network" => component(network_view::<false>, props.clone()),
        "peers" => component(peers_view, props.clone()),
        "logs" => component(logs_view, props.clone()),
        "settings" if zh => component(settings_view::<true>, props.clone()),
        "settings" => component(settings_view::<false>, props.clone()),
        "about" => component(about_view, props.clone()),
        "diagnostics" => component(diagnostics_view, props.clone()),
        _ => component(home_view, props.clone()),
    }
}

/// Build the per-render [`PageCtx`] from body props. Shared by every page
/// component so they all wire context the same way.
fn page_ctx(props: &BodyProps, cx: &mut RenderCx) -> PageCtx {
    // Stable selection handler for the editor's sub-tab NavigationView. Memoized
    // so its binding doesn't churn across the per-second repaint (a fresh
    // closure each render makes the NavigationView drop selection clicks).
    let on_sub_change = cx.use_callback((), {
        let set = props.handles.set_sub_tab.clone();
        move |t: String| {
            if !t.is_empty() {
                set.call(t);
            }
        }
    });

    PageCtx {
        store: props.store.clone(),
        set_store: props.handles.set_store.clone(),
        set_tab: props.handles.set_tab.clone(),
        sub_tab: props.sub_tab.clone(),
        on_sub_change,
        engine: props.handles.engine.clone(),
        snap: props.handles.engine.snapshot(),
    }
}

// One distinct component type per page so tab switches remount cleanly — see
// `body_view`. Each rebuilds the shared `PageCtx` and defers to its page fn.
fn home_view(props: &BodyProps, cx: &mut RenderCx) -> Element {
    home_page(&page_ctx(props, cx))
}
// `ZH` is a render-only discriminant: it makes the Chinese and English
// instantiations distinct component types so the page remounts on a language
// switch (see `body_view`). It is intentionally unused in the body.
fn network_view<const ZH: bool>(props: &BodyProps, cx: &mut RenderCx) -> Element {
    network_page(&page_ctx(props, cx))
}
fn peers_view(props: &BodyProps, cx: &mut RenderCx) -> Element {
    peers_page(&page_ctx(props, cx))
}
fn logs_view(props: &BodyProps, cx: &mut RenderCx) -> Element {
    logs_page(&page_ctx(props, cx))
}
fn settings_view<const ZH: bool>(props: &BodyProps, cx: &mut RenderCx) -> Element {
    settings_page(&page_ctx(props, cx))
}
fn about_view(_props: &BodyProps, _cx: &mut RenderCx) -> Element {
    about_page()
}
fn diagnostics_view(props: &BodyProps, cx: &mut RenderCx) -> Element {
    diagnostics_page(&page_ctx(props, cx))
}

// ─────────────────────────────── Edit macros ──────────────────────────────

/// Edit the *currently selected* profile.
macro_rules! on_edit {
    ($ctx:expr, |$p:ident, $v:ident : $t:ty| $body:expr) => {{
        let set = $ctx.set_store.clone();
        let store = $ctx.store.clone();
        move |$v: $t| {
            let mut s = store.clone();
            let i = s.selected.min(s.profiles.len() - 1);
            {
                let $p = &mut s.profiles[i];
                $body;
            }
            s.save();
            set.call(s);
        }
    }};
}

/// Edit global settings.
macro_rules! on_setting {
    ($ctx:expr, |$st:ident, $v:ident : $t:ty| $body:expr) => {{
        let set = $ctx.set_store.clone();
        let store = $ctx.store.clone();
        move |$v: $t| {
            let mut s = store.clone();
            {
                let $st = &mut s.settings;
                $body;
            }
            s.save();
            set.call(s);
        }
    }};
}

// ───────────────────────────── shared widgets ─────────────────────────────

fn pad(l: f64, t: f64, r: f64, b: f64) -> Thickness {
    Thickness {
        left: l,
        top: t,
        right: r,
        bottom: b,
    }
}

/// Page chrome: a heading + subtitle over a scrollable, padded body.
fn page(title: &str, subtitle: &str, mut children: Vec<Element>) -> Element {
    let header = vstack((
        text_block(t(title)).font_size(28.0).bold(),
        text_block(t(subtitle)).font_size(13.0).opacity(0.6),
    ))
    .spacing(2.0)
    .into();

    let mut col = vec![header];
    col.append(&mut children);

    border(
        scroll_view(
            vstack(col)
                .spacing(18.0)
                .horizontal_alignment(HorizontalAlignment::Stretch),
        )
        .content_orientation(ScrollViewContentOrientation::Vertical),
    )
    .padding(pad(32.0, 22.0, 32.0, 28.0))
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .into()
}

/// A titled surface card. `title` may be empty for a chromeless card.
fn card(title: &str, body: Element) -> Element {
    // `title` is an i18n key, or empty for a chromeless card.
    let mut col: Vec<Element> = Vec::new();
    if !title.is_empty() {
        col.push(text_block(t(title)).font_size(15.0).semibold().into());
    }
    col.push(body);

    border(vstack(col).spacing(14.0))
        .background(ThemeRef::CardBackground)
        .corner_radius(8.0)
        .padding(Thickness::uniform(20.0))
        .into()
}

/// Big value over a dim label.
fn stat(label: &str, value: impl Into<String>) -> Element {
    vstack((
        text_block(value.into()).font_size(20.0).semibold(),
        text_block(t(label)).font_size(11.0).opacity(0.6),
    ))
    .spacing(2.0)
    .min_width(86.0)
    .into()
}

fn color_for(s: Status) -> Color {
    match s {
        Status::Connected => Color::rgb(38, 194, 129),
        Status::Connecting => Color::rgb(240, 180, 41),
        Status::Error => Color::rgb(232, 78, 65),
        Status::Disconnected => Color::rgb(140, 148, 156),
    }
}

fn dot(s: Status, size: f64) -> Element {
    border(text_block(""))
        .background(color_for(s))
        .width(size)
        .height(size)
        .corner_radius(size / 2.0)
        .into()
}

fn or_dash(s: &str) -> String {
    if s.is_empty() {
        "—".to_string()
    } else {
        s.to_string()
    }
}

fn cell(text: String, w: f64, bold: bool, dim: bool) -> Element {
    // Tooltip carries the full value so width-clipped cells are still readable.
    let t = text_block(text.clone())
        .font_size(13.0)
        .width(w)
        .tooltip(text);
    let t = if bold { t.semibold() } else { t };
    let t = if dim { t.opacity(0.6) } else { t };
    t.into()
}

/// A table cell whose value can be clicked to copy (and hovered for the full
/// text). A plain `text_block` with `on_tapped` — NOT a button — so it stays a
/// normal left-aligned label and lines up with the other columns (the
/// framework's buttons center their content and add padding, breaking the grid).
fn copy_cell(text: String, w: f64) -> Element {
    if text.is_empty() || text == "—" {
        return cell(text, w, false, true);
    }
    text_block(text.clone())
        .font_size(13.0)
        .width(w)
        .tooltip(format!("{text}  —  click to copy"))
        .on_tapped(move || {
            crate::dialog::write_clipboard_text(&text);
        })
        .into()
}

/// Section divider used inside cards — a small horizontal line with margin.
fn divider() -> Element {
    border(text_block(""))
        .background(ThemeRef::CardStroke)
        .height(1.0)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .into()
}

/// Headline toggle row: switch + (label / help) stacked, full card width so
/// the help text never truncates.
///
/// Blank `on_content`/`off_content` collapses the WinUI switch's reserved
/// horizontal space (~120 px for the default "On"/"Off" labels) down to just
/// the slider — without this the label floats half a card away.
fn switch_row(label: &str, help: &str, value: bool, on_change: impl Fn(bool) + 'static) -> Element {
    hstack((
        ToggleSwitch::new(value)
            .on_content("")
            .off_content("")
            .on_changed(on_change),
        vstack((
            text_block(t(label)).font_size(13.0).semibold(),
            text_block(t(help)).font_size(11.0).opacity(0.6).wrap(),
        ))
        .spacing(2.0)
        .horizontal_alignment(HorizontalAlignment::Stretch),
    ))
    .spacing(12.0)
    .vertical_alignment(VerticalAlignment::Top)
    .into()
}

/// Dense flag cell: checkbox + label + an `i` tooltip glyph for the help.
fn flag(label: &str, help: &str, value: bool, on_change: impl Fn(bool) + 'static) -> Element {
    hstack((
        CheckBox::new(value).label(t(label)).on_changed(on_change),
        text_block("\u{E946}")
            .font_family("Segoe MDL2 Assets")
            .font_size(13.0)
            .opacity(0.5)
            .tooltip(t(help)),
    ))
    .spacing(6.0)
    .vertical_alignment(VerticalAlignment::Center)
    .into()
}

/// Three-column grid of `flag()` cells. Wraps with even row spacing.
fn flag_grid(flags: Vec<Element>) -> Element {
    let mut rows: Vec<Element> = Vec::new();
    let mut chunk: Vec<Element> = Vec::with_capacity(3);
    for f in flags {
        chunk.push(f);
        if chunk.len() == 3 {
            rows.push(flag_row(std::mem::take(&mut chunk)));
        }
    }
    if !chunk.is_empty() {
        rows.push(flag_row(chunk));
    }
    vstack(rows).spacing(10.0).into()
}

fn flag_row(items: Vec<Element>) -> Element {
    let cells: Vec<Element> = items
        .into_iter()
        .enumerate()
        .map(|(i, e)| e.grid_column(i as i32))
        .collect();
    grid(cells)
        .columns([
            GridLength::Star(1.0),
            GridLength::Star(1.0),
            GridLength::Star(1.0),
        ])
        .column_spacing(16.0)
        .into()
}

fn live_warning() -> Element {
    InfoBar::new(t("network.live_title"))
        .message(t("network.live_message"))
        .informational()
        .is_closable(false)
        .into()
}

/// A clickable text link that opens `url` in the default browser.
fn link(label: &str, url: &str) -> Element {
    HyperlinkButton::new(t(label))
        .navigate_uri(url.to_string())
        .into()
}

// ───────────────────────────────── Home ───────────────────────────────────

pub fn home_page(ctx: &PageCtx) -> Element {
    let snap = &ctx.snap;

    let summary = card(
        "",
        hstack((
            stat("home.stat_connected", format!("{}", snap.connected_count())),
            stat(
                "home.stat_networks",
                format!("{}", ctx.store.profiles.len()),
            ),
            stat("home.stat_peers", format!("{}", snap.total_peers())),
            stat("home.stat_downloaded", human_bytes(snap.total_rx())),
            stat("home.stat_uploaded", human_bytes(snap.total_tx())),
        ))
        .spacing(32.0)
        .into(),
    );

    // Network-list toolbar — Add / Import live with the list of networks they
    // affect, not in app-level Settings.
    let toolbar = card(
        "",
        grid((
            text_block(t("home.saved_networks"))
                .font_size(13.0)
                .opacity(0.7)
                .vertical_alignment(VerticalAlignment::Center)
                .grid_column(0),
            hstack((
                add_network_button(ctx),
                import_button(ctx),
                paste_button(ctx),
                backup_button(ctx),
                restore_button(ctx),
            ))
            .spacing(10.0)
            .grid_column(1),
        ))
        .columns([GridLength::Star(1.0), GridLength::Auto])
        .into(),
    );

    let mut children: Vec<Element> = Vec::new();
    if !crate::elevate::is_elevated() {
        children.push(tun_notice());
    }
    children.push(summary);
    children.push(toolbar);
    for idx in 0..ctx.store.profiles.len() {
        children.push(network_card(ctx, idx));
    }

    page("home.title", "home.subtitle", children)
}

/// Shown when running without admin: TUN is off, proxies still work.
fn tun_notice() -> Element {
    InfoBar::new(t("home.tun_notice_title"))
        .message(t("home.tun_notice_message"))
        .informational()
        .is_closable(false)
        .into()
}

fn network_card(ctx: &PageCtx, idx: usize) -> Element {
    let prof = ctx.store.profiles[idx].clone();
    let net = ctx.snap.net(&prof.id).cloned();
    let status = net.as_ref().map_or(Status::Disconnected, |n| n.status);
    let live = matches!(status, Status::Connected | Status::Connecting);

    let ip = net
        .as_ref()
        .map(|n| or_dash(&n.virtual_ip))
        .unwrap_or_else(|| "—".to_string());
    let peers = net.as_ref().map_or(0, |n| n.peers.len());
    let rx = net.as_ref().map(|n| n.rx_bytes).unwrap_or(0);
    let tx = net.as_ref().map(|n| n.tx_bytes).unwrap_or(0);

    let action: Element = if live {
        let e = ctx.engine.clone();
        let id = prof.id.clone();
        button(t("common.disconnect"))
            .on_click(move || e.stop(id.clone()))
            .min_width(128.0)
            .into()
    } else {
        let e = ctx.engine.clone();
        let id = prof.id.clone();
        let cfg = prof.to_network_config();
        button(t("common.connect"))
            .accent()
            .on_click(move || e.start(id.clone(), cfg.clone()))
            .min_width(128.0)
            .into()
    };

    let edit = {
        let set = ctx.set_store.clone();
        let set_tab = ctx.set_tab.clone();
        let store = ctx.store.clone();
        button(t("common.edit"))
            .subtle()
            .icon(SymbolGlyph::Edit)
            .on_click(move || {
                let mut s = store.clone();
                s.selected = idx;
                s.save();
                set.call(s);
                set_tab.call("network".to_string());
            })
    };

    let del = delete_button(ctx, idx, prof.id.clone());
    let export = export_button(&prof);

    let top = grid((
        hstack((
            dot(status, 12.0),
            text_block(prof.name.clone()).font_size(16.0).semibold(),
        ))
        .spacing(10.0)
        .vertical_alignment(VerticalAlignment::Center)
        .grid_column(0),
        hstack((edit, export, del, action))
            .spacing(10.0)
            .vertical_alignment(VerticalAlignment::Center)
            .grid_column(1),
    ))
    .columns([GridLength::Star(1.0), GridLength::Auto]);

    let info = hstack((
        text_block(t(status.label()))
            .font_size(12.0)
            .foreground(color_for(status)),
        text_block(format!(
            "{}  {}",
            t("home.field_network"),
            prof.network_name
        ))
        .font_size(12.0)
        .opacity(0.65),
        text_block(format!("IP  {ip}"))
            .font_size(12.0)
            .opacity(0.65),
        text_block(format!("{}  {peers}", t("home.field_peers")))
            .font_size(12.0)
            .opacity(0.65),
        text_block(format!("↓ {}  ↑ {}", human_bytes(rx), human_bytes(tx)))
            .font_size(12.0)
            .opacity(0.65),
    ))
    .spacing(22.0);

    let mut body: Vec<Element> = vec![top.into(), info.into()];
    if let Some(err) = net.as_ref().and_then(|n| n.error.clone()) {
        body.push(
            InfoBar::new(t("home.connection_problem"))
                .message(err)
                .error()
                .is_closable(false)
                .into(),
        );
    }

    border(vstack(body).spacing(12.0))
        .background(ThemeRef::CardBackground)
        .corner_radius(8.0)
        .padding(pad(20.0, 16.0, 20.0, 16.0))
        .into()
}

// ──────────────────────────────── Network ─────────────────────────────────

pub fn network_page(ctx: &PageCtx) -> Element {
    let p = ctx.store.current().clone();
    let live = ctx.snap.is_live(&p.id);
    let sub_tab = ctx.sub_tab.as_str();

    // Compact toolbar: status dot + name + network picker (only shown when >1
    // network exists) + Connect/Disconnect button. No big card — saves ~120 px.
    let mut toolbar_items: Vec<Element> = vec![
        hstack((
            dot(
                ctx.snap
                    .net(&p.id)
                    .map_or(Status::Disconnected, |n| n.status),
                12.0,
            ),
            text_block(p.name.clone()).font_size(16.0).semibold(),
        ))
        .spacing(10.0)
        .vertical_alignment(VerticalAlignment::Center)
        .grid_column(0)
        .into(),
    ];
    if ctx.store.profiles.len() > 1 {
        let names: Vec<String> = ctx.store.profiles.iter().map(|x| x.name.clone()).collect();
        toolbar_items.push(
            ComboBox::new(names)
                .selected_index(ctx.store.selected as i32)
                .min_width(220.0)
                .on_selection_changed({
                    let set = ctx.set_store.clone();
                    let store = ctx.store.clone();
                    move |i: i32| {
                        // Ignore WinUI's spurious -1, fired when the item list is
                        // rebuilt (e.g. a profile rename) — otherwise it would
                        // snap the selection back to the first network.
                        if i < 0 {
                            return;
                        }
                        let mut s = store.clone();
                        s.selected = (i as usize).min(s.profiles.len() - 1);
                        s.save();
                        set.call(s);
                    }
                })
                .grid_column(1)
                .into(),
        );
    } else {
        toolbar_items.push(text_block("").grid_column(1).into());
    }
    let action: Element = {
        let e = ctx.engine.clone();
        let id = p.id.clone();
        let cfg = p.to_network_config();
        if live {
            button(t("common.disconnect"))
                .on_click(move || e.stop(id.clone()))
                .min_width(140.0)
                .grid_column(2)
                .into()
        } else {
            button(t("common.connect"))
                .accent()
                .on_click(move || e.start(id.clone(), cfg.clone()))
                .min_width(140.0)
                .grid_column(2)
                .into()
        }
    };
    toolbar_items.push(action);

    let toolbar = grid(toolbar_items)
        .columns([GridLength::Star(1.0), GridLength::Auto, GridLength::Auto])
        .column_spacing(14.0);

    let panel: Element = match sub_tab {
        "security" => security_panel(ctx, &p),
        "routing" => routing_panel(ctx, &p),
        "proxies" => proxies_panel(ctx, &p),
        "advanced" => advanced_panel(ctx, &p),
        _ => general_panel(ctx, &p),
    };

    // Per-network editor tabs as a top-pane NavigationView. Selection works
    // reliably because `sub_tab` lives in the root and flows down via props (see
    // `BodyProps`), and `on_sub_change` is memoized so the binding doesn't churn
    // on the per-second repaint.
    let panel_body =
        border(scroll_view(panel).content_orientation(ScrollViewContentOrientation::Vertical))
            .padding(pad(32.0, 16.0, 32.0, 28.0))
            .horizontal_alignment(HorizontalAlignment::Stretch);

    let items = vec![
        NavViewItem::new(t("network.tab_general")).tag("general"),
        NavViewItem::new(t("network.tab_security")).tag("security"),
        NavViewItem::new(t("network.tab_routing")).tag("routing"),
        NavViewItem::new(t("network.tab_proxies")).tag("proxies"),
        NavViewItem::new(t("network.tab_advanced")).tag("advanced"),
    ];
    let nav = NavigationView::new(items, panel_body)
        .pane_display_mode(NavViewPaneDisplayMode::Top)
        .selected_tag(sub_tab)
        .on_selection_changed(ctx.on_sub_change.clone())
        .settings_visible(false)
        .back_button_visible(false)
        .pane_toggle_button_visible(false);

    // Chrome (title + toolbar + optional live warning) stays sticky above the
    // tab nav so the network identity and Connect button stay accessible.
    let mut chrome_items: Vec<Element> = vec![
        vstack((
            text_block(t("network.title")).font_size(28.0).bold(),
            text_block(p.name.clone()).font_size(13.0).opacity(0.6),
        ))
        .spacing(2.0)
        .into(),
        toolbar.into(),
    ];
    if live {
        chrome_items.push(live_warning());
    }
    let chrome = border(vstack(chrome_items).spacing(16.0)).padding(pad(32.0, 22.0, 32.0, 12.0));

    grid((chrome.grid_row(0), nav.grid_row(1)))
        .rows([GridLength::Auto, GridLength::Star(1.0)])
        .columns([GridLength::Star(1.0)])
        .into()
}

fn split_lines(v: &str) -> Vec<String> {
    v.split('\n').map(|l| l.trim().to_string()).collect()
}

fn sanitize(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    if s.is_empty() {
        "network".to_string()
    } else {
        s
    }
}

/// Force `path` to end in `.ext` (the chosen format's real extension), so a file
/// saved through a filter the dialog didn't auto-extension still gets the right
/// suffix.
fn ensure_ext(mut path: std::path::PathBuf, ext: &str) -> std::path::PathBuf {
    let ok = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case(ext))
        .unwrap_or(false);
    if !ok {
        path.set_extension(ext);
    }
    path
}

// ─────────────────────────── Network → General ────────────────────────────

fn general_panel(ctx: &PageCtx, p: &Profile) -> Element {
    let profile_card = card(
        "network.general.profile_card",
        vstack((
            text_box(p.name.clone())
                .header(t("network.general.profile_name"))
                .placeholder(t("network.general.profile_name_placeholder"))
                .on_changed(on_edit!(ctx, |p, v: String| p.name = v)),
            hstack((duplicate_button(ctx), export_button(p))).spacing(10.0),
        ))
        .spacing(14.0)
        .into(),
    );

    let identity = card(
        "network.general.identity",
        vstack((
            text_box(p.network_name.clone())
                .header(t("network.general.network_name"))
                .placeholder(t("network.general.network_name_placeholder"))
                .on_changed(on_edit!(ctx, |p, v: String| p.network_name = v)),
            PasswordBox::new()
                .value(p.network_secret.clone())
                .header(t("network.general.network_secret"))
                .placeholder(t("network.general.network_secret_placeholder"))
                .on_changed(on_edit!(ctx, |p, v: String| p.network_secret = v)),
        ))
        .spacing(14.0)
        .into(),
    );

    let device = card(
        "network.general.device",
        vstack((
            text_box(p.hostname.clone())
                .header(t("network.general.display_name"))
                .placeholder(t("network.general.display_name_placeholder"))
                .on_changed(on_edit!(ctx, |p, v: String| p.hostname = v)),
            text_box(p.dev_name.clone())
                .header(t("network.general.tun_name"))
                .placeholder(t("network.general.tun_name_placeholder"))
                .on_changed(on_edit!(ctx, |p, v: String| p.dev_name = v)),
        ))
        .spacing(14.0)
        .into(),
    );

    let method_labels: Vec<String> = JoinMethod::ALL
        .iter()
        .map(|m| m.label().to_string())
        .collect();
    let mut conn: Vec<Element> = vec![
        ComboBox::new(method_labels)
            .header(t("network.general.how_to_join"))
            .selected_index(p.join_method.index())
            .on_selection_changed(
                on_edit!(ctx, |p, v: i32| p.join_method = JoinMethod::from_index(v))
            )
            .into(),
    ];
    match p.join_method {
        JoinMethod::PublicServer => conn.push(
            text_box(p.public_server.clone())
                .header(t("network.general.server_address"))
                .placeholder("tcp://public.easytier.cn:11010")
                .on_changed(on_edit!(ctx, |p, v: String| p.public_server = v))
                .into(),
        ),
        JoinMethod::Manual => conn.push(
            text_box(p.peers.join("\n"))
                .header(t("network.general.peer_addresses"))
                .placeholder("tcp://192.0.2.10:11010")
                .multiline()
                .height(110.0)
                .on_changed(on_edit!(ctx, |p, v: String| p.peers = split_lines(&v)))
                .into(),
        ),
        JoinMethod::Standalone => conn.push(
            text_block(t("network.general.standalone_hint"))
                .font_size(12.0)
                .opacity(0.7)
                .wrap()
                .into(),
        ),
    }
    let connection = card(
        "network.general.connection",
        vstack(conn).spacing(14.0).into(),
    );

    let mut addr: Vec<Element> = vec![switch_row(
        "network.general.dhcp",
        "network.general.dhcp_help",
        p.dhcp,
        on_edit!(ctx, |p, v: bool| p.dhcp = v),
    )];
    if !p.dhcp {
        addr.push(
            grid((
                text_box(p.virtual_ipv4.clone())
                    .header(t("network.general.virtual_ipv4"))
                    .placeholder("10.126.126.1")
                    .on_changed(on_edit!(ctx, |p, v: String| p.virtual_ipv4 = v))
                    .grid_column(0),
                NumberBox::new(p.network_length as f64)
                    .header(t("network.general.prefix_length"))
                    .range(1.0, 32.0)
                    .on_value_changed(on_edit!(ctx, |p, v: f64| {
                        p.network_length = v.clamp(1.0, 32.0) as u8
                    }))
                    .grid_column(1),
            ))
            .columns([GridLength::Star(3.0), GridLength::Star(1.0)])
            .column_spacing(12.0)
            .into(),
        );
    }
    let addressing = card(
        "network.general.addressing",
        vstack(addr).spacing(14.0).into(),
    );

    vstack((profile_card, identity, device, connection, addressing))
        .spacing(18.0)
        .into()
}

// ─────────────────────── Network-list action buttons ──────────────────────

fn add_network_button(ctx: &PageCtx) -> Button {
    let set = ctx.set_store.clone();
    let store = ctx.store.clone();
    button(t("common.add_network"))
        .accent()
        .icon(SymbolGlyph::Add)
        .on_click(move || {
            let mut s = store.clone();
            let p = Profile {
                name: format!("Network {}", s.profiles.len() + 1),
                ..Profile::default()
            };
            s.profiles.push(p);
            s.selected = s.profiles.len() - 1;
            s.save();
            set.call(s);
        })
}

fn import_button(ctx: &PageCtx) -> Button {
    let set = ctx.set_store.clone();
    let store = ctx.store.clone();
    button(t("common.import"))
        .icon(SymbolGlyph::Download)
        .on_click(move || {
            let f_cfg = t("dialog.et_configs");
            let f_all = t("dialog.all_files");
            if let Some(path) = crate::dialog::open_file(&[
                (f_cfg.as_str(), "*.toml;*.json"),
                (f_all.as_str(), "*.*"),
            ]) && let Ok(profiles) = import_profiles(&path)
                && !profiles.is_empty()
            {
                let mut s = store.clone();
                let first = s.profiles.len();
                s.profiles.extend(profiles);
                s.selected = first;
                s.save();
                set.call(s);
            }
        })
}

/// Import network(s) from EasyTier config text on the clipboard (TOML or JSON).
fn paste_button(ctx: &PageCtx) -> Button {
    let set = ctx.set_store.clone();
    let store = ctx.store.clone();
    button(t("common.paste_config"))
        .icon(SymbolGlyph::Paste)
        .on_click(move || {
            let Some(text) = crate::dialog::read_clipboard_text() else {
                return;
            };
            if let Ok(profiles) = parse_profiles_from_text(&text)
                && !profiles.is_empty()
            {
                let mut s = store.clone();
                let first = s.profiles.len();
                s.profiles.extend(profiles);
                s.selected = first;
                s.save();
                set.call(s);
            }
        })
}

fn duplicate_button(ctx: &PageCtx) -> Button {
    let set = ctx.set_store.clone();
    let store = ctx.store.clone();
    button(t("common.duplicate"))
        .icon(SymbolGlyph::Copy)
        .on_click(move || {
            let mut s = store.clone();
            let mut p = s.current().clone();
            p.id = crate::config::Profile::default().id;
            p.name = format!("{} (copy)", p.name);
            s.profiles.push(p);
            s.selected = s.profiles.len() - 1;
            s.save();
            set.call(s);
        })
}

/// Export one network, letting the user pick the format in the save dialog:
/// Polaris-native (lossless), EasyTier TOML, or EasyTier NetworkConfig JSON.
fn export_button(p: &Profile) -> Button {
    let p = p.clone();
    button(t("common.export"))
        .subtle()
        .icon(SymbolGlyph::Upload)
        .on_click(move || {
            // The two *.json formats can't be told apart by extension, so use the
            // save dialog's 1-based file-type index (1 Polaris, 2 TOML, 3 EasyTier).
            let default = format!("{}.json", sanitize(&p.name));
            let f_polaris = t("dialog.polaris_config");
            let f_toml = t("dialog.et_config");
            let f_json = t("dialog.network_config_json");
            if let Some((path, idx)) = crate::dialog::save_file_typed(
                &default,
                &[
                    (f_polaris.as_str(), "*.json"),
                    (f_toml.as_str(), "*.toml"),
                    (f_json.as_str(), "*.json"),
                ],
            ) {
                let (content, ext) = match idx {
                    2 => (profile_to_toml(&p), "toml"),
                    3 => (profile_to_json(&p), "json"),
                    _ => (profile_to_polaris_json(&p), "json"),
                };
                if let Ok(text) = content {
                    let _ = std::fs::write(ensure_ext(path, ext), text);
                }
            }
        })
}

/// Back up the whole store — every network plus the app settings — to one
/// Polaris JSON file. Restore with [`restore_button`].
fn backup_button(ctx: &PageCtx) -> Button {
    let store = ctx.store.clone();
    button(t("common.backup"))
        .icon(SymbolGlyph::Save)
        .on_click(move || {
            let f = t("dialog.polaris_backup");
            if let Some(path) =
                crate::dialog::save_file("polaris-backup.json", &[(f.as_str(), "*.json")])
                && let Ok(text) = store_to_backup_json(&store)
            {
                let _ = std::fs::write(ensure_ext(path, "json"), text);
            }
        })
}

/// Restore a full backup, replacing all networks + settings. Stops everything
/// currently running first (restored profiles are given fresh ids).
fn restore_button(ctx: &PageCtx) -> Button {
    let set = ctx.set_store.clone();
    let store = ctx.store.clone();
    let engine = ctx.engine.clone();
    button(t("common.restore"))
        .icon(SymbolGlyph::Sync)
        .on_click(move || {
            let f = t("dialog.polaris_backup");
            let f_all = t("dialog.all_files");
            if let Some(path) =
                crate::dialog::open_file(&[(f.as_str(), "*.json"), (f_all.as_str(), "*.*")])
                && let Ok(new_store) = parse_backup(&path)
            {
                for p in &store.profiles {
                    engine.stop(p.id.clone());
                }
                new_store.save();
                set.call(new_store);
            }
        })
}

/// Delete the profile at `idx`. Disabled when only one network remains.
fn delete_button(ctx: &PageCtx, idx: usize, prof_id: String) -> Button {
    let can_delete = ctx.store.profiles.len() > 1;
    let set = ctx.set_store.clone();
    let store = ctx.store.clone();
    let engine = ctx.engine.clone();
    button(t("common.delete"))
        .subtle()
        .icon(SymbolGlyph::Delete)
        .enabled(can_delete)
        .on_click(move || {
            let mut s = store.clone();
            if s.profiles.len() > 1 && idx < s.profiles.len() {
                engine.stop(prof_id.clone());
                s.profiles.remove(idx);
                s.selected = s.selected.min(s.profiles.len() - 1);
                s.save();
                set.call(s);
            }
        })
}

// ─────────────────────────── Network → Security ───────────────────────────

fn security_panel(ctx: &PageCtx, p: &Profile) -> Element {
    let crypto = card(
        "network.security.encryption",
        switch_row(
            "network.security.encrypt",
            "network.security.encrypt_help",
            p.enable_encryption,
            on_edit!(ctx, |p, v: bool| p.enable_encryption = v),
        ),
    );

    let access = card(
        "network.security.access_control",
        vstack((
            switch_row(
                "network.security.private_mode",
                "network.security.private_mode_help",
                p.enable_private_mode,
                on_edit!(ctx, |p, v: bool| p.enable_private_mode = v),
            ),
            divider(),
            switch_row(
                "network.security.relay_whitelist",
                "network.security.relay_whitelist_help",
                p.enable_relay_network_whitelist,
                on_edit!(ctx, |p, v: bool| p.enable_relay_network_whitelist = v),
            ),
            // Whitelist editor is always present (greyed by EasyTier semantics when toggle is
            // off, but the field stays so the user can keep typing).
            text_box(p.relay_network_whitelist.join("\n"))
                .header(t("network.security.allowed_networks"))
                .placeholder("home-lab\nstaging")
                .multiline()
                .height(80.0)
                .enabled(p.enable_relay_network_whitelist)
                .on_changed(on_edit!(ctx, |p, v: String| {
                    p.relay_network_whitelist = split_lines(&v)
                })),
        ))
        .spacing(14.0)
        .into(),
    );

    let dns = card(
        "network.security.dns",
        switch_row(
            "network.security.magic_dns",
            "network.security.magic_dns_help",
            p.enable_magic_dns,
            on_edit!(ctx, |p, v: bool| p.enable_magic_dns = v),
        ),
    );

    vstack((crypto, access, dns)).spacing(18.0).into()
}

// ─────────────────────────── Network → Routing ────────────────────────────

fn routing_panel(ctx: &PageCtx, p: &Profile) -> Element {
    let preference = card(
        "network.routing.path_selection",
        switch_row(
            "network.routing.latency_first",
            "network.routing.latency_first_help",
            p.latency_first,
            on_edit!(ctx, |p, v: bool| p.latency_first = v),
        ),
    );

    let subnet_proxy = card(
        "network.routing.subnet_proxy",
        vstack((
            text_block(t("network.routing.subnet_proxy_text"))
                .font_size(12.0)
                .opacity(0.7)
                .wrap(),
            text_box(p.proxy_cidrs.join("\n"))
                .header(t("network.routing.shared_subnets"))
                .placeholder("192.168.1.0/24")
                .multiline()
                .height(80.0)
                .on_changed(on_edit!(ctx, |p, v: String| p.proxy_cidrs = split_lines(&v))),
        ))
        .spacing(10.0)
        .into(),
    );

    let manual_routes = card(
        "network.routing.manual_routes",
        vstack((
            switch_row(
                "network.routing.announce_routes",
                "network.routing.announce_routes_help",
                p.enable_manual_routes,
                on_edit!(ctx, |p, v: bool| p.enable_manual_routes = v),
            ),
            text_box(p.routes.join("\n"))
                .header(t("network.routing.routes"))
                .placeholder("192.168.0.0/16")
                .multiline()
                .height(80.0)
                .enabled(p.enable_manual_routes)
                .on_changed(on_edit!(ctx, |p, v: String| p.routes = split_lines(&v))),
        ))
        .spacing(14.0)
        .into(),
    );

    let exit = card(
        "network.routing.exit_nodes",
        vstack((
            switch_row(
                "network.routing.act_as_exit",
                "network.routing.act_as_exit_help",
                p.enable_exit_node,
                on_edit!(ctx, |p, v: bool| p.enable_exit_node = v),
            ),
            divider(),
            text_block(t("network.routing.use_exit_text"))
                .font_size(12.0)
                .opacity(0.7)
                .wrap(),
            text_box(p.exit_nodes.join("\n"))
                .placeholder("10.126.126.10")
                .multiline()
                .height(80.0)
                .on_changed(on_edit!(ctx, |p, v: String| p.exit_nodes = split_lines(&v))),
        ))
        .spacing(12.0)
        .into(),
    );

    vstack((preference, subnet_proxy, manual_routes, exit))
        .spacing(18.0)
        .into()
}

// ─────────────────────────── Network → Proxies ────────────────────────────

fn proxies_panel(ctx: &PageCtx, p: &Profile) -> Element {
    let socks = {
        let mut body: Vec<Element> = vec![switch_row(
            "network.proxies.socks5_toggle",
            "network.proxies.socks5_help",
            p.enable_socks5,
            on_edit!(ctx, |p, v: bool| p.enable_socks5 = v),
        )];
        body.push(
            NumberBox::new(p.socks5_port as f64)
                .header(t("network.proxies.listen_port"))
                .range(1.0, 65535.0)
                .enabled(p.enable_socks5)
                .on_value_changed(on_edit!(ctx, |p, v: f64| {
                    p.socks5_port = v.clamp(1.0, 65535.0) as u16
                }))
                .into(),
        );
        card("network.proxies.socks5", vstack(body).spacing(14.0).into())
    };

    let portal = {
        let mut body: Vec<Element> = vec![
            switch_row(
                "network.proxies.portal_toggle",
                "network.proxies.portal_help",
                p.enable_vpn_portal,
                on_edit!(ctx, |p, v: bool| p.enable_vpn_portal = v),
            ),
            grid((
                text_box(p.vpn_portal_client_network_addr.clone())
                    .header(t("network.proxies.client_subnet"))
                    .placeholder("10.14.14.0")
                    .enabled(p.enable_vpn_portal)
                    .on_changed(on_edit!(ctx, |p, v: String| {
                        p.vpn_portal_client_network_addr = v
                    }))
                    .grid_column(0),
                NumberBox::new(p.vpn_portal_client_network_len as f64)
                    .header(t("network.proxies.prefix"))
                    .range(1.0, 32.0)
                    .enabled(p.enable_vpn_portal)
                    .on_value_changed(on_edit!(ctx, |p, v: f64| {
                        p.vpn_portal_client_network_len = v.clamp(1.0, 32.0) as u8
                    }))
                    .grid_column(1),
                NumberBox::new(p.vpn_portal_listen_port as f64)
                    .header(t("network.proxies.listen_port_udp"))
                    .range(1.0, 65535.0)
                    .enabled(p.enable_vpn_portal)
                    .on_value_changed(on_edit!(ctx, |p, v: f64| {
                        p.vpn_portal_listen_port = v.clamp(1.0, 65535.0) as u16
                    }))
                    .grid_column(2),
            ))
            .columns([
                GridLength::Star(3.0),
                GridLength::Star(1.0),
                GridLength::Star(2.0),
            ])
            .column_spacing(12.0)
            .into(),
        ];
        if p.enable_vpn_portal
            && let Some(net) = ctx.snap.net(&p.id)
        {
            if let Some(cfg) = net.vpn_portal_cfg.clone() {
                body.push(vpn_portal_viewer(cfg));
            } else {
                body.push(
                    text_block(t("network.proxies.portal_connect_hint"))
                        .font_size(12.0)
                        .opacity(0.6)
                        .into(),
                );
            }
        }
        card("network.proxies.portal", vstack(body).spacing(14.0).into())
    };

    let forwards = port_forwards_card(ctx, p);

    vstack((socks, portal, forwards)).spacing(18.0).into()
}

/// Render the live WireGuard client config in a selectable text box.
fn vpn_portal_viewer(cfg: String) -> Element {
    let url = "https://www.wireguardconfig.com/qrcode";
    border(
        vstack((
            text_block(t("network.proxies.wg_config_title"))
                .font_size(12.0)
                .semibold(),
            text_box(cfg)
                .multiline()
                .height(170.0)
                .font_family("Consolas")
                .font_size(12.0),
            hstack((
                text_block(t("network.proxies.wg_paste_hint"))
                    .font_size(11.0)
                    .opacity(0.6)
                    .vertical_alignment(VerticalAlignment::Center),
                link("network.proxies.wg_qr_link", url),
            ))
            .spacing(2.0)
            .vertical_alignment(VerticalAlignment::Center),
        ))
        .spacing(8.0),
    )
    .background(ThemeRef::SubtleFill)
    .corner_radius(6.0)
    .padding(Thickness::uniform(12.0))
    .into()
}

fn port_forwards_card(ctx: &PageCtx, p: &Profile) -> Element {
    let header = grid((
        text_block(t("network.proxies.forwards_text"))
            .font_size(12.0)
            .opacity(0.7)
            .wrap()
            .grid_column(0),
        button(t("network.proxies.add_forward"))
            .accent()
            .icon(SymbolGlyph::Add)
            .on_click({
                let set = ctx.set_store.clone();
                let store = ctx.store.clone();
                move || {
                    let mut s = store.clone();
                    let i = s.selected.min(s.profiles.len() - 1);
                    s.profiles[i].port_forwards.push(PortForward::default());
                    s.save();
                    set.call(s);
                }
            })
            .grid_column(1),
    ))
    .columns([GridLength::Star(1.0), GridLength::Auto])
    .column_spacing(16.0);

    let mut rows: Vec<Element> = vec![header.into()];

    if p.port_forwards.is_empty() {
        rows.push(
            text_block(t("network.proxies.no_forwards"))
                .font_size(12.0)
                .opacity(0.55)
                .into(),
        );
    } else {
        rows.push(port_forward_header());
        for i in 0..p.port_forwards.len() {
            rows.push(port_forward_row(ctx, p, i));
        }
    }

    card(
        "network.proxies.port_forwards",
        vstack(rows).spacing(10.0).into(),
    )
}

fn port_forward_header() -> Element {
    hstack((
        cell(t("network.proxies.col_proto"), 70.0, true, false),
        cell(t("network.proxies.col_bind_ip"), 140.0, true, false),
        cell(t("network.proxies.col_bind_port"), 95.0, true, false),
        cell(t("network.proxies.col_dest_ip"), 160.0, true, false),
        cell(t("network.proxies.col_dest_port"), 95.0, true, false),
        cell("".into(), 44.0, true, false),
    ))
    .spacing(8.0)
    .into()
}

fn port_forward_row(ctx: &PageCtx, p: &Profile, i: usize) -> Element {
    let pf = p.port_forwards[i].clone();
    let proto_options: Vec<String> = vec!["tcp".into(), "udp".into()];
    let proto_idx = if pf.proto == "udp" { 1 } else { 0 };

    let proto = ComboBox::new(proto_options)
        .selected_index(proto_idx)
        .width(70.0)
        .on_selection_changed({
            let set = ctx.set_store.clone();
            let store = ctx.store.clone();
            move |idx: i32| {
                let mut s = store.clone();
                let sel = s.selected.min(s.profiles.len() - 1);
                if let Some(row) = s.profiles[sel].port_forwards.get_mut(i) {
                    row.proto = if idx == 1 { "udp".into() } else { "tcp".into() };
                    s.save();
                    set.call(s);
                }
            }
        });

    let bind_ip = text_box(pf.bind_ip.clone()).width(140.0).on_changed({
        let set = ctx.set_store.clone();
        let store = ctx.store.clone();
        move |v: String| {
            let mut s = store.clone();
            let sel = s.selected.min(s.profiles.len() - 1);
            if let Some(row) = s.profiles[sel].port_forwards.get_mut(i) {
                row.bind_ip = v;
                s.save();
                set.call(s);
            }
        }
    });

    let bind_port = NumberBox::new(pf.bind_port as f64)
        .width(95.0)
        .range(0.0, 65535.0)
        .on_value_changed({
            let set = ctx.set_store.clone();
            let store = ctx.store.clone();
            move |v: f64| {
                let mut s = store.clone();
                let sel = s.selected.min(s.profiles.len() - 1);
                if let Some(row) = s.profiles[sel].port_forwards.get_mut(i) {
                    row.bind_port = v.clamp(0.0, 65535.0) as u16;
                    s.save();
                    set.call(s);
                }
            }
        });

    let dst_ip = text_box(pf.dst_ip.clone()).width(160.0).on_changed({
        let set = ctx.set_store.clone();
        let store = ctx.store.clone();
        move |v: String| {
            let mut s = store.clone();
            let sel = s.selected.min(s.profiles.len() - 1);
            if let Some(row) = s.profiles[sel].port_forwards.get_mut(i) {
                row.dst_ip = v;
                s.save();
                set.call(s);
            }
        }
    });

    let dst_port = NumberBox::new(pf.dst_port as f64)
        .width(95.0)
        .range(0.0, 65535.0)
        .on_value_changed({
            let set = ctx.set_store.clone();
            let store = ctx.store.clone();
            move |v: f64| {
                let mut s = store.clone();
                let sel = s.selected.min(s.profiles.len() - 1);
                if let Some(row) = s.profiles[sel].port_forwards.get_mut(i) {
                    row.dst_port = v.clamp(0.0, 65535.0) as u16;
                    s.save();
                    set.call(s);
                }
            }
        });

    let del = button("")
        .subtle()
        .icon(SymbolGlyph::Delete)
        .width(44.0)
        .on_click({
            let set = ctx.set_store.clone();
            let store = ctx.store.clone();
            move || {
                let mut s = store.clone();
                let sel = s.selected.min(s.profiles.len() - 1);
                if i < s.profiles[sel].port_forwards.len() {
                    s.profiles[sel].port_forwards.remove(i);
                    s.save();
                    set.call(s);
                }
            }
        });

    hstack((proto, bind_ip, bind_port, dst_ip, dst_port, del))
        .spacing(8.0)
        .vertical_alignment(VerticalAlignment::Center)
        .into()
}

// ─────────────────────────── Network → Advanced ───────────────────────────

fn advanced_panel(ctx: &PageCtx, p: &Profile) -> Element {
    let listeners_card = card(
        "network.advanced.listeners",
        vstack((
            text_block(t("network.advanced.listeners_text"))
                .font_size(12.0)
                .opacity(0.7)
                .wrap(),
            text_box(p.listeners.join("\n"))
                .header(t("network.advanced.listener_urls"))
                .placeholder("tcp://0.0.0.0:11010\nudp://0.0.0.0:11010\nwg://0.0.0.0:11011")
                .multiline()
                .height(96.0)
                .on_changed(on_edit!(ctx, |p, v: String| p.listeners = split_lines(&v))),
            text_box(p.mapped_listeners.join("\n"))
                .header(t("network.advanced.mapped_listeners"))
                .placeholder("tcp://my.public.dns:11010")
                .multiline()
                .height(72.0)
                .on_changed(on_edit!(ctx, |p, v: String| {
                    p.mapped_listeners = split_lines(&v)
                })),
        ))
        .spacing(14.0)
        .into(),
    );

    let limits = card(
        "network.advanced.limits",
        grid((
            NumberBox::new(p.mtu as f64)
                .header(t("network.advanced.mtu"))
                .range(0.0, 1500.0)
                .on_value_changed(on_edit!(ctx, |p, v: f64| p.mtu = v.clamp(0.0, 1500.0) as u32))
                .grid_column(0),
            NumberBox::new(p.instance_recv_bps_limit as f64)
                .header(t("network.advanced.recv_limit"))
                .range(0.0, 1_000_000_000_000.0)
                .on_value_changed(on_edit!(ctx, |p, v: f64| {
                    p.instance_recv_bps_limit = v.max(0.0) as u64
                }))
                .grid_column(1),
        ))
        .columns([GridLength::Star(1.0), GridLength::Star(2.0)])
        .column_spacing(14.0)
        .into(),
    );

    let transport = card(
        "network.advanced.transport",
        flag_grid(vec![
            flag(
                "network.advanced.kcp_proxy",
                "network.advanced.kcp_proxy_help",
                p.enable_kcp_proxy,
                on_edit!(ctx, |p, v: bool| p.enable_kcp_proxy = v),
            ),
            flag(
                "network.advanced.refuse_kcp",
                "network.advanced.refuse_kcp_help",
                p.disable_kcp_input,
                on_edit!(ctx, |p, v: bool| p.disable_kcp_input = v),
            ),
            flag(
                "network.advanced.quic_proxy",
                "network.advanced.quic_proxy_help",
                p.enable_quic_proxy,
                on_edit!(ctx, |p, v: bool| p.enable_quic_proxy = v),
            ),
            flag(
                "network.advanced.refuse_quic",
                "network.advanced.refuse_quic_help",
                p.disable_quic_input,
                on_edit!(ctx, |p, v: bool| p.disable_quic_input = v),
            ),
            flag(
                "network.advanced.smoltcp",
                "network.advanced.smoltcp_help",
                p.use_smoltcp,
                on_edit!(ctx, |p, v: bool| p.use_smoltcp = v),
            ),
            flag(
                "network.advanced.disable_ipv6",
                "network.advanced.disable_ipv6_help",
                p.disable_ipv6,
                on_edit!(ctx, |p, v: bool| p.disable_ipv6 = v),
            ),
            flag(
                "network.advanced.auto_ipv6",
                "network.advanced.auto_ipv6_help",
                p.ipv6_public_addr_auto,
                on_edit!(ctx, |p, v: bool| p.ipv6_public_addr_auto = v),
            ),
            flag(
                "network.advanced.udp_broadcast",
                "network.advanced.udp_broadcast_help",
                p.enable_udp_broadcast_relay,
                on_edit!(ctx, |p, v: bool| p.enable_udp_broadcast_relay = v),
            ),
        ]),
    );

    let p2p = card(
        "network.advanced.p2p_nat",
        flag_grid(vec![
            flag(
                "network.advanced.disable_p2p",
                "network.advanced.disable_p2p_help",
                p.disable_p2p,
                on_edit!(ctx, |p, v: bool| p.disable_p2p = v),
            ),
            flag(
                "network.advanced.p2p_only",
                "network.advanced.p2p_only_help",
                p.p2p_only,
                on_edit!(ctx, |p, v: bool| p.p2p_only = v),
            ),
            flag(
                "network.advanced.lazy_p2p",
                "network.advanced.lazy_p2p_help",
                p.lazy_p2p,
                on_edit!(ctx, |p, v: bool| p.lazy_p2p = v),
            ),
            flag(
                "network.advanced.require_p2p",
                "network.advanced.require_p2p_help",
                p.need_p2p,
                on_edit!(ctx, |p, v: bool| p.need_p2p = v),
            ),
            flag(
                "network.advanced.skip_tcp_punch",
                "network.advanced.skip_tcp_punch_help",
                p.disable_tcp_hole_punching,
                on_edit!(ctx, |p, v: bool| p.disable_tcp_hole_punching = v),
            ),
            flag(
                "network.advanced.skip_udp_punch",
                "network.advanced.skip_udp_punch_help",
                p.disable_udp_hole_punching,
                on_edit!(ctx, |p, v: bool| p.disable_udp_hole_punching = v),
            ),
            flag(
                "network.advanced.skip_sym_punch",
                "network.advanced.skip_sym_punch_help",
                p.disable_sym_hole_punching,
                on_edit!(ctx, |p, v: bool| p.disable_sym_hole_punching = v),
            ),
            flag(
                "network.advanced.disable_upnp",
                "network.advanced.disable_upnp_help",
                p.disable_upnp,
                on_edit!(ctx, |p, v: bool| p.disable_upnp = v),
            ),
        ]),
    );

    let instance = card(
        "network.advanced.instance",
        flag_grid(vec![
            flag(
                "network.advanced.relay_rpc",
                "network.advanced.relay_rpc_help",
                p.relay_all_peer_rpc,
                on_edit!(ctx, |p, v: bool| p.relay_all_peer_rpc = v),
            ),
            flag(
                "network.advanced.multi_thread",
                "network.advanced.multi_thread_help",
                p.multi_thread,
                on_edit!(ctx, |p, v: bool| p.multi_thread = v),
            ),
            flag(
                "network.advanced.forward_by_system",
                "network.advanced.forward_by_system_help",
                p.proxy_forward_by_system,
                on_edit!(ctx, |p, v: bool| p.proxy_forward_by_system = v),
            ),
            flag(
                "network.advanced.bind_device",
                "network.advanced.bind_device_help",
                p.bind_device,
                on_edit!(ctx, |p, v: bool| p.bind_device = v),
            ),
            flag(
                "network.advanced.no_tun",
                "network.advanced.no_tun_help",
                p.no_tun,
                on_edit!(ctx, |p, v: bool| p.no_tun = v),
            ),
        ]),
    );

    vstack((listeners_card, limits, transport, p2p, instance))
        .spacing(18.0)
        .into()
}

// ───────────────────────────────── Peers ──────────────────────────────────

pub fn peers_page(ctx: &PageCtx) -> Element {
    let live: Vec<&Profile> = ctx
        .store
        .profiles
        .iter()
        .filter(|p| ctx.snap.net(&p.id).is_some_and(|n| !n.peers.is_empty()))
        .collect();

    if live.is_empty() {
        return page(
            "nav.peers",
            "peers.subtitle",
            vec![card(
                "",
                InfoBar::new(t("peers.empty_title"))
                    .message(t("peers.empty_message"))
                    .informational()
                    .is_closable(false)
                    .into(),
            )],
        );
    }

    let mut sections = Vec::new();
    for prof in live {
        let net = ctx.snap.net(&prof.id).unwrap();
        sections.push(card(
            &format!("{}  ({})", prof.name, tn("peers.count", net.peers.len())),
            vstack((my_node_chips(net), peer_table(&net.peers)))
                .spacing(14.0)
                .into(),
        ));
    }

    page("nav.peers", "peers.subtitle", sections)
}

fn my_node_chips(net: &NetSnapshot) -> Element {
    let mut chips: Vec<Element> = Vec::new();
    let mut push = |label: &str, value: &str| {
        if value.is_empty() {
            return;
        }
        let value = value.to_string();
        chips.push(
            border(
                hstack((
                    text_block(t(label))
                        .font_size(11.0)
                        .opacity(0.55)
                        .semibold(),
                    // Normal label; click to copy, hover for the full value.
                    text_block(value.clone())
                        .font_size(11.0)
                        .tooltip(format!("{value}  —  click to copy"))
                        .on_tapped({
                            let value = value.clone();
                            move || {
                                crate::dialog::write_clipboard_text(&value);
                            }
                        }),
                ))
                .spacing(6.0)
                .vertical_alignment(VerticalAlignment::Center),
            )
            .background(ThemeRef::SubtleFill)
            .corner_radius(10.0)
            .padding(pad(8.0, 3.0, 8.0, 3.0))
            .into(),
        );
    };

    push("peers.chip_id", &net.peer_id.to_string());
    push("peers.chip_virtual_ip", &net.virtual_ip);
    push("peers.chip_tun", &net.dev_name);
    push("peers.chip_nat", &net.nat_type);
    push("peers.chip_public_v4", &net.public_ipv4);
    push("peers.chip_public_v6", &net.public_ipv6);
    push("peers.chip_version", &net.version);
    for (i, l) in net.listeners.iter().enumerate() {
        push(&tn("peers.chip_listener", i + 1), l);
    }

    let mut rows: Vec<Element> = Vec::new();
    let mut acc: Vec<Element> = Vec::new();
    for c in chips {
        acc.push(c);
        if acc.len() >= 4 {
            rows.push(hstack(std::mem::take(&mut acc)).spacing(6.0).into());
        }
    }
    if !acc.is_empty() {
        rows.push(hstack(acc).spacing(6.0).into());
    }

    vstack(rows).spacing(6.0).into()
}

fn peer_table(peers: &[PeerRow]) -> Element {
    const HEADERS: [&str; 10] = [
        "peers.col_device",
        "peers.col_virtual_ip",
        "peers.col_route",
        "peers.col_latency",
        "peers.col_tunnel",
        "peers.col_nat",
        "peers.col_down",
        "peers.col_up",
        "peers.col_loss",
        "peers.col_version",
    ];
    // Per-column (min, max) width clamps in px — columns size to their content
    // within these bounds; anything wider is clipped (the tooltip shows it all).
    const LIMITS: [(f64, f64); 10] = [
        (120.0, 260.0), // Device
        (90.0, 180.0),  // Virtual IP
        (58.0, 110.0),  // Route
        (58.0, 90.0),   // Latency
        (54.0, 90.0),   // Tunnel
        (90.0, 180.0),  // NAT
        (60.0, 110.0),  // Down
        (60.0, 110.0),  // Up
        (44.0, 70.0),   // Loss
        (60.0, 150.0),  // Version
    ];

    // The display string for every cell, so columns can be sized to fit.
    let body: Vec<[String; 10]> = peers
        .iter()
        .map(|p| {
            [
                p.hostname.clone(),
                or_dash(&p.ipv4),
                p.cost.clone(),
                p.latency_ms
                    .map_or_else(|| "—".to_string(), |l| format!("{l:.0} ms")),
                p.tunnel.clone(),
                p.nat.clone(),
                human_bytes(p.rx),
                human_bytes(p.tx),
                format!("{:.0}%", p.loss * 100.0),
                p.version.clone(),
            ]
        })
        .collect();

    // Translated headers, used both for column sizing and the header row.
    let headers: [String; 10] = std::array::from_fn(|i| t(HEADERS[i]));

    // Auto-size each column to its widest value (header included), clamped.
    // ~7.6 px/char is a rough proportional-font estimate; the clamps bound it.
    let widths: [f64; 10] = std::array::from_fn(|i| {
        let chars = body
            .iter()
            .map(|r| r[i].chars().count())
            .chain(std::iter::once(headers[i].chars().count()))
            .max()
            .unwrap_or(0);
        (chars as f64 * 7.6 + 8.0).clamp(LIMITS[i].0, LIMITS[i].1)
    });

    let header = hstack(
        headers
            .iter()
            .enumerate()
            .map(|(i, h)| cell(h.clone(), widths[i], true, false))
            .collect::<Vec<_>>(),
    )
    .spacing(8.0)
    .into();

    let mut rows: Vec<Element> = vec![header];
    for p in peers {
        rows.push(
            hstack(vec![
                copy_cell(p.hostname.clone(), widths[0]),
                copy_cell(or_dash(&p.ipv4), widths[1]),
                cell(
                    if p.cost == "Direct" {
                        t("peers.cost_direct")
                    } else {
                        p.cost.clone()
                    },
                    widths[2],
                    false,
                    p.cost != "Direct",
                ),
                cell(
                    p.latency_ms
                        .map_or_else(|| "—".to_string(), |l| format!("{l:.0} ms")),
                    widths[3],
                    false,
                    p.latency_ms.is_none(),
                ),
                cell(p.tunnel.clone(), widths[4], false, false),
                cell(p.nat.clone(), widths[5], false, true),
                cell(human_bytes(p.rx), widths[6], false, true),
                cell(human_bytes(p.tx), widths[7], false, true),
                cell(format!("{:.0}%", p.loss * 100.0), widths[8], false, true),
                cell(p.version.clone(), widths[9], false, true),
            ])
            .spacing(8.0)
            .into(),
        );
    }

    // Use the legacy ScrollViewer: the modern ScrollView's `content_orientation`
    // is never forwarded to the backend in this reactor rev, so it can't scroll
    // horizontally at all. ScrollViewer's HorizontalScrollBarVisibility *is*
    // applied — Auto scrolls sideways when the table is wider than the card;
    // vertical is disabled so the page owns vertical scrolling of the whole card.
    //
    // The bottom padding reserves a gutter so the overlay horizontal scrollbar
    // floats below the last row instead of covering it.
    scroll_viewer(border(vstack(rows).spacing(8.0)).padding(pad(0.0, 0.0, 0.0, 16.0)))
        .horizontal_scroll_bar_visibility(ScrollBarVisibility::Auto)
        .vertical_scroll_bar_visibility(ScrollBarVisibility::Disabled)
        .into()
}

// ──────────────────────────────── Activity ────────────────────────────────

pub fn logs_page(ctx: &PageCtx) -> Element {
    let active: Vec<&Profile> = ctx
        .store
        .profiles
        .iter()
        .filter(|p| ctx.snap.net(&p.id).is_some_and(|n| !n.events.is_empty()))
        .collect();

    if active.is_empty() {
        return page(
            "nav.activity",
            "activity.subtitle",
            vec![card(
                "",
                text_block(t("activity.empty"))
                    .font_size(13.0)
                    .opacity(0.7)
                    .into(),
            )],
        );
    }

    let mut sections = Vec::new();
    for prof in active {
        let net = ctx.snap.net(&prof.id).unwrap();
        let lines: Vec<Element> = net
            .events
            .iter()
            .map(|e| {
                text_block(e.clone())
                    .font_family("Consolas")
                    .font_size(12.0)
                    .wrap()
                    .into()
            })
            .collect();
        sections.push(card(
            &format!(
                "{}  ({})",
                prof.name,
                tn("activity.count", net.events.len())
            ),
            scroll_view(vstack(lines).spacing(4.0))
                .max_height(320.0)
                .into(),
        ));
    }

    page("nav.activity", "activity.subtitle", sections)
}

// ──────────────────────────────── Settings ────────────────────────────────

pub fn settings_page(ctx: &PageCtx) -> Element {
    let s = &ctx.store.settings;

    let theme_labels: Vec<String> = Theme::ALL.iter().map(|t| t.label().to_string()).collect();
    let material_labels: Vec<String> = Material::ALL
        .iter()
        .map(|m| m.label().to_string())
        .collect();

    let appearance = card(
        "settings.appearance",
        vstack((
            ComboBox::new(theme_labels)
                .header(t("settings.theme"))
                .selected_index(s.theme.index())
                .on_selection_changed(
                    on_setting!(ctx, |st, v: i32| st.theme = Theme::from_index(v))
                ),
            ComboBox::new(material_labels)
                .header(t("settings.material"))
                .selected_index(s.material.index())
                .on_selection_changed(
                    on_setting!(ctx, |st, v: i32| st.material = Material::from_index(v))
                ),
        ))
        .spacing(14.0)
        .into(),
    );

    let behavior = card(
        "settings.behaviour",
        switch_row(
            "settings.auto_connect",
            "settings.auto_connect_help",
            s.auto_connect,
            on_setting!(ctx, |st, v: bool| st.auto_connect = v),
        ),
    );

    let tray = card(
        "settings.tray",
        vstack((
            switch_row(
                "settings.close_to_tray",
                "settings.close_to_tray_help",
                s.close_to_tray,
                on_setting!(ctx, |st, v: bool| st.close_to_tray = v),
            ),
            divider(),
            switch_row(
                "settings.minimize_to_tray",
                "settings.minimize_to_tray_help",
                s.minimize_to_tray,
                on_setting!(ctx, |st, v: bool| st.minimize_to_tray = v),
            ),
        ))
        .spacing(14.0)
        .into(),
    );

    let language_labels: Vec<String> = Language::ALL.iter().map(|l| l.label()).collect();
    let language = card(
        "settings.language",
        ComboBox::new(language_labels)
            .header(t("settings.language"))
            .selected_index(s.language.index())
            .on_selection_changed(
                on_setting!(ctx, |st, v: i32| st.language = Language::from_index(v))
            )
            .into(),
    );

    let mut cards = vec![appearance, language, behavior, tray];
    cards.push(diagnostics_card(ctx));
    cards.push(admin_card(ctx));

    page("nav.settings", "settings.subtitle", cards)
}

fn admin_card(ctx: &PageCtx) -> Element {
    let elevated = crate::elevate::is_elevated();
    let status = if elevated {
        t("settings.admin_elevated")
    } else {
        t("settings.admin_not_elevated")
    };

    // A packaged (MSIX) build elevates through its manifest (allowElevation +
    // highestAvailable, `--features msix`), not a UAC relaunch — so the launch
    // toggle and on-demand restart don't apply. Show read-only status instead.
    if crate::elevate::is_packaged() {
        let detail = if elevated {
            status.clone()
        } else {
            t("settings.admin_packaged_not_elevated")
        };
        return card(
            "settings.admin",
            text_block(detail)
                .font_size(12.0)
                .opacity(0.6)
                .wrap()
                .into(),
        );
    }

    let mut body: Vec<Element> = vec![
        switch_row(
            "settings.always_admin",
            "settings.always_admin_help",
            ctx.store.settings.always_admin,
            on_setting!(ctx, |st, v: bool| st.always_admin = v),
        ),
        text_block(status)
            .font_size(12.0)
            .opacity(0.6)
            .wrap()
            .into(),
    ];
    if !elevated {
        body.push(
            button(t("settings.restart_admin"))
                .accent()
                .on_click(|| {
                    if crate::elevate::relaunch_elevated() {
                        std::process::exit(0);
                    }
                })
                .horizontal_alignment(HorizontalAlignment::Left)
                .into(),
        );
    }

    card("settings.admin", vstack(body).spacing(14.0).into())
}

// ───────────────────────────────── About ──────────────────────────────────

pub fn about_page() -> Element {
    let about = card(
        "about.app_title",
        vstack((
            text_block(t("about.desc")).font_size(14.0),
            text_block(format!(
                "{} {}",
                t("about.version"),
                env!("CARGO_PKG_VERSION")
            ))
            .font_size(12.0)
            .opacity(0.7),
            text_block(format!("{} {}", t("about.engine"), easytier::VERSION))
                .font_size(12.0)
                .opacity(0.7),
            text_block(t("about.built_with"))
                .font_size(12.0)
                .opacity(0.7),
        ))
        .spacing(6.0)
        .into(),
    );

    let links = card(
        "about.learn_more",
        vstack((
            link("about.link_polaris", "https://github.com/l5z12/polaris_et"),
            link("about.link_website", "https://easytier.cn"),
            link(
                "about.link_easy_tier",
                "https://github.com/EasyTier/EasyTier",
            ),
            link(
                "about.link_reactor",
                "https://github.com/microsoft/windows-rs",
            ),
        ))
        .spacing(2.0)
        .into(),
    );

    let credits = card(
        "about.credits",
        vstack((
            text_block(t("about.icon_attribution"))
                .font_size(12.0)
                .opacity(0.7)
                .wrap(),
            link(
                "about.icon_link",
                "https://github.com/microsoft/fluentui-system-icons",
            ),
        ))
        .spacing(6.0)
        .into(),
    );

    let license = card(
        "about.license",
        vstack((
            text_block(t("about.copyright"))
                .font_size(12.0)
                .opacity(0.7),
            text_block(t("about.license_text"))
                .font_size(12.0)
                .opacity(0.7)
                .wrap(),
            link(
                "about.link_gpl",
                "https://www.gnu.org/licenses/gpl-3.0.html",
            ),
            link("about.link_source", "https://github.com/l5z12/polaris_et"),
        ))
        .spacing(6.0)
        .into(),
    );

    page(
        "nav.about",
        "about.subtitle",
        vec![about, links, credits, license],
    )
}

// ───────────────────────────── Diagnostics ────────────────────────────────

/// Retention presets (days) for the log-cleanup dropdown.
const RETENTION_DAYS: [u32; 5] = [1, 3, 7, 14, 30];

fn retention_index(days: u32) -> i32 {
    RETENTION_DAYS.iter().position(|d| *d == days).unwrap_or(2) as i32
}

/// `yes`/`no` for the plain-text exported log header (never translated).
fn yn(b: bool) -> &'static str {
    if b { "yes" } else { "no" }
}

/// i18n key for the translated yes/no shown on the Diagnostics page.
fn yn_key(b: bool) -> &'static str {
    if b {
        "diagnostics.yes"
    } else {
        "diagnostics.no"
    }
}

/// One-line environment summary, prepended to exported logs.
fn diagnostics_header() -> String {
    format!(
        "Polaris {} · EasyTier {} · elevated:{} · packaged:{} · {}",
        env!("CARGO_PKG_VERSION"),
        easytier::VERSION,
        yn(crate::elevate::is_elevated()),
        yn(crate::elevate::is_packaged()),
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
    )
}

fn export_logs() {
    let name = format!(
        "polaris-logs-{}.txt",
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    );
    let f_text = t("dialog.text");
    let f_log = t("dialog.log");
    let f_all = t("dialog.all_files");
    if let Some(path) = crate::dialog::save_file(
        &name,
        &[
            (f_text.as_str(), "*.txt"),
            (f_log.as_str(), "*.log"),
            (f_all.as_str(), "*.*"),
        ],
    ) {
        let _ = crate::logging::export(&path, &diagnostics_header());
    }
}

fn open_logs_folder() {
    let dir = crate::logging::logs_dir();
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::process::Command::new("explorer").arg(dir).spawn();
}

/// Settings → Diagnostics: enable toggle, then log level / retention / export.
fn diagnostics_card(ctx: &PageCtx) -> Element {
    let s = &ctx.store.settings;

    let mut body: Vec<Element> = vec![switch_row(
        "diagnostics.verbose",
        "diagnostics.verbose_help",
        s.diagnostics_enabled,
        on_setting!(ctx, |st, v: bool| st.diagnostics_enabled = v),
    )];

    if s.diagnostics_enabled {
        let level_labels: Vec<String> = LogLevel::ALL
            .iter()
            .map(|l| l.label().to_string())
            .collect();
        let retention_labels: Vec<String> = RETENTION_DAYS
            .iter()
            .map(|d| tn("diagnostics.retention_days", *d as usize))
            .collect();

        body.push(divider());
        body.push(
            ComboBox::new(level_labels)
                .header(t("diagnostics.log_level"))
                .selected_index(s.log_level.index())
                .on_selection_changed(
                    on_setting!(ctx, |st, v: i32| st.log_level = LogLevel::from_index(v))
                )
                .into(),
        );
        body.push(
            ComboBox::new(retention_labels)
                .header(t("diagnostics.retention"))
                .selected_index(retention_index(s.log_retention_days))
                .on_selection_changed(on_setting!(ctx, |st, v: i32| {
                    let i = (v.max(0) as usize).min(RETENTION_DAYS.len() - 1);
                    st.log_retention_days = RETENTION_DAYS[i];
                }))
                .into(),
        );
        body.push(
            hstack((
                button(t("diagnostics.export_logs")).on_click(export_logs),
                button(t("diagnostics.open_folder")).on_click(open_logs_folder),
            ))
            .spacing(8.0)
            .into(),
        );
        body.push(
            text_block(format!(
                "{}: {}",
                t("diagnostics.logs_path"),
                crate::logging::logs_dir().display()
            ))
            .font_size(11.0)
            .opacity(0.55)
            .wrap()
            .into(),
        );
    }

    card("diagnostics.card", vstack(body).spacing(14.0).into())
}

/// The Diagnostics page: environment summary + a live tail of the log buffer.
/// Re-renders each second (via `tick`), so the log updates live.
pub fn diagnostics_page(ctx: &PageCtx) -> Element {
    let active = ctx
        .store
        .profiles
        .iter()
        .filter(|p| ctx.snap.net(&p.id).is_some())
        .count();

    let info = card(
        "diagnostics.environment",
        vstack((
            text_block(format!(
                "Polaris {}  ·  EasyTier {}",
                env!("CARGO_PKG_VERSION"),
                easytier::VERSION
            ))
            .font_size(13.0)
            .semibold(),
            text_block(format!(
                "{}: {}   ·   {}: {}   ·   {}: {}",
                t("diagnostics.elevated"),
                t(yn_key(crate::elevate::is_elevated())),
                t("diagnostics.packaged"),
                t(yn_key(crate::elevate::is_packaged())),
                t("diagnostics.active_networks"),
                active,
            ))
            .font_size(12.0)
            .opacity(0.7),
            text_block(format!(
                "{}: {}   ·   {}",
                t("diagnostics.log_level"),
                ctx.store.settings.log_level.label(),
                crate::logging::logs_dir().display(),
            ))
            .font_size(11.0)
            .opacity(0.55)
            .wrap(),
        ))
        .spacing(4.0)
        .into(),
    );

    let lines = crate::logging::recent(400);
    let log_body: Element = if lines.is_empty() {
        text_block(t("diagnostics.no_log"))
            .font_size(12.0)
            .opacity(0.7)
            .wrap()
            .into()
    } else {
        let rows: Vec<Element> = lines
            .iter()
            .map(|l| {
                text_block(l.clone())
                    .font_family("Consolas")
                    .font_size(11.0)
                    .wrap()
                    .into()
            })
            .collect();
        scroll_view(vstack(rows).spacing(2.0))
            .max_height(460.0)
            .into()
    };

    let toolbar: Element = hstack((
        button(t("diagnostics.export_logs")).on_click(export_logs),
        button(t("diagnostics.open_folder")).on_click(open_logs_folder),
        button(t("diagnostics.clear")).on_click(crate::logging::clear),
    ))
    .spacing(8.0)
    .into();

    let logs = card(
        &format!(
            "{}  ({})",
            t("diagnostics.live_log"),
            tn("diagnostics.lines", lines.len())
        ),
        vstack(vec![toolbar, log_body]).spacing(12.0).into(),
    );

    page("nav.diagnostics", "diagnostics.subtitle", vec![info, logs])
}
