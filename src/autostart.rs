//! "Run Polaris when Windows starts".
//!
//! Two mechanisms, chosen by how the app was installed:
//!
//!  - **Unpackaged** (Inno Setup installer / portable exe): a per-user value
//!    under `HKCU\…\CurrentVersion\Run` pointing at the exe. No admin needed.
//!    The command carries [`STARTUP_ARG`] so the launch starts hidden in the
//!    tray instead of popping the window open at every sign-in.
//!
//!  - **Packaged** (MSIX / Store): a declared `windows.startupTask` (see
//!    `packaging/AppxManifest.xml`), toggled through the WinRT `StartupTask`
//!    API. Windows owns the final say here — once the user disables the task in
//!    Settings ▸ Apps ▸ Startup, the app can no longer re-enable it.
//!
//! The OS entry is the single source of truth (autostart is machine-local, so
//! it is deliberately *not* part of the saved/exported `Settings`). The state is
//! queried once at [`init`] and cached so the settings toggle reads it cheaply
//! on every render.

use std::future::IntoFuture;
use std::sync::atomic::{AtomicU8, Ordering};

use windows::ApplicationModel::{StartupTask, StartupTaskState};
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_SZ, RRF_RT_REG_SZ, RegCloseKey, RegDeleteValueW,
    RegGetValueW, RegOpenKeyExW, RegSetValueExW,
};
use windows::core::{HSTRING, PCWSTR, w};

/// Run-key value name (unpackaged) and the `Run` subkey it lives under.
const RUN_VALUE: PCWSTR = w!("Polaris");
const RUN_SUBKEY: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
/// MSIX `StartupTask` id — MUST match `TaskId` in `packaging/AppxManifest.xml`.
const TASK_ID: &str = "PolarisStartup";

/// Passed on the autostart command line so the app starts hidden in the tray
/// rather than showing its window at sign-in.
pub const STARTUP_ARG: &str = "--startup";

// 0 = not yet queried, 1 = on, 2 = off.
static STATE: AtomicU8 = AtomicU8::new(0);

fn cache(on: bool) -> bool {
    STATE.store(if on { 1 } else { 2 }, Ordering::Relaxed);
    on
}

/// Query the OS once and cache the result. For the unpackaged path this also
/// refreshes a present Run entry to the current exe path, so updating or moving
/// the exe keeps autostart pointing at the right file. Call early in `main`,
/// after [`crate::elevate::init`].
pub fn init() {
    let on = if crate::elevate::is_packaged() {
        packaged_query()
    } else if reg_present() {
        reg_write(); // refresh the stored path
        true
    } else {
        false
    };
    cache(on);
}

/// Whether Polaris is set to launch at sign-in. Cheap (cached) — safe to call
/// on every render.
pub fn is_enabled() -> bool {
    match STATE.load(Ordering::Relaxed) {
        1 => true,
        2 => false,
        // Not initialized yet — query live and cache.
        _ => cache(if crate::elevate::is_packaged() {
            packaged_query()
        } else {
            reg_present()
        }),
    }
}

/// Turn launch-at-sign-in on or off. Returns the resulting state, which can
/// differ from `on` for a packaged build the user has disabled in Windows
/// Settings (the API refuses to re-enable it).
pub fn set_enabled(on: bool) -> bool {
    let effective = if crate::elevate::is_packaged() {
        packaged_set(on)
    } else {
        reg_set(on);
        on
    };
    cache(effective)
}

// ─────────────────────────────── registry ─────────────────────────────────

/// The autostart command: the quoted exe path plus [`STARTUP_ARG`].
fn run_command() -> Vec<u16> {
    let exe = std::env::current_exe().unwrap_or_default();
    format!("\"{}\" {STARTUP_ARG}", exe.display())
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect()
}

fn reg_set(on: bool) {
    if on {
        reg_write();
    } else {
        reg_delete();
    }
}

/// Open `HKCU\…\Run` for writing. The key always exists, so this only fails on
/// an unexpected access error.
fn open_run_key() -> Option<HKEY> {
    unsafe {
        let mut hkey = HKEY(std::ptr::null_mut());
        (RegOpenKeyExW(HKEY_CURRENT_USER, RUN_SUBKEY, None, KEY_SET_VALUE, &mut hkey)
            == ERROR_SUCCESS)
            .then_some(hkey)
    }
}

fn reg_write() {
    let Some(hkey) = open_run_key() else { return };
    unsafe {
        let data = run_command();
        let bytes = std::slice::from_raw_parts(data.as_ptr().cast::<u8>(), data.len() * 2);
        let _ = RegSetValueExW(hkey, RUN_VALUE, None, REG_SZ, Some(bytes));
        let _ = RegCloseKey(hkey);
    }
}

fn reg_delete() {
    let Some(hkey) = open_run_key() else { return };
    unsafe {
        // ERROR_FILE_NOT_FOUND when it was never set — harmless.
        let _ = RegDeleteValueW(hkey, RUN_VALUE);
        let _ = RegCloseKey(hkey);
    }
}

fn reg_present() -> bool {
    unsafe {
        let mut size = 0u32;
        RegGetValueW(
            HKEY_CURRENT_USER,
            RUN_SUBKEY,
            RUN_VALUE,
            RRF_RT_REG_SZ,
            None,
            None,
            Some(&mut size),
        ) == ERROR_SUCCESS
    }
}

// ───────────────────────────── MSIX StartupTask ────────────────────────────

/// Run a WinRT closure on a short-lived thread with COM initialized, so the
/// `StartupTask` factory and the awaited operations have an apartment regardless
/// of the caller's thread (the main thread, pre-WinUI, has none; the UI thread is
/// STA). The closure returns the effective enabled state.
fn winrt(f: impl FnOnce() -> bool + Send + 'static) -> bool {
    std::thread::spawn(move || unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let r = f();
        CoUninitialize();
        r
    })
    .join()
    .unwrap_or(false)
}

/// Block the current thread until a future (e.g. a WinRT `IAsyncOperation`)
/// completes. A minimal park/unpark executor — `windows-future` 0.3 dropped the
/// blocking `get()` accessor and exposes only `IntoFuture`, and these StartupTask
/// operations complete near-instantly, so a full async runtime would be overkill.
fn block_on<F: IntoFuture>(fut: F) -> F::Output {
    use std::sync::Arc;
    use std::task::{Context, Poll, Wake, Waker};

    struct ThreadWaker(std::thread::Thread);
    impl Wake for ThreadWaker {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
        fn wake_by_ref(self: &Arc<Self>) {
            self.0.unpark();
        }
    }

    let waker = Waker::from(Arc::new(ThreadWaker(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    let mut fut = std::pin::pin!(fut.into_future());
    loop {
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(v) => return v,
            Poll::Pending => std::thread::park(),
        }
    }
}

fn task() -> Option<StartupTask> {
    block_on(StartupTask::GetAsync(&HSTRING::from(TASK_ID)).ok()?).ok()
}

fn is_on(state: StartupTaskState) -> bool {
    state == StartupTaskState::Enabled || state == StartupTaskState::EnabledByPolicy
}

fn packaged_query() -> bool {
    winrt(|| task().and_then(|t| t.State().ok()).is_some_and(is_on))
}

fn packaged_set(on: bool) -> bool {
    winrt(move || {
        let Some(task) = task() else { return false };
        if on {
            task.RequestEnableAsync()
                .ok()
                .and_then(|op| block_on(op).ok())
                .is_some_and(is_on)
        } else {
            let _ = task.Disable();
            false
        }
    })
}
