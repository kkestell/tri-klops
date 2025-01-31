#define SourceDir "C:\Users\Kyle\Source\tri-klops"

[Setup]
AppId={{c447ed56-3520-4c8e-a3a7-9c82d001a824}}
AppName=Tri-Klops
AppVersion=0.2.0
AppVerName=Tri-Klops
AppPublisher=Kyle Kestell
AppPublisherURL=https://github.com/kkestell/tri-klops
AppSupportURL=https://github.com/kkestell/tri-klops
AppUpdatesURL=https://github.com/kkestell/tri-klops
DefaultDirName={autopf}\Tri-Klops
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
DisableProgramGroupPage=yes
LicenseFile={#SourceDir}\LICENSE
PrivilegesRequired=lowest
OutputDir={#SourceDir}\publish
OutputBaseFilename=Tri-Klops_0.2.0_Setup
SetupIconFile={#SourceDir}\assets\icon.ico
Compression=lzma
SolidCompression=yes
WizardStyle=modern
UninstallDisplayIcon={app}\triklops.exe

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "powershellshortcut"; Description: "Create desktop shortcut to PowerShell with Tri-Klops in PATH (recommended)"; GroupDescription: "Optional shortcuts:"

[Files]
Source: "{#SourceDir}\target\release\triklops.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\assets\icon.ico"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
[Icons]
Name: "{group}\Tri-Klops"; Filename: "{app}\triklops.exe"; IconFilename: "{app}\triklops.exe"
Name: "{userdesktop}\Tri-Klops PowerShell"; Filename: "powershell.exe"; Parameters: "-NoExit -Command ""cd $HOME; $env:PATH = '{app}' + ';' + $env:PATH; triklops --help"""; Tasks: powershellshortcut