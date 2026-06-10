//! Single-instance enforcement.
//!
//! A named mutex detects whether another Polaris is already running. A second
//! launch broadcasts a registered "show" message (handled by the first
//! instance's window subclass in [`crate::tray`]) so the existing window comes
//! to the front — then the second process exits.
//!
//! Existence is *peeked* (via `OpenMutexW`) before any elevation step, so that
//! merely re-launching to surface the window never triggers a UAC prompt, even
//! when "always launch as administrator" is enabled. The surviving instance
//! later *creates* and holds the mutex with [`acquire`].

use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, LPARAM, WPARAM};
use windows::Win32::System::Threading::{CreateMutexW, OpenMutexW, SYNCHRONIZATION_ACCESS_RIGHTS};
use windows::Win32::UI::WindowsAndMessaging::{
    HWND_BROADCAST, PostMessageW, RegisterWindowMessageW,
};
use windows::core::{PCWSTR, w};

const MUTEX_NAME: PCWSTR = w!("Polaris.EasyTier.SingleInstance");
const SYNCHRONIZE: SYNCHRONIZATION_ACCESS_RIGHTS = SYNCHRONIZATION_ACCESS_RIGHTS(0x0010_0000);

static SHOW_MSG: AtomicU32 = AtomicU32::new(0);

/// System-wide registered message id that asks a running instance to surface
/// its window. The same string maps to the same id in every process.
pub fn show_message() -> u32 {
    let cached = SHOW_MSG.load(Ordering::Relaxed);
    if cached != 0 {
        return cached;
    }
    let m = unsafe { RegisterWindowMessageW(w!("PolarisShowWindowMessage")) };
    SHOW_MSG.store(m, Ordering::Relaxed);
    m
}

/// `true` if another instance already holds the single-instance mutex. Does not
/// create anything, so it's safe to call before deciding whether to elevate.
pub fn exists() -> bool {
    unsafe {
        match OpenMutexW(SYNCHRONIZE, false, MUTEX_NAME) {
            Ok(handle) => {
                let _ = CloseHandle(handle);
                true
            }
            Err(_) => false,
        }
    }
}

/// Block until no other instance holds the single-instance mutex, or `timeout`
/// elapses. Used by an elevated relaunch to wait for the predecessor it is
/// replacing to exit before [`acquire`]-ing the lock itself.
pub fn wait_until_free(timeout: Duration) {
    let start = Instant::now();
    while exists() && start.elapsed() < timeout {
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Ask the running instance to bring its window to the front.
pub fn broadcast_show() {
    unsafe {
        let _ = PostMessageW(Some(HWND_BROADCAST), show_message(), WPARAM(0), LPARAM(0));
    }
}

/// Create and hold the single-instance mutex for this process. Returns `false`
/// if another instance won the race and already created it. The handle is
/// intentionally leaked (`HANDLE` has no `Drop`); the OS frees it on exit.
pub fn acquire() -> bool {
    unsafe {
        let _ = CreateMutexW(None, false, MUTEX_NAME);
        GetLastError() != ERROR_ALREADY_EXISTS
    }
}
