<#
.SYNOPSIS
  Combine the per-architecture Polaris .msix packages into one .msixbundle — a
  single artifact that carries both x64 and arm64, which is what you upload to the
  Microsoft Store.

.DESCRIPTION
  Run pack-msix.ps1 once per arch first, then this. It stages the per-arch .msix
  into a clean directory and runs `makeappx bundle`.

  Unsigned, like the per-arch packages: the Store re-signs on submission. To
  sideload a bundle, sign it (cert Subject = the manifest Publisher,
  CN=8ED44A81-DE3E-4BF0-8F28-5F94C3FAAF7D) with signtool, same as a single .msix.

.PARAMETER Version
  4-part bundle version. Defaults to the Cargo.toml version (4th part forced to 0).

.PARAMETER InputDir
  Directory holding the per-arch .msix files. Default: target\.

.PARAMETER Out
  Output .msixbundle path. Default: target\Polaris-<version>.msixbundle.

.EXAMPLE
  pwsh packaging\pack-msix.ps1 -Arch x64
  pwsh packaging\pack-msix.ps1 -Arch arm64
  pwsh packaging\bundle-msix.ps1
#>
[CmdletBinding()]
param(
  [string]$Version,
  [string]$InputDir,
  [string]$Out
)

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot   # packaging\ -> repo root

# --- version (default from Cargo.toml, normalised to 4 parts) ---------------
if (-not $Version) {
  $cargo = Get-Content (Join-Path $repo 'Cargo.toml') -Raw
  if ($cargo -notmatch '(?m)^\s*version\s*=\s*"([^"]+)"') { throw "Couldn't read version from Cargo.toml" }
  $Version = $Matches[1]
}
$parts = $Version.Split('-')[0].Split('.')
while ($parts.Count -lt 4) { $parts += '0' }
$Version = ($parts[0..3] -join '.')

if (-not $InputDir) { $InputDir = Join-Path $repo 'target' }
if (-not $Out) { $Out = Join-Path $repo "target\Polaris-$Version.msixbundle" }

# --- locate makeappx (newest version-numbered Windows SDK; same as pack-msix) -
function Get-SdkTool([string]$name) {
  $roots = @("${env:ProgramFiles(x86)}\Windows Kits\10\bin", "${env:ProgramFiles}\Windows Kits\10\bin")
  $hit = Get-ChildItem $roots -Recurse -Filter $name -ErrorAction SilentlyContinue |
    Where-Object { $_.FullName -match '\\10\.\d+\.\d+\.\d+\\x64\\' } |
    Sort-Object {
      $v = $null; [void][version]::TryParse($_.Directory.Parent.Name, [ref]$v)
      if ($v) { $v } else { [version]'0.0' }
    } |
    Select-Object -Last 1
  if (-not $hit) { throw "$name not found — install the Windows SDK (makeappx)." }
  return $hit.FullName
}
$makeappx = Get-SdkTool 'makeappx.exe'

# --- stage just the .msix packages (makeappx bundle wants a clean dir) -------
# *.msix matches the per-arch packages but not an existing *.msixbundle.
$pkgs = @(Get-ChildItem $InputDir -File -Filter '*.msix' -ErrorAction SilentlyContinue)
if ($pkgs.Count -eq 0) { throw "No .msix packages in $InputDir — run pack-msix.ps1 first." }
Write-Host "Bundling $($pkgs.Count) package(s) @ $Version" -ForegroundColor Cyan
$pkgs | ForEach-Object { Write-Host "  $($_.Name)" }

$stage = Join-Path $repo 'target\bundle-stage'
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Force $stage | Out-Null
$pkgs | ForEach-Object { Copy-Item $_.FullName $stage }

# --- bundle -----------------------------------------------------------------
& $makeappx bundle /d $stage /p $Out /bv $Version /overwrite
if ($LASTEXITCODE -ne 0) { throw "makeappx bundle failed ($LASTEXITCODE)" }
Remove-Item -Recurse -Force $stage

Write-Host "Bundled -> $Out" -ForegroundColor Green
if ($pkgs.Count -lt 2) {
  Write-Host "(only $($pkgs.Count) package bundled — run pack-msix.ps1 for the other arch for a full x64+arm64 bundle)" -ForegroundColor Yellow
}
