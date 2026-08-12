#define AppName "KitsuTrack Bridge"
#define AppVersion "0.1.0"
#define AppPublisher "KitsuTrack contributors"
#define AppExeName "kitsutrack-bridge.exe"

[Setup]
AppId={{9938029A-3357-4A5D-8799-6DB12D0BBA35}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher={#AppPublisher}
DefaultDirName={autopf}\KitsuTrack
DefaultGroupName={#AppName}
OutputDir=dist
OutputBaseFilename=KitsuTrack-Windows-Setup-x64
Compression=lzma2
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=lowest
WizardStyle=modern
LicenseFile=..\LICENSE
UninstallDisplayIcon={app}\{#AppExeName}
; Keep this ID stable: Inno Setup uses it to recognize an installed release
; and install a newer package as an update rather than as a second app.
CloseApplications=force
RestartApplications=no

[Files]
Source: "dist\KitsuTrack\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\{#AppName}"; Filename: "{app}\{#AppExeName}"; WorkingDir: "{app}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExeName}"; WorkingDir: "{app}"; Tasks: desktopicon

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Additional icons:"

[Run]
Filename: "{app}\{#AppExeName}"; Description: "Launch {#AppName}"; Flags: nowait postinstall skipifsilent
Filename: "ms-windows-store://pdp/?ProductId=9NP83LWLPZ9K"; Description: "Install Apple Devices from Microsoft Store"; Flags: shellexec postinstall skipifsilent unchecked
Filename: "https://www.apple.com/itunes/download/win64"; Description: "Download legacy iTunes for Windows (64-bit)"; Flags: shellexec postinstall skipifsilent unchecked
Filename: "https://updates.cdn-apple.com/2020/windows/001-39935-20200911-1A70AA56-F448-11EA-8CC0-99D41950005E/iCloudSetup.exe"; Description: "Download legacy iCloud for Windows"; Flags: shellexec postinstall skipifsilent unchecked
