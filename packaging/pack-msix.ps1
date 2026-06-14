<#
.SYNOPSIS
  Build Polaris and pack it into an MSIX using the Store identity baked into
  packaging\AppxManifest.xml (see ..\.identity / PACKAGING.md).

.DESCRIPTION
  Steps: cargo build (release) -> stage a package layout -> stamp the manifest's
  Version + ProcessorArchitecture -> (best effort) generate a package resources.pri
  -> makeappx pack -> optionally signtool sign.

  Identity (Name/Publisher/PublisherDisplayName) is NOT touched here — it lives in
  AppxManifest.xml and must match the Partner Center reservation exactly.

.PARAMETER Arch
  x64 (default) or arm64.

.PARAMETER Version
  4-part package version (e.g. 0.1.0.0). Defaults to the Cargo.toml version with a
  ".0" revision appended (the Store requires the 4th part to be 0).

.PARAMETER ProxyOnly
  Build the proxy-only package (plain `cargo build`, no elevation). Omit for the
  default VPN-capable build (`--features msix`, Windows 11+), which embeds a
  highestAvailable UAC manifest and pairs with the allowElevation capability.

.PARAMETER Sign
  Sign the produced .msix. Requires -CertPath (and usually -CertPassword). The
  certificate Subject MUST equal the manifest Publisher
  (CN=8ED44A81-DE3E-4BF0-8F28-5F94C3FAAF7D) or Windows rejects the package.
  NOT needed for a Store upload — Partner Center re-signs with the Store cert.

.PARAMETER SkipBuild
  Reuse an existing target\<triple>\release build instead of running cargo.

.EXAMPLE
  # VPN-capable x64 package for Store upload (no local signing needed):
  pwsh packaging\pack-msix.ps1

.EXAMPLE
  # Sideload test build, signed with a self-signed cert for local install:
  pwsh packaging\pack-msix.ps1 -Sign -CertPath polaris-test.pfx -CertPassword (Read-Host -AsSecureString)
#>
[CmdletBinding()]
param(
  [ValidateSet('x64', 'arm64')] [string]$Arch = 'x64',
  [string]$Version,
  [switch]$ProxyOnly,
  [switch]$Sign,
  [string]$CertPath,
  [System.Security.SecureString]$CertPassword,
  [switch]$SkipBuild
)

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot   # packaging\ -> repo root
$pkg  = $PSScriptRoot

# --- arch-specific bits -----------------------------------------------------
$triple = if ($Arch -eq 'arm64') { 'aarch64-pc-windows-msvc' } else { 'x86_64-pc-windows-msvc' }
$tpdir  = if ($Arch -eq 'arm64') { 'arm64' } else { 'x86_64' }   # easytier\third_party\<tpdir>\Packet.dll

# --- version (default from Cargo.toml, normalised to 4 parts) ---------------
if (-not $Version) {
  $cargo = Get-Content (Join-Path $repo 'Cargo.toml') -Raw
  if ($cargo -notmatch '(?m)^\s*version\s*=\s*"([^"]+)"') { throw "Couldn't read version from Cargo.toml" }
  $Version = $Matches[1]
}
$parts = $Version.Split('-')[0].Split('.')          # drop any -rc suffix
while ($parts.Count -lt 4) { $parts += '0' }
$Version = ($parts[0..3] -join '.')                 # Store requires the 4th part = 0
Write-Host "Package version: $Version ($Arch)" -ForegroundColor Cyan

# --- locate Windows SDK tools (newest version that has makeappx) ------------
function Get-SdkTool([string]$name) {
  $roots = @("${env:ProgramFiles(x86)}\Windows Kits\10\bin", "${env:ProgramFiles}\Windows Kits\10\bin")
  # Prefer the host-arch (x64) tool under a version-numbered SDK dir, newest first.
  $hit = Get-ChildItem $roots -Recurse -Filter $name -ErrorAction SilentlyContinue |
    Where-Object { $_.FullName -match '\\10\.\d+\.\d+\.\d+\\x64\\' } |
    Sort-Object {
      $v = $null; [void][version]::TryParse($_.Directory.Parent.Name, [ref]$v)
      if ($v) { $v } else { [version]'0.0' }
    } |
    Select-Object -Last 1
  if (-not $hit) { throw "$name not found — install the Windows SDK (makeappx/makepri/signtool)." }
  return $hit.FullName
}
$makeappx = Get-SdkTool 'makeappx.exe'
$makepri  = Get-SdkTool 'makepri.exe'

# --- 1. build ---------------------------------------------------------------
if (-not $SkipBuild) {
  $feat = if ($ProxyOnly) { @() } else { @('--features', 'msix') }
  Write-Host "cargo build --release --target $triple $($feat -join ' ')" -ForegroundColor Cyan
  & cargo build --release --bin polaris_et --target $triple @feat
  if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
}

