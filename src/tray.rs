// SPDX-License-Identifier: GPL-3.0-only
//! Windows notification-area (system tray) icon.
//!
//! Runs on its own dedicated thread with a hidden message window and a private
//! message pump, fully isolated from the WinUI render loop. It shares the
//! [`Engine`] (which is `Send` + thread-safe) so the tooltip can show live
//! connection status, and exposes a right-click menu (Show / Quit) plus
//! left-click / double-click to bring the main window forward.

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};

use windows::Win32::Foundation::*;
use windows::Win32::System::Com::StructuredStorage::{PROPVARIANT, PropVariantClear};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::{
    AttachThreadInput, GetCurrentProcessId, GetCurrentThreadId,
};
use windows::Win32::System::Variant::VT_LPWSTR;
use windows::Win32::UI::Shell::PropertiesSystem::{IPropertyStore, SHGetPropertyStoreForWindow};
use windows::Win32::UI::Shell::{
    DefSubclassProc, NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIIF_INFO, NIM_ADD, NIM_DELETE,
    NIM_MODIFY, NOTIFYICONDATAW, SHStrDupW, SetWindowSubclass, Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{BOOL, GUID, PCWSTR, w};

use crate::engine::Engine;

/// Tray callback message (icon → our window). Must be in the WM_APP range.
const WM_TRAY: u32 = WM_APP + 1;
const ID_SHOW: u32 = 1;
const ID_QUIT: u32 = 2;
const TIP_TIMER: usize = 1;
const MAIN_SUBCLASS_ID: usize = 1;

/// Close/minimize behavior, mirrored from `Settings` (updated by the UI).
static CLOSE_TO_TRAY: AtomicBool = AtomicBool::new(true);
static MINIMIZE_TO_TRAY: AtomicBool = AtomicBool::new(false);
/// Whether the main window's close hook has been installed yet.
static HOOKED: AtomicBool = AtomicBool::new(false);
/// Whether the main window's taskbar/Alt-Tab icon has been set yet.
static ICON_SET: AtomicBool = AtomicBool::new(false);
/// Whether the "still running in the tray" hint has been shown this run.
static HINT_SHOWN: AtomicBool = AtomicBool::new(false);
/// Whether the OS has told us the session is ending (shutdown / restart /
/// logoff). Once set, the close hook stops diverting to the tray so the app
/// exits quietly instead of lingering and blocking the shutdown.
static SHUTTING_DOWN: AtomicBool = AtomicBool::new(false);
/// Launched at Windows sign-in: hide the window to the tray once it exists,
/// instead of showing it. Cleared after the one-shot hide so a later manual
/// open isn't re-hidden.
static START_HIDDEN: AtomicBool = AtomicBool::new(false);
/// HWND of the tray's hidden message window (for showing balloon hints).
static TRAY_HWND: AtomicIsize = AtomicIsize::new(0);

/// Mirror the close-to-tray setting (called from the UI thread).
pub fn set_close_to_tray(on: bool) {
    CLOSE_TO_TRAY.store(on, Ordering::Relaxed);
}

/// Mirror the minimize-to-tray setting (called from the UI thread).
pub fn set_minimize_to_tray(on: bool) {
    MINIMIZE_TO_TRAY.store(on, Ordering::Relaxed);
}

/// Request that the main window start hidden in the tray (autostart launch).
/// Acted on by [`ensure_window_hook`] once the window exists.
pub fn set_start_hidden() {
    START_HIDDEN.store(true, Ordering::Relaxed);
}

struct Tray {
    engine: Engine,
    nid: NOTIFYICONDATAW,
}

thread_local! {
    static TRAY: RefCell<Option<Tray>> = const { RefCell::new(None) };
}

/// Start the tray icon on a background thread. Returns immediately.
pub fn spawn(engine: Engine) {
    let _ = std::thread::Builder::new()
        .name("polaris-tray".into())
        .spawn(move || unsafe { run(engine) });
}

unsafe fn run(engine: Engine) {
    unsafe {
        let Ok(module) = GetModuleHandleW(None) else {
            return;
        };
        let hinstance = HINSTANCE(module.0);
        let class_name = w!("PolarisTrayWindow");

        let wc = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(wndproc),
            hInstance: hinstance,
            lpszClassName: class_name,
            ..Default::default()
        };
        if RegisterClassExW(&wc) == 0 {
            return;
        }

        // A normal (but never-shown) top-level window. It must be a real window
        // — not message-only — so it can become foreground for the popup menu.
        let Ok(hwnd) = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class_name,
            w!("Polaris Tray"),
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            None,
            None,
            Some(hinstance),
            None,
        ) else {
            return;
        };

        TRAY_HWND.store(hwnd.0 as isize, Ordering::Relaxed);

        // The application icon embedded by build.rs (resource id 1), falling
        // back to the generic icon if it can't be loaded.
        let hicon = LoadIconW(Some(hinstance), app_icon_id())
            .or_else(|_| LoadIconW(None, IDI_APPLICATION))
            .unwrap_or_default();

        let mut nid = NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: 1,
            uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
            uCallbackMessage: WM_TRAY,
            hIcon: hicon,
            ..Default::default()
        };
        write_wide(&mut nid.szTip, &tooltip(&engine));
        let _ = Shell_NotifyIconW(NIM_ADD, &nid);

        TRAY.with(|t| *t.borrow_mut() = Some(Tray { engine, nid }));

        // Refresh the tooltip with live status every few seconds.
        let _ = SetTimer(Some(hwnd), TIP_TIMER, 3000, None);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_TRAY => {
                // Classic mode: the mouse message is the low word of lParam.
                match (lparam.0 as u32) & 0xFFFF {
                    WM_LBUTTONUP | WM_LBUTTONDBLCLK => show_main_window(),
                    WM_RBUTTONUP | WM_CONTEXTMENU => show_menu(hwnd),
                    _ => {}
                }
                LRESULT(0)
            }
            WM_COMMAND => {
                match (wparam.0 as u32) & 0xFFFF {
                    ID_SHOW => show_main_window(),
                    ID_QUIT => quit(),
                    _ => {}
                }
                LRESULT(0)
            }
            WM_TIMER => {
                refresh_tooltip();
                LRESULT(0)
            }
            WM_DESTROY => {
                remove_icon();
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

unsafe fn show_menu(hwnd: HWND) {
    unsafe {
        let Ok(menu) = CreatePopupMenu() else {
            return;
        };
        // Built at runtime so they follow the current UI language. AppendMenuW
        // copies the string, so the temporaries can drop right after.
        let show = wide(&crate::i18n::t("tray.show_polaris"));
        let quit = wide(&crate::i18n::t("tray.quit"));
        let _ = AppendMenuW(menu, MF_STRING, ID_SHOW as usize, PCWSTR(show.as_ptr()));
        let _ = AppendMenuW(menu, MF_STRING, ID_QUIT as usize, PCWSTR(quit.as_ptr()));

        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        // Required so the menu dismisses correctly when clicking elsewhere.
        let _ = SetForegroundWindow(hwnd);
        let _ = TrackPopupMenu(
            menu,
            TPM_RIGHTBUTTON | TPM_BOTTOMALIGN,
            pt.x,
            pt.y,
            Some(0),
            hwnd,
            None,
        );
        let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(menu);
    }
}

unsafe fn show_main_window() {
    let Some(hwnd) = find_main_window() else {
        return;
    };
    unsafe {
        // Un-hide (close / minimize-to-tray uses SW_HIDE), then un-minimize.
        let _ = ShowWindow(hwnd, SW_SHOW);
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }
        force_foreground(hwnd);
    }
}

