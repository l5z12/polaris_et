//! Windows privilege & packaging detection.
//!
//! EasyTier's TUN adapter can only be created with administrator rights. We
//! never force elevation: by default Polaris runs unprivileged and the engine
//! disables TUN (SOCKS5 / port-forward proxies still work). The user can opt in
//! with the "Always launch as administrator" setting, or restart elevated on
//! demand.
//!
//! An MSIX / Store package elevates differently: not via a UAC relaunch but
//! through its manifest — the `allowElevation` capability plus a
//! `highestAvailable` execution level, built with `--features msix` (Windows
//! 11+). So a packaged build skips the runtime relaunch and replaces the
//! opt-in controls with read-only status. See [`is_packaged`].

use std::os::windows::ffi::OsStrExt;
use std::sync::atomic::{AtomicU8, Ordering};

use windows::Win32::Foundation::{APPMODEL_ERROR_NO_PACKAGE, CloseHandle, HANDLE};
use windows::Win32::Security::{GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation};
use windows::Win32::Storage::Packaging::Appx::GetCurrentPackageFullName;
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::{PCWSTR, w};

// 0 = not yet queried, 1 = yes, 2 = no.
static ELEVATED: AtomicU8 = AtomicU8::new(0);
static PACKAGED: AtomicU8 = AtomicU8::new(0);

fn flag(v: bool) -> u8 {
    if v { 1 } else { 2 }
}

/// Query and cache privilege/packaging state once, early in `main`.
pub fn init() {
    ELEVATED.store(flag(query_elevated()), Ordering::Relaxed);
    PACKAGED.store(flag(query_packaged()), Ordering::Relaxed);
}

/// `true` if the process is running with administrator rights.
pub fn is_elevated() -> bool {
    ELEVATED.load(Ordering::Relaxed) == 1
}

/// `true` if running from an MSIX/AppX package (Store / sideload). A packaged
/// build elevates through its manifest rather than a UAC relaunch, so callers
/// use this to skip [`relaunch_elevated`] and show status instead of controls.
pub fn is_packaged() -> bool {
    PACKAGED.load(Ordering::Relaxed) == 1
}

fn query_elevated() -> bool {
    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION::default();
        let mut size = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            Some((&mut elevation as *mut TOKEN_ELEVATION).cast()),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut size,
        );
        let _ = CloseHandle(token);
        ok.is_ok() && elevation.TokenIsElevated != 0
    }
}

fn query_packaged() -> bool {
    unsafe {
        let mut len = 0u32;
        // Returns APPMODEL_ERROR_NO_PACKAGE when run as a plain (unpackaged) exe.
        GetCurrentPackageFullName(&mut len, None) != APPMODEL_ERROR_NO_PACKAGE
    }
}

/// Command-line marker passed to the elevated relaunch. It tells the new
/// process it must *take over* from the exiting instance — waiting for that
/// instance's single-instance mutex to drop — instead of detecting the
/// still-alive predecessor and quitting as a duplicate launch.
const RELAUNCH_FLAG: &str = "--relaunch-elevated";

/// `true` if this process was started by [`relaunch_elevated`] (it carries the
/// takeover flag on its command line).
pub fn is_relaunch() -> bool {
    std::env::args().any(|a| a == RELAUNCH_FLAG)
}

/// Relaunch this executable elevated via UAC. Returns `true` if a new elevated
/// process was started (the caller should then exit). Not used when packaged —
/// MSIX builds elevate via their manifest, not by relaunching the raw exe.
///
/// The child carries [`RELAUNCH_FLAG`] so it waits for this (exiting) instance's
/// single-instance mutex to release and then takes over; without it the child
/// would see the predecessor, bow out, and leave nothing running.
pub fn relaunch_elevated() -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let exe: Vec<u16> = exe
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    // Carry the takeover flag, and preserve `--startup` so an autostart launch
    // that relaunches to elevate still starts hidden in the tray.
    let mut args = RELAUNCH_FLAG.to_string();
    if std::env::args().any(|a| a == crate::autostart::STARTUP_ARG) {
        args.push(' ');
        args.push_str(crate::autostart::STARTUP_ARG);
    }
    let params: Vec<u16> = args.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let result = ShellExecuteW(
            None,
            w!("runas"),
            PCWSTR(exe.as_ptr()),
            PCWSTR(params.as_ptr()),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        );
        // ShellExecute returns a value > 32 on success.
        result.0 as usize > 32
    }
}
