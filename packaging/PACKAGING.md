# Packaging Polaris as MSIX (Microsoft Store / sideload)

Polaris detects at runtime whether it is running from an MSIX package
(`elevate::is_packaged`) and adapts. There are two packaged shapes:

| Distribution | Admin / TUN VPN | Proxy (SOCKS5, port-forward) |
| --- | --- | --- |
| **Standalone .exe** | Yes — self-elevates on demand (Settings → *Always launch as administrator*, or *Restart as administrator now*) | Yes |
| **MSIX, proxy-only** (default `cargo build`) | No — exe stays `asInvoker`; TUN auto-disabled, the admin card shows read-only status | Yes |
| **MSIX, VPN-capable** (`cargo build --features msix`, Win 11+) | Yes — `allowElevation` capability + a `highestAvailable` exe manifest; administrators elevate at launch, standard users fall back to proxy-only | Yes |

A packaged app *can* request admin: declare the `allowElevation` restricted
capability in `AppxManifest.xml` **and** build with `--features msix`, which
embeds a `highestAvailable` UAC manifest in the exe (see `build.rs`). Caveats:

- **Windows 11+.** Elevated WinAppSDK / WinUI 3 packaged apps need OS support
  that only exists on Windows 11
  ([WindowsAppSDK #896](https://github.com/microsoft/WindowsAppSDK/issues/896)).
  On Windows 10 the elevated package may not launch correctly — ship the
  proxy-only package (or the standalone exe) there.
- **Restricted capability.** `allowElevation` is a `rescap` capability: fine for
  sideloading, but Store submissions must justify it at certification. For a
  proxy-only Store build, remove the `allowElevation` line and build without
  `--features msix`.
- **Per-launch UAC.** `highestAvailable` prompts administrators for consent on
  every launch. The lower-friction alternative is an elevated helper **service**
  (installed separately, running as SYSTEM) that the unprivileged GUI drives
  over IPC — the model WireGuard and Tailscale use. That's a larger change and
  out of scope here.

## Prerequisites

- **Windows SDK** — provides `makeappx.exe` and `signtool.exe`, typically under
  `C:\Program Files (x86)\Windows Kits\10\bin\<version>\x64`.
- **Windows App SDK runtime.** The app links the Windows App SDK (WinUI 3).
  Either declare a framework `<PackageDependency>` on
  `Microsoft.WindowsAppRuntime.1.x` in the manifest, or build self-contained and
  copy the WinAppSDK DLLs next to the exe in the layout.

## Assets

Add PNG logos under `packaging/Assets/` (referenced by `AppxManifest.xml`):

- `StoreLogo.png` — 50×50
- `Square44x44Logo.png` — 44×44
- `Square150x150Logo.png` — 150×150
- `Wide310x150Logo.png` — 310×150

## Build a layout and pack

```powershell
# 1. Release build.
#    Proxy-only package:    cargo build --release --bin polaris_et
#    VPN-capable (Win 11+): cargo build --release --bin polaris_et --features msix
cargo build --release --bin polaris_et --features msix

# 2. Assemble the package layout
$lay = "target\msix"
Remove-Item -Recurse -Force $lay -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force "$lay\Assets" | Out-Null
Copy-Item target\release\polaris_et.exe $lay
Copy-Item packaging\AppxManifest.xml    $lay
Copy-Item packaging\Assets\*            "$lay\Assets"
# Bundled wintun driver — must sit next to the exe so rust-tun loads our
# WireGuard-signed copy instead of a foreign wintun.dll found via PATH.
# (`cargo build` already drops it in target\release; copy from there or vendor.)
Copy-Item wintun\bin\amd64\wintun.dll   $lay
# Also copy any required Windows App SDK DLLs into $lay.

# 3. Pack
makeappx.exe pack /d $lay /p target\Polaris.msix

# 4. Sign (test certificate, or your Store-associated cert)
signtool.exe sign /fd SHA256 /a /f mycert.pfx /p <password> target\Polaris.msix
```

Set the manifest `Identity` (`Name`, `Publisher`, `Version`) to match your
Store reservation and the subject of your signing certificate. For local
sideloading, create and trust a self-signed cert whose subject equals
`Publisher`, then `Add-AppxPackage target\Polaris.msix`.