/// Bring `hwnd` to the foreground. `SetForegroundWindow` is normally refused
/// for a background thread (like the tray), so we briefly attach our input
/// queue to the current foreground thread's, which lifts the restriction.
unsafe fn force_foreground(hwnd: HWND) {
    unsafe {
        let fg_thread = GetWindowThreadProcessId(GetForegroundWindow(), None);
        let this_thread = GetCurrentThreadId();
        if fg_thread != 0 && fg_thread != this_thread {
            let _ = AttachThreadInput(this_thread, fg_thread, true);
            let _ = SetForegroundWindow(hwnd);
            let _ = BringWindowToTop(hwnd);
            let _ = AttachThreadInput(this_thread, fg_thread, false);
        } else {
            let _ = SetForegroundWindow(hwnd);
        }
    }
}

/// Find the app's main window by process ownership (title matching is
/// unreliable for WinUI windows). Returns the first top-level, captioned
/// window owned by this process that isn't the tray's own hidden window.
fn find_main_window() -> Option<HWND> {
    let mut found: isize = 0;
    unsafe {
        let _ = EnumWindows(Some(enum_main), LPARAM(&mut found as *mut isize as isize));
    }
    (found != 0).then_some(HWND(found as *mut _))
}

unsafe extern "system" fn enum_main(hwnd: HWND, lparam: LPARAM) -> BOOL {
    unsafe {
        let mut pid = 0u32;
        let _ = GetWindowThreadProcessId(hwnd, Some(&mut pid));
        let is_tray = hwnd.0 as isize == TRAY_HWND.load(Ordering::Relaxed);
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
        let has_caption = style & WS_CAPTION.0 == WS_CAPTION.0;
        if pid == GetCurrentProcessId() && !is_tray && has_caption {
            *(lparam.0 as *mut isize) = hwnd.0 as isize;
            return BOOL(0); // found — stop enumerating
        }
        BOOL(1) // keep going
    }
}

