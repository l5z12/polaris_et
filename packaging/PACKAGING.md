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

## Identity (Microsoft Store reservation)

`AppxManifest.xml` is preset with this app's Partner Center reservation — keep
these in sync with `../.identity`, never edit them ad-hoc:

| Manifest field | Value |
| --- | --- |
| `Identity/@Name` | `L5Z12.Polaris-MeshVPN` |
| `Identity/@Publisher` | `CN=8ED44A81-DE3E-4BF0-8F28-5F94C3FAAF7D` |
| `Properties/PublisherDisplayName` | `L5Z12` |

(For reference: PFN `L5Z12.Polaris-MeshVPN_8kxbp6pv6h7wp`, Store ID
`9P3SRX33XN34`.) The Store rejects an upload whose identity doesn't match the
reservation exactly. `Version` and `ProcessorArchitecture` are stamped per build
by `pack-msix.ps1`, so leave their literal values in the manifest valid.

## Prerequisites

- **Windows SDK** — provides `makeappx.exe`, `makepri.exe`, and `signtool.exe`,
  under `C:\Program Files (x86)\Windows Kits\10\bin\<version>\x64`. The script
  finds the newest automatically.
- **Windows App SDK runtime.** The app links the Windows App SDK (WinUI 3). The
  manifest declares a framework `<PackageDependency>` on
  `Microsoft.WindowsAppRuntime.2` (MinVersion `2.1.3.0`, PFN
  `Microsoft.WindowsAppRuntime.2_8wekyb3d8bbwe`) so a packaged install resolves
  the framework instead of needing it pre-installed. The package also still
  ships `Microsoft.WindowsAppRuntime.Bootstrap.dll` (harmless in the packaged
  case; the runtime auto-initialises from the framework dependency). Bump the
  dependency's MinVersion whenever you bump the `windows-reactor` rev to a newer
  WinAppSDK — see *Store / clean machine* below.

## Assets

PNG logos live under `packaging/Assets/` (referenced by `AppxManifest.xml`) and
are already committed: `StoreLogo.png` (50×50), `Square44x44Logo.png` (44×44),
`Square150x150Logo.png` (150×150), `Wide310x150Logo.png` (310×150).

## Pack — `pack-msix.ps1`

The packaging is scripted end to end (build → stage layout → stamp
Version/arch → generate package `resources.pri` → `makeappx pack` → optional
sign):

```powershell
# VPN-capable x64 package (default: `--features msix`), for Store upload.
pwsh packaging\pack-msix.ps1

# Other shapes:
pwsh packaging\pack-msix.ps1 -Arch arm64            # ARM64
pwsh packaging\pack-msix.ps1 -ProxyOnly             # no elevation (drop allowElevation too)
pwsh packaging\pack-msix.ps1 -SkipBuild             # reuse target\<triple>\release
```

Output: `target\Polaris-<version>-<arch>.msix`.

For a **Store upload**, combine both CPUs into one `.msixbundle` with
`bundle-msix.ps1` (pack each arch first):

```powershell
pwsh packaging\pack-msix.ps1 -Arch x64
pwsh packaging\pack-msix.ps1 -Arch arm64
pwsh packaging\bundle-msix.ps1   # -> target\Polaris-<version>.msixbundle
```

Do *not* sign locally — Partner Center re-signs with the Store certificate.
Upload the `.msixbundle` (or a single-arch `.msix` if you only ship one CPU).

**CI:** pushing a version tag (`v*`) runs `release.yml`, which attaches, all
**unsigned**, `polaris-<tag>-x64.msix` + `-arm64.msix` (the `msix` job, via
`pack-msix.ps1`) and the combined `polaris-<tag>.msixbundle` (the `msixbundle`
job, via `bundle-msix.ps1`) to the GitHub Release. Unsigned for the same reason —
the Store re-signs; sideloaders sign their own.

## Sideload to test it actually launches

A Store upload is signed by the Store, but to install and smoke-test the package
locally you must sign it with a cert whose **Subject equals the manifest
`Publisher`** (`CN=8ED44A81-DE3E-4BF0-8F28-5F94C3FAAF7D`):

```powershell
# One-time: create a self-signed test cert with that exact subject and trust it.
$cert = New-SelfSignedCertificate -Type Custom -Subject "CN=8ED44A81-DE3E-4BF0-8F28-5F94C3FAAF7D" `
  -KeyUsage DigitalSignature -CertStoreLocation Cert:\CurrentUser\My `
  -TextExtension @("2.5.29.37={text}1.3.6.1.5.5.7.3.3", "2.5.29.19={text}")
Export-PfxCertificate -Cert $cert -FilePath polaris-test.pfx -Password (Read-Host -AsSecureString)
# Trust it (admin): import the .cer into LocalMachine\TrustedPeople.

# Sign the package and install it.
pwsh packaging\pack-msix.ps1 -SkipBuild -Sign -CertPath polaris-test.pfx -CertPassword (Read-Host -AsSecureString)
Add-AppxPackage target\Polaris-0.1.0.0-x64.msix
```

If it launches, the bootstrapper found the WinAppSDK runtime. If it dies with
`REGDB_E_CLASSNOTREG` / `Class not registered`, the runtime is missing (next
section).

## Store / clean machine: the WinAppSDK runtime

This is a *framework-dependent* build — the WinAppSDK runtime DLLs aren't inside
the package; the manifest's `<PackageDependency>` on
`Microsoft.WindowsAppRuntime.2` points at them. How that resolves:

- **Sideload** — `Add-AppxPackage` binds to any installed `.2` framework
  ≥ `2.1.3.0`. Present on your dev box (the build downloaded it); on a bare
  machine, install the runtime first (the WinAppSDK 2.x redistributable), or
  hand the framework `.msix` to `Add-AppxPackage -DependencyPath`.
- **Microsoft Store** — ⚠️ the Store only auto-provisions **stable (1.x)**
  WinAppSDK frameworks. `Microsoft.WindowsAppRuntime.2` is the **preview**
  channel, so a Store submission that depends on it will fail certification with
  an unresolved-dependency error. To ship through the Store you must either:
  1. move off the 2.x preview to a stable 1.x WinAppSDK (bump the
     `windows-reactor` rev and update the dependency Name/MinVersion), or
  2. build **self-contained** — pull the WinAppSDK runtime DLLs into the layout
     and drop the `<PackageDependency>`.

Confirm whichever path you pick by sideloading on a VM that has never had the
runtime installed.

## Store listing: privacy policy

Partner Center requires a **Privacy policy URL** in the submission's *Properties*,
per listing language. The homepage (`homepage/`, Cloudflare Pages) publishes the
policy — Polaris collects no data, with a link to EasyTier's own policy:

| Listing | URL |
| --- | --- |
| Default / English | `https://polaris.l5z12.dev/privacy.html` (auto-detects) or `/en/privacy.html` |
| Chinese (zh-CN) | `https://polaris.l5z12.dev/zh/privacy.html` (locked to Chinese) |

The `/en/` and `/zh/` variants default to that language (and surface EasyTier's
matching `en`/`zh` policy link); `/privacy.html` auto-detects from the browser.
Edit the copy in `homepage/content.ts` (`privacy`), not the generated HTML.
