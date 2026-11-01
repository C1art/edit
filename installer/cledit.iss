; The Cledit installer, compiled by scripts/release.sh with Inno Setup:
;   ISCC /DAppVersion=<version> /DDist=<files to install> /DIcon=<cledit.ico> /DOutput=<folder> cledit.iss
; Cledit's updater runs the next version's installer with /SILENT, which keeps the folder,
; install mode and choices made here, and with /relaunch=1 to start Cledit again after.

#ifndef AppVersion
  #error Pass /DAppVersion=<version>
#endif

[Setup]
; Never change: Windows knows the installed Cledit by it, and updates replace that install.
AppId={{C150D3A9-C758-4C99-A7A4-27CB8B72382C}
AppName=Cledit
AppVersion={#AppVersion}
AppVerName=Cledit {#AppVersion}
AppPublisher=Clart
AppPublisherURL=https://github.com/C1art/edit
AppSupportURL=https://github.com/C1art/edit/issues
AppUpdatesURL=https://github.com/C1art/edit/releases
VersionInfoVersion={#AppVersion}
; Just for the user by default, so neither installing nor updating needs administrator
; rights; the first page offers installing for all users instead.
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
DefaultDirName={autopf}\Cledit
DisableProgramGroupPage=yes
LicenseFile={#Dist}\LICENSE
SetupIconFile={#Icon}
UninstallDisplayIcon={app}\cledit.exe
UninstallDisplayName=Cledit
OutputDir={#Output}
OutputBaseFilename=Cledit-{#AppVersion}-Setup
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
ChangesAssociations=yes
CloseApplications=yes

[Tasks]
Name: startmenuicon; Description: "Add Cledit to the &Start menu"; GroupDescription: "Shortcuts:"
Name: desktopicon; Description: "Add a &desktop shortcut"; GroupDescription: "Shortcuts:"; Flags: unchecked
Name: fileassoc; Description: "Open .cledit and .editproj &projects with Cledit"; GroupDescription: "Files:"

[InstallDelete]
; An update may ship a different FFmpeg, whose old DLLs would otherwise stay behind.
Type: files; Name: "{app}\*.dll"

[Files]
Source: "{#Dist}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\Cledit"; Filename: "{app}\cledit.exe"; Tasks: startmenuicon
Name: "{autodesktop}\Cledit"; Filename: "{app}\cledit.exe"; Tasks: desktopicon

[Registry]
Root: HKA; Subkey: "Software\Classes\.cledit\OpenWithProgids"; ValueType: string; ValueName: "Cledit.Project"; ValueData: ""; Flags: uninsdeletevalue; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\.cledit"; ValueType: string; ValueName: ""; ValueData: "Cledit.Project"; Flags: uninsdeletevalue; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\.editproj\OpenWithProgids"; ValueType: string; ValueName: "Cledit.Project"; ValueData: ""; Flags: uninsdeletevalue; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\.editproj"; ValueType: string; ValueName: ""; ValueData: "Cledit.Project"; Flags: uninsdeletevalue; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\Cledit.Project"; ValueType: string; ValueName: ""; ValueData: "Cledit Project"; Flags: uninsdeletekey; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\Cledit.Project\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: """{app}\cledit.exe"",0"; Tasks: fileassoc
Root: HKA; Subkey: "Software\Classes\Cledit.Project\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\cledit.exe"" ""%1"""; Tasks: fileassoc

[Run]
Filename: "{app}\cledit.exe"; Description: "&Start Cledit"; Flags: nowait postinstall skipifsilent
Filename: "{app}\cledit.exe"; Parameters: "--updated"; Flags: nowait runasoriginaluser; Check: Relaunch

[Code]
{ Set by Cledit's Restart Now: start the updated Cledit once installed. }
function Relaunch: Boolean;
begin
  Result := ExpandConstant('{param:relaunch|0}') = '1';
end;
