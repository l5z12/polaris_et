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
    let title = t(title);
    let mut col: Vec<Element> = Vec::new();
    if !title.is_empty() {
        col.push(text_block(title).font_size(15.0).semibold().into());
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
    InfoBar::new(t("This network is live"))
        .message(t("Edits are saved now and apply the next time you reconnect it."))
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
            stat("Connected", format!("{}", snap.connected_count())),
            stat("Networks", format!("{}", ctx.store.profiles.len())),
            stat("Peers", format!("{}", snap.total_peers())),
            stat("Downloaded", human_bytes(snap.total_rx())),
            stat("Uploaded", human_bytes(snap.total_tx())),
        ))
        .spacing(32.0)
        .into(),
    );

    // Network-list toolbar — Add / Import live with the list of networks they
    // affect, not in app-level Settings.
    let toolbar = card(
        "",
        grid((
            text_block(t("Your saved networks"))
                .font_size(13.0)
                .opacity(0.7)
                .vertical_alignment(VerticalAlignment::Center)
                .grid_column(0),
            hstack((
                add_network_button(ctx),
                import_button(ctx),
                paste_button(ctx),
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

    page(
        "Networks",
        "Connect to one or more EasyTier networks at the same time.",
        children,
    )
}

/// Shown when running without admin: TUN is off, proxies still work.
fn tun_notice() -> Element {
    InfoBar::new(t("Running without administrator rights"))
        .message(t(
            "VPN mode (the TUN adapter) is disabled. SOCKS5 and port-forward proxies still \
             work. For full VPN, enable “Always launch as administrator” in Settings.",
        ))
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
        button(t("Disconnect"))
            .on_click(move || e.stop(id.clone()))
            .min_width(128.0)
            .into()
    } else {
        let e = ctx.engine.clone();
        let id = prof.id.clone();
        let cfg = prof.to_network_config();
        button(t("Connect"))
            .accent()
            .on_click(move || e.start(id.clone(), cfg.clone()))
            .min_width(128.0)
            .into()
    };

    let edit = {
        let set = ctx.set_store.clone();
        let set_tab = ctx.set_tab.clone();
        let store = ctx.store.clone();
        button(t("Edit"))
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

    let top = grid((
        hstack((
            dot(status, 12.0),
            text_block(prof.name.clone()).font_size(16.0).semibold(),
        ))
        .spacing(10.0)
        .vertical_alignment(VerticalAlignment::Center)
        .grid_column(0),
        hstack((edit, del, action))
            .spacing(10.0)
            .vertical_alignment(VerticalAlignment::Center)
            .grid_column(1),
    ))
    .columns([GridLength::Star(1.0), GridLength::Auto]);

    let info = hstack((
        text_block(t(status.label()))
            .font_size(12.0)
            .foreground(color_for(status)),
        text_block(format!("{}  {}", t("Network"), prof.network_name))
            .font_size(12.0)
            .opacity(0.65),
        text_block(format!("IP  {ip}"))
            .font_size(12.0)
            .opacity(0.65),
        text_block(format!("{}  {peers}", t("Peers")))
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
            InfoBar::new(t("Connection problem"))
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
            button(t("Disconnect"))
                .on_click(move || e.stop(id.clone()))
                .min_width(140.0)
                .grid_column(2)
                .into()
        } else {
            button(t("Connect"))
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
        NavViewItem::new(t("General")).tag("general"),
        NavViewItem::new(t("Security")).tag("security"),
        NavViewItem::new(t("Routing")).tag("routing"),
        NavViewItem::new(t("Proxies")).tag("proxies"),
        NavViewItem::new(t("Advanced")).tag("advanced"),
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
            text_block(t("Network")).font_size(28.0).bold(),
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

// ─────────────────────────── Network → General ────────────────────────────

fn general_panel(ctx: &PageCtx, p: &Profile) -> Element {
    let profile_card = card(
        "This profile",
        vstack((
            text_box(p.name.clone())
                .header(t("Profile name (shown in Polaris only)"))
                .placeholder(t("My network"))
                .on_changed(on_edit!(ctx, |p, v: String| p.name = v)),
            hstack((duplicate_button(ctx), export_button(p))).spacing(10.0),
        ))
        .spacing(14.0)
        .into(),
    );

    let identity = card(
        "Identity",
        vstack((
            text_box(p.network_name.clone())
                .header(t("Network name (shared with peers)"))
                .placeholder(t("e.g. home-lab"))
                .on_changed(on_edit!(ctx, |p, v: String| p.network_name = v)),
            PasswordBox::new()
                .value(p.network_secret.clone())
                .header(t("Network secret (shared password)"))
                .placeholder(t("everyone on the network uses this"))
                .on_changed(on_edit!(ctx, |p, v: String| p.network_secret = v)),
        ))
        .spacing(14.0)
        .into(),
    );

    let device = card(
        "This device",
        vstack((
            text_box(p.hostname.clone())
                .header(t("Display name (optional)"))
                .placeholder(t("defaults to the system hostname"))
                .on_changed(on_edit!(ctx, |p, v: String| p.hostname = v)),
            text_box(p.dev_name.clone())
                .header(t("TUN device name (optional)"))
                .placeholder(t("leave blank for the system default"))
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
            .header(t("How to join"))
            .selected_index(p.join_method.index())
            .on_selection_changed(
                on_edit!(ctx, |p, v: i32| p.join_method = JoinMethod::from_index(v))
            )
            .into(),
    ];
    match p.join_method {
        JoinMethod::PublicServer => conn.push(
            text_box(p.public_server.clone())
                .header(t("Server address"))
                .placeholder("tcp://public.easytier.cn:11010")
                .on_changed(on_edit!(ctx, |p, v: String| p.public_server = v))
                .into(),
        ),
        JoinMethod::Manual => conn.push(
            text_box(p.peers.join("\n"))
                .header(t("Peer addresses (one per line)"))
                .placeholder("tcp://192.0.2.10:11010")
                .multiline()
                .height(110.0)
                .on_changed(on_edit!(ctx, |p, v: String| p.peers = split_lines(&v)))
                .into(),
        ),
        JoinMethod::Standalone => conn.push(
            text_block(t(
                "Standalone — other devices connect to this node's listeners \
                 (set on the Advanced tab).",
            ))
            .font_size(12.0)
            .opacity(0.7)
            .wrap()
            .into(),
        ),
    }
    let connection = card("Connection", vstack(conn).spacing(14.0).into());

    let mut addr: Vec<Element> = vec![switch_row(
        "Automatic IP (DHCP)",
        "Get a virtual IP from the network automatically. Turn off to assign a fixed address.",
        p.dhcp,
        on_edit!(ctx, |p, v: bool| p.dhcp = v),
    )];
    if !p.dhcp {
        addr.push(
            grid((
                text_box(p.virtual_ipv4.clone())
                    .header(t("Virtual IPv4"))
                    .placeholder("10.126.126.1")
                    .on_changed(on_edit!(ctx, |p, v: String| p.virtual_ipv4 = v))
                    .grid_column(0),
                NumberBox::new(p.network_length as f64)
                    .header(t("Prefix length"))
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
    let addressing = card("Addressing", vstack(addr).spacing(14.0).into());

    vstack((profile_card, identity, device, connection, addressing))
        .spacing(18.0)
        .into()
}

// ─────────────────────── Network-list action buttons ──────────────────────

fn add_network_button(ctx: &PageCtx) -> Button {
    let set = ctx.set_store.clone();
    let store = ctx.store.clone();
    button(t("Add network"))
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
    button(t("Import…"))
        .icon(SymbolGlyph::Download)
        .on_click(move || {
            let f_cfg = t("EasyTier configs (*.toml, *.json)");
            let f_all = t("All files");
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
    button(t("Paste config"))
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
    button(t("Duplicate"))
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

fn export_button(p: &Profile) -> Button {
    let p = p.clone();
    button(t("Export…"))
        .icon(SymbolGlyph::Upload)
        .on_click(move || {
            let default = format!("{}.toml", sanitize(&p.name));
            let f_toml = t("EasyTier config (*.toml)");
            let f_json = t("NetworkConfig JSON (*.json)");
            if let Some(path) = crate::dialog::save_file(
                &default,
                &[
                    (f_toml.as_str(), "*.toml"),
                    (f_json.as_str(), "*.json"),
                ],
            ) {
                let is_json = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.eq_ignore_ascii_case("json"))
                    .unwrap_or(false);
                let content = if is_json {
                    profile_to_json(&p)
                } else {
                    profile_to_toml(&p)
                };
                if let Ok(text) = content {
                    let _ = std::fs::write(&path, text);
                }
            }
        })
}

/// Delete the profile at `idx`. Disabled when only one network remains.
fn delete_button(ctx: &PageCtx, idx: usize, prof_id: String) -> Button {
    let can_delete = ctx.store.profiles.len() > 1;
    let set = ctx.set_store.clone();
    let store = ctx.store.clone();
    let engine = ctx.engine.clone();
    button(t("Delete"))
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
        "Encryption",
        switch_row(
            "Encrypt traffic between peers",
            "AES-GCM by default. Only turn this off on a trusted LAN — without it, anyone who \
             captures a packet can read it.",
            p.enable_encryption,
            on_edit!(ctx, |p, v: bool| p.enable_encryption = v),
        ),
    );

    let access = card(
        "Access control",
        vstack((
            switch_row(
                "Private mode",
                "Reject peers whose network name does not match exactly. Without this, anyone \
                 who guesses the secret can join even with a different network name.",
                p.enable_private_mode,
                on_edit!(ctx, |p, v: bool| p.enable_private_mode = v),
            ),
            divider(),
            switch_row(
                "Only relay traffic for trusted networks",
                "When acting as a relay, refuse to forward packets for networks not in the list \
                 below.",
                p.enable_relay_network_whitelist,
                on_edit!(ctx, |p, v: bool| p.enable_relay_network_whitelist = v),
            ),
            // Whitelist editor is always present (greyed by EasyTier semantics when toggle is
            // off, but the field stays so the user can keep typing).
            text_box(p.relay_network_whitelist.join("\n"))
                .header(t("Allowed network names (one per line)"))
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
        "DNS",
        switch_row(
            "Magic DNS",
            "Resolve peer hostnames (e.g. `laptop.local`) inside this network without any DNS \
             server.",
            p.enable_magic_dns,
            on_edit!(ctx, |p, v: bool| p.enable_magic_dns = v),
        ),
    );

    vstack((crypto, access, dns)).spacing(18.0).into()
}

// ─────────────────────────── Network → Routing ────────────────────────────

fn routing_panel(ctx: &PageCtx, p: &Profile) -> Element {
    let preference = card(
        "Path selection",
        switch_row(
            "Prefer the lowest-latency path",
            "When more than one route exists, pick the one with the shortest RTT instead of the \
             fewest hops.",
            p.latency_first,
            on_edit!(ctx, |p, v: bool| p.latency_first = v),
        ),
    );

    let subnet_proxy = card(
        "Subnet proxy",
        vstack((
            text_block(t(
                "Re-export local subnets to peers so they can reach hosts on your LAN as if \
                 those hosts were on the overlay network.",
            ))
            .font_size(12.0)
            .opacity(0.7)
            .wrap(),
            text_box(p.proxy_cidrs.join("\n"))
                .header(t("Shared subnets (CIDR, one per line)"))
                .placeholder("192.168.1.0/24")
                .multiline()
                .height(80.0)
                .on_changed(on_edit!(ctx, |p, v: String| p.proxy_cidrs = split_lines(&v))),
        ))
        .spacing(10.0)
        .into(),
    );

    let manual_routes = card(
        "Manual routes",
        vstack((
            switch_row(
                "Announce custom routes",
                "Advertise additional CIDRs to peers; they will route traffic to those ranges \
                 through this node.",
                p.enable_manual_routes,
                on_edit!(ctx, |p, v: bool| p.enable_manual_routes = v),
            ),
            text_box(p.routes.join("\n"))
                .header(t("Routes (one CIDR per line)"))
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
        "Exit nodes",
        vstack((
            switch_row(
                "Act as an exit node for this network",
                "Forward arbitrary internet traffic from other peers through this device.",
                p.enable_exit_node,
                on_edit!(ctx, |p, v: bool| p.enable_exit_node = v),
            ),
            divider(),
            text_block(t(
                "Send this device's internet traffic through one of these peers (use their \
                 virtual IPs):",
            ))
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
            "Run a SOCKS5 proxy on this device",
            "Other apps on this machine can route traffic through the EasyTier network by \
             pointing at 127.0.0.1:<port>.",
            p.enable_socks5,
            on_edit!(ctx, |p, v: bool| p.enable_socks5 = v),
        )];
        body.push(
            NumberBox::new(p.socks5_port as f64)
                .header(t("Listen port"))
                .range(1.0, 65535.0)
                .enabled(p.enable_socks5)
                .on_value_changed(on_edit!(ctx, |p, v: f64| {
                    p.socks5_port = v.clamp(1.0, 65535.0) as u16
                }))
                .into(),
        );
        card("SOCKS5", vstack(body).spacing(14.0).into())
    };

    let portal = {
        let mut body: Vec<Element> = vec![
            switch_row(
                "Expose a built-in WireGuard server",
                "Lets a WireGuard client outside this network dial into the mesh.",
                p.enable_vpn_portal,
                on_edit!(ctx, |p, v: bool| p.enable_vpn_portal = v),
            ),
            grid((
                text_box(p.vpn_portal_client_network_addr.clone())
                    .header(t("Client subnet"))
                    .placeholder("10.14.14.0")
                    .enabled(p.enable_vpn_portal)
                    .on_changed(on_edit!(ctx, |p, v: String| {
                        p.vpn_portal_client_network_addr = v
                    }))
                    .grid_column(0),
                NumberBox::new(p.vpn_portal_client_network_len as f64)
                    .header(t("Prefix"))
                    .range(1.0, 32.0)
                    .enabled(p.enable_vpn_portal)
                    .on_value_changed(on_edit!(ctx, |p, v: f64| {
                        p.vpn_portal_client_network_len = v.clamp(1.0, 32.0) as u8
                    }))
                    .grid_column(1),
                NumberBox::new(p.vpn_portal_listen_port as f64)
                    .header(t("Listen port (UDP)"))
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
                    text_block(t(
                        "Connect this network to make Polaris publish a WireGuard client \
                             config here.",
                    ))
                    .font_size(12.0)
                    .opacity(0.6)
                    .into(),
                );
            }
        }
        card("VPN portal", vstack(body).spacing(14.0).into())
    };

    let forwards = port_forwards_card(ctx, p);

    vstack((socks, portal, forwards)).spacing(18.0).into()
}

/// Render the live WireGuard client config in a selectable text box.
fn vpn_portal_viewer(cfg: String) -> Element {
    let url = "https://www.wireguardconfig.com/qrcode";
    border(
        vstack((
            text_block(t("WireGuard client config (Ctrl+A then Ctrl+C to copy)"))
                .font_size(12.0)
                .semibold(),
            text_box(cfg)
                .multiline()
                .height(170.0)
                .font_family("Consolas")
                .font_size(12.0),
            hstack((
                text_block(t("Paste into any WireGuard client, or"))
                    .font_size(11.0)
                    .opacity(0.6)
                    .vertical_alignment(VerticalAlignment::Center),
                link("make a QR code", url),
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
        text_block(t("Forward TCP/UDP ports from this device into the mesh."))
            .font_size(12.0)
            .opacity(0.7)
            .wrap()
            .grid_column(0),
        button(t("Add forward"))
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
            text_block(t("No forwards configured."))
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

    card("Port forwards", vstack(rows).spacing(10.0).into())
}

fn port_forward_header() -> Element {
    hstack((
        cell(t("Proto"), 70.0, true, false),
        cell(t("Bind IP"), 140.0, true, false),
        cell(t("Bind port"), 95.0, true, false),
        cell(t("Destination IP"), 160.0, true, false),
        cell(t("Dest port"), 95.0, true, false),
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
        "Listeners",
        vstack((
            text_block(t(
                "URLs this node binds for inbound connections. Leave the first list empty for \
                 EasyTier's defaults (tcp/udp/wg on standard ports).",
            ))
            .font_size(12.0)
            .opacity(0.7)
            .wrap(),
            text_box(p.listeners.join("\n"))
                .header(t("Listener URLs (one per line)"))
                .placeholder("tcp://0.0.0.0:11010\nudp://0.0.0.0:11010\nwg://0.0.0.0:11011")
                .multiline()
                .height(96.0)
                .on_changed(on_edit!(ctx, |p, v: String| p.listeners = split_lines(&v))),
            text_box(p.mapped_listeners.join("\n"))
                .header(t("Mapped listeners (public URLs other peers should dial)"))
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
        "Limits",
        grid((
            NumberBox::new(p.mtu as f64)
                .header(t("MTU (0 = default)"))
                .range(0.0, 1500.0)
                .on_value_changed(on_edit!(ctx, |p, v: f64| p.mtu = v.clamp(0.0, 1500.0) as u32))
                .grid_column(0),
            NumberBox::new(p.instance_recv_bps_limit as f64)
                .header(t("Receive bandwidth limit (bps; 0 = unlimited)"))
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
        "Transport",
        flag_grid(vec![
            flag(
                "KCP proxy",
                "Wrap proxied TCP in KCP (TCP-over-UDP). Smoother over lossy links.",
                p.enable_kcp_proxy,
                on_edit!(ctx, |p, v: bool| p.enable_kcp_proxy = v),
            ),
            flag(
                "Refuse inbound KCP",
                "Don't accept incoming KCP proxies from other peers.",
                p.disable_kcp_input,
                on_edit!(ctx, |p, v: bool| p.disable_kcp_input = v),
            ),
            flag(
                "QUIC proxy",
                "Wrap proxied TCP in QUIC — useful behind some firewalls.",
                p.enable_quic_proxy,
                on_edit!(ctx, |p, v: bool| p.enable_quic_proxy = v),
            ),
            flag(
                "Refuse inbound QUIC",
                "Don't accept incoming QUIC proxy sessions.",
                p.disable_quic_input,
                on_edit!(ctx, |p, v: bool| p.disable_quic_input = v),
            ),
            flag(
                "smoltcp stack",
                "Bypass the OS TCP stack. Required on devices without TUN.",
                p.use_smoltcp,
                on_edit!(ctx, |p, v: bool| p.use_smoltcp = v),
            ),
            flag(
                "Disable IPv6",
                "Use IPv4 only. Try this if v6 paths are flaky on your network.",
                p.disable_ipv6,
                on_edit!(ctx, |p, v: bool| p.disable_ipv6 = v),
            ),
            flag(
                "Auto public IPv6",
                "Ask STUN for this device's public v6 address.",
                p.ipv6_public_addr_auto,
                on_edit!(ctx, |p, v: bool| p.ipv6_public_addr_auto = v),
            ),
            flag(
                "Relay UDP broadcasts",
                "Reflect UDP broadcast/multicast (e.g. mDNS) across the mesh.",
                p.enable_udp_broadcast_relay,
                on_edit!(ctx, |p, v: bool| p.enable_udp_broadcast_relay = v),
            ),
        ]),
    );

    let p2p = card(
        "Peer-to-peer & NAT",
        flag_grid(vec![
            flag(
                "Disable direct P2P",
                "Always relay through a server. Slower but predictable.",
                p.disable_p2p,
                on_edit!(ctx, |p, v: bool| p.disable_p2p = v),
            ),
            flag(
                "P2P only",
                "Refuse to relay traffic between other peers.",
                p.p2p_only,
                on_edit!(ctx, |p, v: bool| p.p2p_only = v),
            ),
            flag(
                "Lazy P2P",
                "Only attempt P2P once real traffic starts.",
                p.lazy_p2p,
                on_edit!(ctx, |p, v: bool| p.lazy_p2p = v),
            ),
            flag(
                "Require P2P",
                "Refuse to start if P2P could not be established.",
                p.need_p2p,
                on_edit!(ctx, |p, v: bool| p.need_p2p = v),
            ),
            flag(
                "Skip TCP hole-punching",
                "Don't try TCP NAT traversal.",
                p.disable_tcp_hole_punching,
                on_edit!(ctx, |p, v: bool| p.disable_tcp_hole_punching = v),
            ),
            flag(
                "Skip UDP hole-punching",
                "Don't try UDP NAT traversal.",
                p.disable_udp_hole_punching,
                on_edit!(ctx, |p, v: bool| p.disable_udp_hole_punching = v),
            ),
            flag(
                "Skip symmetric-NAT punch",
                "Skip the specialized symmetric-NAT punching algorithm.",
                p.disable_sym_hole_punching,
                on_edit!(ctx, |p, v: bool| p.disable_sym_hole_punching = v),
            ),
            flag(
                "Disable UPnP",
                "Don't auto-open ports on your router.",
                p.disable_upnp,
                on_edit!(ctx, |p, v: bool| p.disable_upnp = v),
            ),
        ]),
    );

    let instance = card(
        "Instance",
        flag_grid(vec![
            flag(
                "Relay peer RPCs",
                "Forward control-plane messages between peers (needed on relay servers).",
                p.relay_all_peer_rpc,
                on_edit!(ctx, |p, v: bool| p.relay_all_peer_rpc = v),
            ),
            flag(
                "Multi-threaded runtime",
                "Use the full Tokio pool. Disable on single-core devices.",
                p.multi_thread,
                on_edit!(ctx, |p, v: bool| p.multi_thread = v),
            ),
            flag(
                "Forward proxy via OS",
                "Let the kernel route proxied subnets instead of user-space.",
                p.proxy_forward_by_system,
                on_edit!(ctx, |p, v: bool| p.proxy_forward_by_system = v),
            ),
            flag(
                "Bind to a single NIC",
                "Pin outbound traffic to one interface. Required on iOS-like systems.",
                p.bind_device,
                on_edit!(ctx, |p, v: bool| p.bind_device = v),
            ),
            flag(
                "No TUN device",
                "Run without a virtual NIC — proxies only (SOCKS5 / port forwards).",
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
            "Peers",
            "Devices reachable on your networks.",
            vec![card(
                "",
                InfoBar::new(t("Nothing here yet"))
                    .message(t("Connect a network to discover its peers."))
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
            &format!("{}  ({})", prof.name, tn("{n} peers", net.peers.len())),
            vstack((my_node_chips(net), peer_table(&net.peers)))
                .spacing(14.0)
                .into(),
        ));
    }

    page("Peers", "Devices reachable on your networks.", sections)
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

    push("ID", &net.peer_id.to_string());
    push("Virtual IP", &net.virtual_ip);
    push("TUN", &net.dev_name);
    push("NAT", &net.nat_type);
    push("Public v4", &net.public_ipv4);
    push("Public v6", &net.public_ipv6);
    push("Version", &net.version);
    for (i, l) in net.listeners.iter().enumerate() {
        push(&tn("Listener {n}", i + 1), l);
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
        "Device",
        "Virtual IP",
        "Route",
        "Latency",
        "Tunnel",
        "NAT",
        "Down",
        "Up",
        "Loss",
        "Version",
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
                cell(t(&p.cost), widths[2], false, p.cost != "Direct"),
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
            "Activity",
            "Live events from each running network.",
            vec![card(
                "",
                text_block(t("No activity yet. Connect a network to see its events."))
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
            &format!("{}  ({})", prof.name, tn("{n} events", net.events.len())),
            scroll_view(vstack(lines).spacing(4.0))
                .max_height(320.0)
                .into(),
        ));
    }

    page(
        "Activity",
        "Live events from each running network.",
        sections,
    )
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
        "Appearance",
        vstack((
            ComboBox::new(theme_labels)
                .header(t("Theme"))
                .selected_index(s.theme.index())
                .on_selection_changed(
                    on_setting!(ctx, |st, v: i32| st.theme = Theme::from_index(v))
                ),
            ComboBox::new(material_labels)
                .header(t("Window material"))
                .selected_index(s.material.index())
                .on_selection_changed(
                    on_setting!(ctx, |st, v: i32| st.material = Material::from_index(v))
                ),
        ))
        .spacing(14.0)
        .into(),
    );

    let behavior = card(
        "Behaviour",
        switch_row(
            "Connect the selected network on launch",
            "Polaris will start your last-edited network as soon as it opens.",
            s.auto_connect,
            on_setting!(ctx, |st, v: bool| st.auto_connect = v),
        ),
    );

    let tray = card(
        "System tray",
        vstack((
            switch_row(
                "Close button minimizes to tray",
                "Closing the window keeps Polaris running in the notification area. Use the tray \
                 icon's Quit to exit completely.",
                s.close_to_tray,
                on_setting!(ctx, |st, v: bool| st.close_to_tray = v),
            ),
            divider(),
            switch_row(
                "Minimize button minimizes to tray",
                "Hide Polaris to the tray instead of the taskbar when you minimize it.",
                s.minimize_to_tray,
                on_setting!(ctx, |st, v: bool| st.minimize_to_tray = v),
            ),
        ))
        .spacing(14.0)
        .into(),
    );

    let language_labels: Vec<String> = Language::ALL.iter().map(|l| l.label()).collect();
    let language = card(
        "Language",
        ComboBox::new(language_labels)
            .header(t("Language"))
            .selected_index(s.language.index())
            .on_selection_changed(on_setting!(ctx, |st, v: i32| st.language =
                Language::from_index(v)))
            .into(),
    );

    let mut cards = vec![appearance, language, behavior, tray];
    cards.push(diagnostics_card(ctx));
    cards.push(admin_card(ctx));

    page(
        "Settings",
        "Application-wide preferences. Network management lives on the Home page.",
        cards,
    )
}

fn admin_card(ctx: &PageCtx) -> Element {
    let elevated = crate::elevate::is_elevated();
    let status = if elevated {
        t("Running as administrator — VPN (TUN) is available.")
    } else {
        t("Not elevated — VPN mode is disabled; SOCKS5 and port-forward proxies still work.")
    };

    // A packaged (MSIX) build elevates through its manifest (allowElevation +
    // highestAvailable, `--features msix`), not a UAC relaunch — so the launch
    // toggle and on-demand restart don't apply. Show read-only status instead.
    if crate::elevate::is_packaged() {
        let detail = if elevated {
            status.clone()
        } else {
            t("Not elevated — VPN mode is disabled; SOCKS5 and port-forward proxies still \
               work. Launch Polaris as an administrator to enable the VPN (requires \
               Windows 11).")
        };
        return card(
            "Administrator & VPN",
            text_block(detail)
                .font_size(12.0)
                .opacity(0.6)
                .wrap()
                .into(),
        );
    }

    let mut body: Vec<Element> = vec![
        switch_row(
            "Always launch as administrator",
            "Request UAC elevation on startup so Polaris can create the VPN (TUN) adapter. \
             Takes effect next launch.",
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
            button(t("Restart as administrator now"))
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

    card("Administrator & VPN", vstack(body).spacing(14.0).into())
}

// ───────────────────────────────── About ──────────────────────────────────

pub fn about_page() -> Element {
    let about = card(
        "Polaris",
        vstack((
            text_block(t("A fast, friendly WinUI client for the EasyTier mesh VPN.")).font_size(14.0),
            text_block(format!("{} {}", t("Version"), env!("CARGO_PKG_VERSION")))
                .font_size(12.0)
                .opacity(0.7),
            text_block(format!("{} {}", t("EasyTier engine"), easytier::VERSION))
                .font_size(12.0)
                .opacity(0.7),
            text_block(t("Built with windows-reactor (WinUI 3) and the embedded EasyTier core."))
                .font_size(12.0)
                .opacity(0.7),
        ))
        .spacing(6.0)
        .into(),
    );

    let links = card(
        "Learn more",
        vstack((
            link("Polaris on GitHub", "https://github.com/l5z12/polaris_et"),
            link("EasyTier website", "https://easytier.cn"),
            link("EasyTier on GitHub", "https://github.com/EasyTier/EasyTier"),
            link(
                "windows-reactor (windows-rs)",
                "https://github.com/microsoft/windows-rs",
            ),
        ))
        .spacing(2.0)
        .into(),
    );

    let credits = card(
        "Credits",
        vstack((
            text_block(t("App icon from Fluent UI System Icons — © Microsoft, MIT License."))
                .font_size(12.0)
                .opacity(0.7)
                .wrap(),
            link(
                "github.com/microsoft/fluentui-system-icons",
                "https://github.com/microsoft/fluentui-system-icons",
            ),
        ))
        .spacing(6.0)
        .into(),
    );

    let license = card(
        "License",
        vstack((
            text_block(t("© 2026 Polaris contributors"))
                .font_size(12.0)
                .opacity(0.7),
            text_block(t(
                "Polaris is free software under the GNU General Public License, version 3, \
                 and comes with ABSOLUTELY NO WARRANTY.",
            ))
            .font_size(12.0)
            .opacity(0.7)
            .wrap(),
            link("GNU GPL v3.0", "https://www.gnu.org/licenses/gpl-3.0.html"),
            link(
                "Polaris source code (GitHub)",
                "https://github.com/l5z12/polaris_et",
            ),
        ))
        .spacing(6.0)
        .into(),
    );

    page(
        "About",
        "What powers Polaris.",
        vec![about, links, credits, license],
    )
}

// ───────────────────────────── Diagnostics ────────────────────────────────

/// Retention presets (days) for the log-cleanup dropdown.
const RETENTION_DAYS: [u32; 5] = [1, 3, 7, 14, 30];

fn retention_index(days: u32) -> i32 {
    RETENTION_DAYS.iter().position(|d| *d == days).unwrap_or(2) as i32
}

fn yn(b: bool) -> &'static str {
    if b { "yes" } else { "no" }
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
    let f_text = t("Text");
    let f_log = t("Log");
    let f_all = t("All files");
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
        "Verbose engine logging",
        "Capture verbose logs from Polaris and the EasyTier engine to a file and a Diagnostics \
         page — for deep debugging (e.g. why peers stay on relay vs. going P2P). Separate from \
         and off by default: per-network activity (peer connects, route changes) is always on \
         the Activity page without this.",
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
            .map(|d| tn("{n} days", *d as usize))
            .collect();

        body.push(divider());
        body.push(
            ComboBox::new(level_labels)
                .header(t("Log level"))
                .selected_index(s.log_level.index())
                .on_selection_changed(
                    on_setting!(ctx, |st, v: i32| st.log_level = LogLevel::from_index(v))
                )
                .into(),
        );
        body.push(
            ComboBox::new(retention_labels)
                .header(t("Delete logs older than"))
                .selected_index(retention_index(s.log_retention_days))
                .on_selection_changed(on_setting!(ctx, |st, v: i32| {
                    let i = (v.max(0) as usize).min(RETENTION_DAYS.len() - 1);
                    st.log_retention_days = RETENTION_DAYS[i];
                }))
                .into(),
        );
        body.push(
            hstack((
                button(t("Export logs…")).on_click(export_logs),
                button(t("Open logs folder")).on_click(open_logs_folder),
            ))
            .spacing(8.0)
            .into(),
        );
        body.push(
            text_block(format!("{}: {}", t("Logs"), crate::logging::logs_dir().display()))
                .font_size(11.0)
                .opacity(0.55)
                .wrap()
                .into(),
        );
    }

    card("Diagnostics", vstack(body).spacing(14.0).into())
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
        "Environment",
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
                t("Elevated"),
                t(yn(crate::elevate::is_elevated())),
                t("Packaged"),
                t(yn(crate::elevate::is_packaged())),
                t("Active networks"),
                active,
            ))
            .font_size(12.0)
            .opacity(0.7),
            text_block(format!(
                "{}: {}   ·   {}",
                t("Log level"),
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
        text_block(t(
            "No log output yet. Lower the log level (Settings → Diagnostics) and use the app — \
             engine events stream in here.",
        ))
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
        button(t("Export logs…")).on_click(export_logs),
        button(t("Open logs folder")).on_click(open_logs_folder),
        button(t("Clear")).on_click(crate::logging::clear),
    ))
    .spacing(8.0)
    .into();

    let logs = card(
        &format!("{}  ({})", t("Live log"), tn("{n} lines", lines.len())),
        vstack(vec![toolbar, log_body]).spacing(12.0).into(),
    );

    page(
        "Diagnostics",
        "Logs from Polaris and the embedded EasyTier engine.",
        vec![info, logs],
    )
}
