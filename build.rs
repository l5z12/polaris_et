//! Build-time setup:
//!
//! 1. Embeds the Windows application icon into the executable. The icon
//!    (resource id 1) is also loaded at runtime for the system-tray icon
//!    (see `tray.rs`).
//! 2. Copies our bundled, WireGuard-signed `wintun.dll` next to the produced
//!    exe so EasyTier/rust-tun load *ours* instead of a stray `wintun.dll`
//!    from another app (e.g. Cloudflare WARP) found via PATH. See
//!    `copy_bundled_wintun` for the full rationale.

use std::path::{Path, PathBuf};

/// UAC manifest embedded only for the `msix` (VPN-capable packaged) build.
/// `highestAvailable` elevates for administrators (so EasyTier can create the
/// TUN adapter) while still letting standard users run proxy-only — unlike the
/// hard block `requireAdministrator` would impose.
const ELEVATED_MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="highestAvailable" uiAccess="false" />
      </requestedPrivileges>
    </security>
  </trustInfo>
</assembly>
"#;

fn main() {
    println!("cargo:rerun-if-changed=assets/polaris.ico");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon_with_id("assets/polaris.ico", "1");
        // Windows shows `FileDescription` as the app name (taskbar jump list,
        // file properties); winresource otherwise defaults it to the crate name
        // (`polaris_et`), which is why the taskbar read "polaris_et.exe".
        res.set("FileDescription", "Polaris");
        res.set("ProductName", "Polaris");
        // A VPN-capable MSIX (`--features msix`) requests elevation so EasyTier
        // can bring up the TUN adapter. The standalone exe keeps the default
        // `asInvoker` and elevates on demand at runtime instead.
        if std::env::var_os("CARGO_FEATURE_MSIX").is_some() {
            res.set_manifest(ELEVATED_MANIFEST);
        }
        if let Err(e) = res.compile() {
            println!("cargo:warning=failed to embed application icon: {e}");
        }

        copy_bundled_wintun();
    }
}

/// Place our vendored `wintun.dll` beside the output exe.
///
/// rust-tun (via EasyTier) loads the wintun driver by the bare name
/// `"wintun.dll"` and then rejects it unless its Authenticode signer is
/// "WireGuard LLC". With nothing next to the exe, Windows' DLL search order
/// falls through to PATH and can pick up a different vendor's `wintun.dll`
/// (Cloudflare WARP ships one signed "Cloudflare, Inc."), which fails the
/// signer check and the TUN adapter never comes up.
///
/// The application directory is searched *before* PATH, so dropping our
/// WireGuard-signed copy next to `polaris_et.exe` makes it win deterministically.
fn copy_bundled_wintun() {
    // Map the build target arch to wintun's prebuilt-binary layout.
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let sub = match arch.as_str() {
        "x86_64" => "amd64",
        "x86" => "x86",
        "aarch64" => "arm64",
        "arm" => "arm",
        other => {
            println!("cargo:warning=no bundled wintun.dll for target arch '{other}'");
            return;
        }
    };

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let src = Path::new(&manifest_dir)
        .join("wintun/bin")
        .join(sub)
        .join("wintun.dll");
    println!("cargo:rerun-if-changed={}", src.display());

    if !src.exists() {
        println!(
            "cargo:warning=bundled wintun.dll not found at {} — TUN VPN will fail if a \
             foreign wintun.dll is found on PATH",
            src.display()
        );
        return;
    }

    // OUT_DIR is `<target>/<profile>/build/<pkg>-<hash>/out`; the exe lands in
    // `<target>/<profile>`, which is OUT_DIR's 3rd ancestor (holds with both
    // a default and an explicit `--target <triple>`).
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let Some(exe_dir) = Path::new(&out_dir).ancestors().nth(3).map(PathBuf::from) else {
        println!("cargo:warning=could not locate target dir to place wintun.dll");
        return;
    };

    let dst = exe_dir.join("wintun.dll");
    if let Err(e) = std::fs::copy(&src, &dst) {
        println!(
            "cargo:warning=failed to copy wintun.dll to {}: {e}",
            dst.display()
        );
    }
}