unsafe fn refresh_tooltip() {
    unsafe {
        TRAY.with(|t| {
            if let Some(tray) = t.borrow_mut().as_mut() {
                let tip = tooltip(&tray.engine);
                write_wide(&mut tray.nid.szTip, &tip);
                let _ = Shell_NotifyIconW(NIM_MODIFY, &tray.nid);
            }
        });
    }
}

unsafe fn remove_icon() {
    unsafe {
        TRAY.with(|t| {
            if let Some(tray) = t.borrow().as_ref() {
                let _ = Shell_NotifyIconW(NIM_DELETE, &tray.nid);
            }
        });
    }
}

unsafe fn quit() {
    unsafe { remove_icon() };
    std::process::exit(0);
}

fn tooltip(engine: &Engine) -> String {
    format!("Polaris — {}", engine.snapshot().status_summary())
}

/// Build a null-terminated UTF-16 buffer for a Win32 `PCWSTR` argument.
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// `MAKEINTRESOURCEW(1)` — the app icon embedded by build.rs at resource id 1.
///
/// A resource id is passed as a pseudo-pointer whose low word holds the id; it
/// is never dereferenced. NOTE: `ptr::dangling::<u16>()` is **not** id 1 — it
/// yields the alignment of `u16` (2), so it requested a nonexistent resource and
/// Windows fell back to the generic icon.
fn app_icon_id() -> PCWSTR {
    PCWSTR(std::ptr::without_provenance::<u16>(1))
}

/// Copy `s` (UTF-16, truncated, null-terminated) into a fixed wide buffer.
fn write_wide(buf: &mut [u16], s: &str) {
    let max = buf.len().saturating_sub(1);
    let mut i = 0;
    for unit in s.encode_utf16() {
        if i >= max {
            break;
        }
        buf[i] = unit;
        i += 1;
    }
    for slot in &mut buf[i..] {
        *slot = 0;
    }
}

// ───────────────────────── close / minimize to tray ───────────────────────

/// Install the main-window close hook, once the window exists. Idempotent and
/// cheap after it succeeds — safe to call repeatedly (e.g. from the repaint
/// tick) since the window may not exist on the very first render.
///
/// Must run on the UI thread (the thread that owns the main window).
pub fn ensure_window_hook() {
    maybe_hide_on_startup();
    if HOOKED.load(Ordering::Relaxed) {
        return;
    }
    let Some(hwnd) = find_main_window() else {
        return;
    };
    // A WinUI 3 window doesn't inherit the exe's icon for its taskbar / Alt-Tab
    // / window-preview icon — that has to be set explicitly via WM_SETICON.
    if !ICON_SET.swap(true, Ordering::Relaxed) {
        unsafe { set_window_icon(hwnd) };
        unsafe { set_taskbar_identity(hwnd) };
    }
    unsafe {
        if SetWindowSubclass(hwnd, Some(subclass_proc), MAIN_SUBCLASS_ID, 0).as_bool() {
            HOOKED.store(true, Ordering::Relaxed);
        }
    }
}

/// One-shot for an autostart launch: hide the window to the tray as soon as it
/// appears. Runs on each repaint tick (the window may not exist on the first
/// few) until it succeeds, then clears the flag so a later manual reopen isn't
/// hidden again.
fn maybe_hide_on_startup() {
    if !START_HIDDEN.load(Ordering::Relaxed) {
        return;
    }
    if let Some(hwnd) = find_main_window() {
        unsafe {
            let _ = ShowWindow(hwnd, SW_HIDE);
        }
        START_HIDDEN.store(false, Ordering::Relaxed);
    }
}

/// Set the main window's small + large icons to the embedded app icon
/// (resource id 1), so the taskbar and Alt-Tab show it.
unsafe fn set_window_icon(hwnd: HWND) {
    unsafe {
        let Ok(module) = GetModuleHandleW(None) else {
            return;
        };
        let hinst = HINSTANCE(module.0);
        let id = app_icon_id();
        if let Ok(small) = LoadImageW(Some(hinst), id, IMAGE_ICON, 16, 16, LR_DEFAULTCOLOR) {
            let _ = SendMessageW(
                hwnd,
                WM_SETICON,
                Some(WPARAM(0)),
                Some(LPARAM(small.0 as isize)),
            ); // ICON_SMALL
        }
        if let Ok(big) = LoadImageW(Some(hinst), id, IMAGE_ICON, 32, 32, LR_DEFAULTCOLOR) {
            let _ = SendMessageW(
                hwnd,
                WM_SETICON,
                Some(WPARAM(1)),
                Some(LPARAM(big.0 as isize)),
            ); // ICON_BIG
        }
    }
}

