#define SourceDir "C:\Users\Kyle\Source\tri-klops"

[Setup]
AppId={{c447ed56-3520-4c8e-a3a7-9c82d001a824}}
AppName=Tri-Klops
AppVersion=0.3.0
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
OutputBaseFilename=Tri-Klops_0.3.0_Setup
Compression=lzma
SolidCompression=yes
WizardStyle=modern
UninstallDisplayIcon={app}\triklops.exe

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "Create a Desktop shortcut"; GroupDescription: "Additional shortcuts:"
Name: "startmenuicon"; Description: "Create a Start Menu shortcut"; GroupDescription: "Additional shortcuts:"

[Files]
Source: "{#SourceDir}\target\release\triklops.exe"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Tri-Klops"; Filename: "{app}\triklops.exe"; Tasks: startmenuicon
Name: "{userdesktop}\Tri-Klops"; Filename: "{app}\triklops.exe"; Tasks: desktopicon
