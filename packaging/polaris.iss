; Inno Setup script for Polaris (WinUI 3 client for the EasyTier mesh VPN).
;
; Builds a per-architecture installer. The CI release workflow compiles this
; once per arch, passing the payload dir and version via /D defines:
;
;   iscc /DAppVersion=0.1.0 /DVersionInfo=0.1.0 /DArch=x64 /DArchSpec=x64os \
;        /DSourceDir=<staged payload> /DOutputDir=<out> /DOutputBase=<name> \
;        packaging\polaris.iss
;
; For a local build, stage the release payload into <repo>\dist and just run
;   iscc packaging\polaris.iss
; (the #ifndef defaults below point SourceDir at ..\dist).

#ifndef AppVersion
  #define AppVersion "0.0.0-dev"
#endif
#ifndef VersionInfo
  #define VersionInfo "0.0.0"
#endif
#ifndef Arch
  #define Arch "x64"
#endif
#ifndef ArchSpec
  ; Inno architecture identifier: "x64os" (Intel/AMD) or "arm64".
  #define ArchSpec "x64os"
#endif
#ifndef SourceDir
  #define SourceDir "..\dist"
#endif
#ifndef OutputDir
  #define OutputDir ".."
#endif
#ifndef OutputBase
  #define OutputBase "polaris-" + AppVersion + "-" + Arch + "-setup"
#endif

[Setup]
; Keep this AppId stable across releases and arches so upgrades/uninstall work.
AppId={{BACA1D91-EC55-4E9A-84DA-3B6499A67A0E}
AppName=Polaris
AppVersion={#AppVersion}
AppPublisher=Polaris
AppCopyright=Polaris contributors — GPL-3.0-only
VersionInfoVersion={#VersionInfo}
VersionInfoProductName=Polaris
DefaultDirName={autopf}\Polaris
DefaultGroupName=Polaris
DisableProgramGroupPage=yes
UninstallDisplayName=Polaris
UninstallDisplayIcon={app}\polaris_et.exe
SetupIconFile=..\assets\polaris.ico
LicenseFile=..\LICENSE
; Per-machine install (VPN/TUN needs admin anyway); elevates on launch.
PrivilegesRequired=admin
; Windows App SDK requires Windows 10 1809+ (build 17763).
MinVersion=10.0.17763
ArchitecturesAllowed={#ArchSpec}
ArchitecturesInstallIn64BitMode={#ArchSpec}
WizardStyle=modern
Compression=lzma2/max
SolidCompression=yes
OutputDir={#OutputDir}
OutputBaseFilename={#OutputBase}

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
; The whole staged release payload: polaris_et.exe + wintun.dll + the Windows
; App SDK bootstrap DLL + resources.pri + Packet.dll + LICENSE/README/CREDITS.
Source: "{#SourceDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\Polaris"; Filename: "{app}\polaris_et.exe"
Name: "{autodesktop}\Polaris"; Filename: "{app}\polaris_et.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\polaris_et.exe"; Description: "{cm:LaunchProgram,Polaris}"; Flags: nowait postinstall skipifsilent