/// `{9F4C2855-9F79-4B39-A8D0-E1D42DE1D5F3}` — the AppUserModel property fmtid.
const APPUSERMODEL_FMTID: GUID = GUID::from_u128(0x9F4C_2855_9F79_4B39_A8D0_E1D4_2DE1_D5F3);

/// Give the main window an explicit taskbar identity so the jump list / pinned
/// name reads "Polaris" instead of the exe filename. The shell reads these from
/// the *window's* property store (`SHGetPropertyStoreForWindow`); a process-wide
/// `SetCurrentProcessExplicitAppUserModelID` doesn't populate them, and a WinUI 3
/// window doesn't pick up the exe's FileDescription here.
unsafe fn set_taskbar_identity(hwnd: HWND) {
    unsafe {
        let Ok(store) = SHGetPropertyStoreForWindow::<IPropertyStore>(hwnd) else {
            return;
        };
        let exe = std::env::current_exe().unwrap_or_default();
        // pid 5 = ID, 2 = RelaunchCommand, 4 = RelaunchDisplayNameResource.
        set_str_prop(&store, 5, "Polaris.EasyTier");
        set_str_prop(&store, 2, &format!("\"{}\"", exe.display()));
        set_str_prop(&store, 4, "Polaris");
        let _ = store.Commit();
    }
}

/// Set one `VT_LPWSTR` AppUserModel property on the window's property store.
unsafe fn set_str_prop(store: &IPropertyStore, pid: u32, value: &str) {
    unsafe {
        let key = PROPERTYKEY {
            fmtid: APPUSERMODEL_FMTID,
            pid,
        };
        let wide: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
        let Ok(dup) = SHStrDupW(PCWSTR(wide.as_ptr())) else {
            return;
        };
        let mut pv = PROPVARIANT::default();
        let inner = &mut *pv.Anonymous.Anonymous;
        inner.vt = VT_LPWSTR;
        inner.Anonymous.pwszVal = dup;
        let _ = store.SetValue(&key, &pv);
        let _ = PropVariantClear(&mut pv); // frees `dup`
    }
}

/// Intercepts close / minimize on the main window to hide it to the tray.
unsafe extern "system" fn subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _ref: usize,
) -> LRESULT {
    unsafe {
        // A second instance asked us to come to the front.
        if msg == crate::instance::show_message() {
            show_main_window();
            return LRESULT(0);
        }
        match msg {
            // The OS is ending the session (shutdown / restart / logoff). Record
            // it so the close hook below stops hiding to the tray, then let the
            // default proc reply (it returns TRUE, allowing the session to end).
            WM_QUERYENDSESSION => {
                SHUTTING_DOWN.store(true, Ordering::Relaxed);
                DefSubclassProc(hwnd, msg, wparam, lparam)
            }
            // Session is really ending — exit quietly rather than lingering in
            // the tray and stalling the shutdown.
            WM_ENDSESSION if wparam.0 != 0 => {
                std::process::exit(0);
            }
            WM_CLOSE
                if CLOSE_TO_TRAY.load(Ordering::Relaxed)
                    && !SHUTTING_DOWN.load(Ordering::Relaxed) =>
            {
                hide_to_tray(hwnd);
                LRESULT(0)
            }
            WM_SYSCOMMAND
                if MINIMIZE_TO_TRAY.load(Ordering::Relaxed)
                    && (wparam.0 & 0xFFF0) == SC_MINIMIZE as usize =>
            {
                hide_to_tray(hwnd);
                LRESULT(0)
            }
            _ => DefSubclassProc(hwnd, msg, wparam, lparam),
        }
    }
}

unsafe fn hide_to_tray(hwnd: HWND) {
    unsafe {
        let _ = ShowWindow(hwnd, SW_HIDE);
    }
    // First time only: tell the user where the window went.
    if !HINT_SHOWN.swap(true, Ordering::Relaxed) {
        show_hint(
            &crate::i18n::t("tray.balloon_title"),
            &crate::i18n::t("tray.balloon_body"),
        );
    }
}

/// Show a one-shot balloon hint on the tray icon. Builds a fresh
/// `NOTIFYICONDATAW` pointing at the tray window (whose HWND is published in
/// [`TRAY_HWND`]), so it can be called from the UI thread.
fn show_hint(title: &str, body: &str) {
    let raw = TRAY_HWND.load(Ordering::Relaxed);
    if raw == 0 {
        return;
    }
    let mut nid = NOTIFYICONDATAW {
        cbSize: size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: HWND(raw as *mut _),
        uID: 1,
        uFlags: NIF_INFO,
        dwInfoFlags: NIIF_INFO,
        ..Default::default()
    };
    write_wide(&mut nid.szInfoTitle, title);
    write_wide(&mut nid.szInfo, body);
    unsafe {
        let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
    }
}