$rel = Join-Path $repo "target\$triple\release"
if (-not (Test-Path (Join-Path $rel 'polaris_et.exe'))) {
  # Fall back to a default (no --target) build for x64 convenience.
  $alt = Join-Path $repo 'target\release'
  if ($Arch -eq 'x64' -and (Test-Path (Join-Path $alt 'polaris_et.exe'))) {
    Write-Host "Using target\release (no --target build)" -ForegroundColor Yellow
    $rel = $alt
  } else {
    throw "No build at $rel — run without -SkipBuild."
  }
}

# --- 2. stage the layout ----------------------------------------------------
$lay = Join-Path $repo "target\msix\$Arch"
if (Test-Path $lay) { Remove-Item -Recurse -Force $lay }
New-Item -ItemType Directory -Force (Join-Path $lay 'Assets') | Out-Null

# Runtime payload — same file set as the zip/installer, minus the unpackaged
# resources.pri (regenerated below so it matches the package identity).
$need = @{
  'polaris_et.exe'                              = $rel
  'wintun.dll'                                  = $rel
  'Microsoft.WindowsAppRuntime.Bootstrap.dll'   = $rel
}
foreach ($f in $need.Keys) {
  $src = Join-Path $need[$f] $f
  if (-not (Test-Path $src)) { throw "Missing build output: $src" }
  Copy-Item $src $lay
}
Copy-Item (Join-Path $repo "easytier\third_party\$tpdir\Packet.dll") $lay
Copy-Item (Join-Path $pkg 'Assets\*') (Join-Path $lay 'Assets')

# Manifest with the Identity's Version + ProcessorArchitecture stamped for this
# build. Notes on the regex:
#  - -creplace (case-sensitive) so it won't touch the lowercase `version` in the
#    <?xml ... ?> declaration.
#  - (?<!\w) word-boundary lookbehind so `Version="..."` matches only the Identity
#    attribute, NOT `MinVersion="..."` on TargetDeviceFamily / PackageDependency
#    (whose framework version must stay intact).
$manifest = Get-Content (Join-Path $pkg 'AppxManifest.xml') -Raw
$manifest = $manifest -creplace '(?<!\w)Version="[\d.]+"', "Version=`"$Version`""
$manifest = $manifest -creplace 'ProcessorArchitecture="\w+"', "ProcessorArchitecture=`"$Arch`""
Set-Content (Join-Path $lay 'AppxManifest.xml') $manifest -Encoding UTF8

# --- 3. package resources.pri (best effort) ---------------------------------
# The app has no ms-resource:/.resw resources, so this is essentially a package
# identity index. If makepri has nothing to index it can warn — that's fine; the
# package installs without a PRI too, so don't fail the build on it.
Push-Location $lay
try {
  $cfg = Join-Path $lay 'priconfig.xml'
  & $makepri createconfig /cf $cfg /dq en-US /o 2>&1 | Out-Null
  & $makepri new /pr $lay /cf $cfg /mn (Join-Path $lay 'AppxManifest.xml') /of (Join-Path $lay 'resources.pri') /o 2>&1 | Out-Null
  Remove-Item $cfg -ErrorAction SilentlyContinue
  if (Test-Path (Join-Path $lay 'resources.pri')) { Write-Host "Generated package resources.pri" -ForegroundColor DarkGray }
  else { Write-Host "No resources.pri generated (no app resources) — packing without one" -ForegroundColor Yellow }
} finally { Pop-Location }

# --- 4. pack ----------------------------------------------------------------
$msix = Join-Path $repo "target\Polaris-$Version-$Arch.msix"
& $makeappx pack /d $lay /p $msix /overwrite
if ($LASTEXITCODE -ne 0) { throw "makeappx pack failed" }
Write-Host "Packed: $msix" -ForegroundColor Green

# --- 5. sign (optional; not needed for Store upload) ------------------------
if ($Sign) {
  if (-not $CertPath) { throw "-Sign requires -CertPath (cert Subject must equal the manifest Publisher)." }
  $signtool = Get-SdkTool 'signtool.exe'
  $signArgs = @('sign', '/fd', 'SHA256', '/a', '/f', $CertPath)
  if ($CertPassword) {
    $signArgs += '/p'
    $signArgs += [Runtime.InteropServices.Marshal]::PtrToStringAuto(
      [Runtime.InteropServices.Marshal]::SecureStringToBSTR($CertPassword))
  }
  $signArgs += $msix
  & $signtool @signArgs
  if ($LASTEXITCODE -ne 0) { throw "signtool failed" }
  Write-Host "Signed: $msix" -ForegroundColor Green
}

Write-Host ""
Write-Host "Done -> $msix" -ForegroundColor Green
if (-not $Sign) {
  Write-Host "Next: upload to Partner Center (it re-signs), or sign for sideload:" -ForegroundColor DarkGray
  Write-Host "  pwsh packaging\pack-msix.ps1 -SkipBuild -Sign -CertPath <test>.pfx -CertPassword (Read-Host -AsSecureString)" -ForegroundColor DarkGray
}
