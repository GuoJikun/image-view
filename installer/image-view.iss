; image-view Inno Setup 6 script.
;
; Build through scripts\build-release.ps1, or manually:
;   iscc /DAppVersion=2.0.0-alpha.1 /DAppVersionNumeric=2.0.0.0 installer\image-view.iss
;
; Design notes:
; - Per-user install under %LOCALAPPDATA%\Programs, no administrator rights.
; - Associations are registered as "Open with" entries plus Default Apps
;   capabilities. Windows 10/11 does not allow silently changing the default
;   handler, so the optional task writes HKCU defaults and offers the system
;   Settings page; previous values are backed up and restored on uninstall.
; - Uninstall removes program files, shortcuts, registry entries, settings,
;   thumbnail cache and logs.
; - The Inno Setup 6.7 compiler used for the local build reports
;   "Non-commercial use only"; commercial distribution may require a license.

#ifndef AppVersion
  #define AppVersion "0.0.0-dev"
#endif
#ifndef AppVersionNumeric
  #define AppVersionNumeric "0.0.0.0"
#endif

#define AppName "image-view"
#define AppPublisher "image-view"
#define AppURL "https://github.com/GuoJikun/image-view"
#define AppExeName "image-view.exe"
#define AppProgId "ImageView.Image"
#define AppRegRoot "Software\image-view"
#define AppBackupKey "Software\image-view\AssociationBackup"

[Setup]
AppId={{CFFA0BB2-A715-468F-A1A7-7C56BC01E809}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher={#AppPublisher}
AppPublisherURL={#AppURL}
AppSupportURL={#AppURL}
AppUpdatesURL={#AppURL}
DefaultDirName={localappdata}\Programs\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
OutputDir=..\dist
OutputBaseFilename={#AppName}-{#AppVersion}-setup
SetupIconFile=..\assets\icon.ico
UninstallDisplayIcon={app}\{#AppExeName}
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
ChangesAssociations=yes
CloseApplications=yes
RestartApplications=no
VersionInfoVersion={#AppVersionNumeric}
VersionInfoProductName={#AppName}
VersionInfoProductVersion={#AppVersionNumeric}
VersionInfoCompany={#AppPublisher}
VersionInfoDescription={#AppName} installer

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "setdefault"; Description: "Set image-view as the default image viewer (Windows may ask for confirmation)"; GroupDescription: "File associations:"; Flags: unchecked

[Files]
Source: "..\target\release\{#AppExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\CHANGELOG.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#AppName}"; Filename: "{app}\{#AppExeName}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExeName}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#AppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(AppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent
Filename: "ms-settings:defaultapps"; Description: "Open Windows Default Apps settings"; Flags: shellexec nowait postinstall skipifsilent; Tasks: setdefault

[Registry]
; Optional default-viewer task. Only the default value of each extension key is
; written; uninstall deletes the value (and the key if it becomes empty).
Root: HKCU; Subkey: "Software\Classes\.png"; ValueType: string; ValueData: "{#AppProgId}"; Tasks: setdefault; Flags: uninsdeletevalue uninsdeletekeyifempty
Root: HKCU; Subkey: "Software\Classes\.jpg"; ValueType: string; ValueData: "{#AppProgId}"; Tasks: setdefault; Flags: uninsdeletevalue uninsdeletekeyifempty
Root: HKCU; Subkey: "Software\Classes\.jpeg"; ValueType: string; ValueData: "{#AppProgId}"; Tasks: setdefault; Flags: uninsdeletevalue uninsdeletekeyifempty
Root: HKCU; Subkey: "Software\Classes\.gif"; ValueType: string; ValueData: "{#AppProgId}"; Tasks: setdefault; Flags: uninsdeletevalue uninsdeletekeyifempty
Root: HKCU; Subkey: "Software\Classes\.webp"; ValueType: string; ValueData: "{#AppProgId}"; Tasks: setdefault; Flags: uninsdeletevalue uninsdeletekeyifempty
Root: HKCU; Subkey: "Software\Classes\.bmp"; ValueType: string; ValueData: "{#AppProgId}"; Tasks: setdefault; Flags: uninsdeletevalue uninsdeletekeyifempty
Root: HKCU; Subkey: "Software\Classes\.tif"; ValueType: string; ValueData: "{#AppProgId}"; Tasks: setdefault; Flags: uninsdeletevalue uninsdeletekeyifempty
Root: HKCU; Subkey: "Software\Classes\.tiff"; ValueType: string; ValueData: "{#AppProgId}"; Tasks: setdefault; Flags: uninsdeletevalue uninsdeletekeyifempty
Root: HKCU; Subkey: "Software\Classes\.ico"; ValueType: string; ValueData: "{#AppProgId}"; Tasks: setdefault; Flags: uninsdeletevalue uninsdeletekeyifempty
Root: HKCU; Subkey: "Software\Classes\.qoi"; ValueType: string; ValueData: "{#AppProgId}"; Tasks: setdefault; Flags: uninsdeletevalue uninsdeletekeyifempty
Root: HKCU; Subkey: "Software\Classes\.tga"; ValueType: string; ValueData: "{#AppProgId}"; Tasks: setdefault; Flags: uninsdeletevalue uninsdeletekeyifempty
Root: HKCU; Subkey: "Software\Classes\.pnm"; ValueType: string; ValueData: "{#AppProgId}"; Tasks: setdefault; Flags: uninsdeletevalue uninsdeletekeyifempty
Root: HKCU; Subkey: "Software\Classes\.hdr"; ValueType: string; ValueData: "{#AppProgId}"; Tasks: setdefault; Flags: uninsdeletevalue uninsdeletekeyifempty
Root: HKCU; Subkey: "Software\Classes\.exr"; ValueType: string; ValueData: "{#AppProgId}"; Tasks: setdefault; Flags: uninsdeletevalue uninsdeletekeyifempty
Root: HKCU; Subkey: "Software\Classes\.dds"; ValueType: string; ValueData: "{#AppProgId}"; Tasks: setdefault; Flags: uninsdeletevalue uninsdeletekeyifempty
Root: HKCU; Subkey: "Software\Classes\.avif"; ValueType: string; ValueData: "{#AppProgId}"; Tasks: setdefault; Flags: uninsdeletevalue uninsdeletekeyifempty
Root: HKCU; Subkey: "Software\Classes\.heic"; ValueType: string; ValueData: "{#AppProgId}"; Tasks: setdefault; Flags: uninsdeletevalue uninsdeletekeyifempty
Root: HKCU; Subkey: "Software\Classes\.heif"; ValueType: string; ValueData: "{#AppProgId}"; Tasks: setdefault; Flags: uninsdeletevalue uninsdeletekeyifempty

; Explorer "Open with" entries, independent of the default-viewer task.
Root: HKCU; Subkey: "Software\Classes\.png\OpenWithProgids"; ValueType: string; ValueName: "{#AppProgId}"; ValueData: ""; Flags: uninsdeletevalue uninsdeletekeyifempty
Root: HKCU; Subkey: "Software\Classes\.jpg\OpenWithProgids"; ValueType: string; ValueName: "{#AppProgId}"; ValueData: ""; Flags: uninsdeletevalue uninsdeletekeyifempty
Root: HKCU; Subkey: "Software\Classes\.jpeg\OpenWithProgids"; ValueType: string; ValueName: "{#AppProgId}"; ValueData: ""; Flags: uninsdeletevalue uninsdeletekeyifempty
Root: HKCU; Subkey: "Software\Classes\.gif\OpenWithProgids"; ValueType: string; ValueName: "{#AppProgId}"; ValueData: ""; Flags: uninsdeletevalue uninsdeletekeyifempty
Root: HKCU; Subkey: "Software\Classes\.webp\OpenWithProgids"; ValueType: string; ValueName: "{#AppProgId}"; ValueData: ""; Flags: uninsdeletevalue uninsdeletekeyifempty
Root: HKCU; Subkey: "Software\Classes\.bmp\OpenWithProgids"; ValueType: string; ValueName: "{#AppProgId}"; ValueData: ""; Flags: uninsdeletevalue uninsdeletekeyifempty
Root: HKCU; Subkey: "Software\Classes\.tif\OpenWithProgids"; ValueType: string; ValueName: "{#AppProgId}"; ValueData: ""; Flags: uninsdeletevalue uninsdeletekeyifempty
Root: HKCU; Subkey: "Software\Classes\.tiff\OpenWithProgids"; ValueType: string; ValueName: "{#AppProgId}"; ValueData: ""; Flags: uninsdeletevalue uninsdeletekeyifempty
Root: HKCU; Subkey: "Software\Classes\.ico\OpenWithProgids"; ValueType: string; ValueName: "{#AppProgId}"; ValueData: ""; Flags: uninsdeletevalue uninsdeletekeyifempty
Root: HKCU; Subkey: "Software\Classes\.qoi\OpenWithProgids"; ValueType: string; ValueName: "{#AppProgId}"; ValueData: ""; Flags: uninsdeletevalue uninsdeletekeyifempty
Root: HKCU; Subkey: "Software\Classes\.tga\OpenWithProgids"; ValueType: string; ValueName: "{#AppProgId}"; ValueData: ""; Flags: uninsdeletevalue uninsdeletekeyifempty
Root: HKCU; Subkey: "Software\Classes\.pnm\OpenWithProgids"; ValueType: string; ValueName: "{#AppProgId}"; ValueData: ""; Flags: uninsdeletevalue uninsdeletekeyifempty
Root: HKCU; Subkey: "Software\Classes\.hdr\OpenWithProgids"; ValueType: string; ValueName: "{#AppProgId}"; ValueData: ""; Flags: uninsdeletevalue uninsdeletekeyifempty
Root: HKCU; Subkey: "Software\Classes\.exr\OpenWithProgids"; ValueType: string; ValueName: "{#AppProgId}"; ValueData: ""; Flags: uninsdeletevalue uninsdeletekeyifempty
Root: HKCU; Subkey: "Software\Classes\.dds\OpenWithProgids"; ValueType: string; ValueName: "{#AppProgId}"; ValueData: ""; Flags: uninsdeletevalue uninsdeletekeyifempty
Root: HKCU; Subkey: "Software\Classes\.avif\OpenWithProgids"; ValueType: string; ValueName: "{#AppProgId}"; ValueData: ""; Flags: uninsdeletevalue uninsdeletekeyifempty
Root: HKCU; Subkey: "Software\Classes\.heic\OpenWithProgids"; ValueType: string; ValueName: "{#AppProgId}"; ValueData: ""; Flags: uninsdeletevalue uninsdeletekeyifempty
Root: HKCU; Subkey: "Software\Classes\.heif\OpenWithProgids"; ValueType: string; ValueName: "{#AppProgId}"; ValueData: ""; Flags: uninsdeletevalue uninsdeletekeyifempty

; ProgID used by Explorer and the "Open with" menu.
Root: HKCU; Subkey: "Software\Classes\{#AppProgId}"; ValueType: string; ValueData: "image-view image"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\{#AppProgId}\DefaultIcon"; ValueType: string; ValueData: "{app}\{#AppExeName},0"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\{#AppProgId}\shell\open\command"; ValueType: string; ValueData: """{app}\{#AppExeName}"" ""%1"""; Flags: uninsdeletekey

; Applications\image-view.exe entry (also feeds the "Open with" list).
Root: HKCU; Subkey: "Software\Classes\Applications\{#AppExeName}"; ValueType: string; ValueData: "{#AppName}"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\Applications\{#AppExeName}\shell\open\command"; ValueType: string; ValueData: """{app}\{#AppExeName}"" ""%1"""; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\Applications\{#AppExeName}\SupportedTypes"; ValueType: string; ValueName: ".png"; ValueData: ""; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\Applications\{#AppExeName}\SupportedTypes"; ValueType: string; ValueName: ".jpg"; ValueData: ""; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\Applications\{#AppExeName}\SupportedTypes"; ValueType: string; ValueName: ".jpeg"; ValueData: ""; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\Applications\{#AppExeName}\SupportedTypes"; ValueType: string; ValueName: ".gif"; ValueData: ""; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\Applications\{#AppExeName}\SupportedTypes"; ValueType: string; ValueName: ".webp"; ValueData: ""; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\Applications\{#AppExeName}\SupportedTypes"; ValueType: string; ValueName: ".bmp"; ValueData: ""; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\Applications\{#AppExeName}\SupportedTypes"; ValueType: string; ValueName: ".tif"; ValueData: ""; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\Applications\{#AppExeName}\SupportedTypes"; ValueType: string; ValueName: ".tiff"; ValueData: ""; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\Applications\{#AppExeName}\SupportedTypes"; ValueType: string; ValueName: ".ico"; ValueData: ""; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\Applications\{#AppExeName}\SupportedTypes"; ValueType: string; ValueName: ".qoi"; ValueData: ""; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\Applications\{#AppExeName}\SupportedTypes"; ValueType: string; ValueName: ".tga"; ValueData: ""; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\Applications\{#AppExeName}\SupportedTypes"; ValueType: string; ValueName: ".pnm"; ValueData: ""; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\Applications\{#AppExeName}\SupportedTypes"; ValueType: string; ValueName: ".hdr"; ValueData: ""; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\Applications\{#AppExeName}\SupportedTypes"; ValueType: string; ValueName: ".exr"; ValueData: ""; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\Applications\{#AppExeName}\SupportedTypes"; ValueType: string; ValueName: ".dds"; ValueData: ""; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\Applications\{#AppExeName}\SupportedTypes"; ValueType: string; ValueName: ".avif"; ValueData: ""; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\Applications\{#AppExeName}\SupportedTypes"; ValueType: string; ValueName: ".heic"; ValueData: ""; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\Applications\{#AppExeName}\SupportedTypes"; ValueType: string; ValueName: ".heif"; ValueData: ""; Flags: uninsdeletekey

; Default Apps capability registration (Windows 10/11 Settings page).
Root: HKCU; Subkey: "{#AppRegRoot}\Capabilities"; ValueType: string; ValueName: "ApplicationName"; ValueData: "{#AppName}"; Flags: uninsdeletekey
Root: HKCU; Subkey: "{#AppRegRoot}\Capabilities"; ValueType: string; ValueName: "ApplicationDescription"; ValueData: "Lightweight image viewer"; Flags: uninsdeletekey
Root: HKCU; Subkey: "{#AppRegRoot}\Capabilities\FileAssociations"; ValueType: string; ValueName: ".png"; ValueData: "{#AppProgId}"; Flags: uninsdeletekey
Root: HKCU; Subkey: "{#AppRegRoot}\Capabilities\FileAssociations"; ValueType: string; ValueName: ".jpg"; ValueData: "{#AppProgId}"; Flags: uninsdeletekey
Root: HKCU; Subkey: "{#AppRegRoot}\Capabilities\FileAssociations"; ValueType: string; ValueName: ".jpeg"; ValueData: "{#AppProgId}"; Flags: uninsdeletekey
Root: HKCU; Subkey: "{#AppRegRoot}\Capabilities\FileAssociations"; ValueType: string; ValueName: ".gif"; ValueData: "{#AppProgId}"; Flags: uninsdeletekey
Root: HKCU; Subkey: "{#AppRegRoot}\Capabilities\FileAssociations"; ValueType: string; ValueName: ".webp"; ValueData: "{#AppProgId}"; Flags: uninsdeletekey
Root: HKCU; Subkey: "{#AppRegRoot}\Capabilities\FileAssociations"; ValueType: string; ValueName: ".bmp"; ValueData: "{#AppProgId}"; Flags: uninsdeletekey
Root: HKCU; Subkey: "{#AppRegRoot}\Capabilities\FileAssociations"; ValueType: string; ValueName: ".tif"; ValueData: "{#AppProgId}"; Flags: uninsdeletekey
Root: HKCU; Subkey: "{#AppRegRoot}\Capabilities\FileAssociations"; ValueType: string; ValueName: ".tiff"; ValueData: "{#AppProgId}"; Flags: uninsdeletekey
Root: HKCU; Subkey: "{#AppRegRoot}\Capabilities\FileAssociations"; ValueType: string; ValueName: ".ico"; ValueData: "{#AppProgId}"; Flags: uninsdeletekey
Root: HKCU; Subkey: "{#AppRegRoot}\Capabilities\FileAssociations"; ValueType: string; ValueName: ".qoi"; ValueData: "{#AppProgId}"; Flags: uninsdeletekey
Root: HKCU; Subkey: "{#AppRegRoot}\Capabilities\FileAssociations"; ValueType: string; ValueName: ".tga"; ValueData: "{#AppProgId}"; Flags: uninsdeletekey
Root: HKCU; Subkey: "{#AppRegRoot}\Capabilities\FileAssociations"; ValueType: string; ValueName: ".pnm"; ValueData: "{#AppProgId}"; Flags: uninsdeletekey
Root: HKCU; Subkey: "{#AppRegRoot}\Capabilities\FileAssociations"; ValueType: string; ValueName: ".hdr"; ValueData: "{#AppProgId}"; Flags: uninsdeletekey
Root: HKCU; Subkey: "{#AppRegRoot}\Capabilities\FileAssociations"; ValueType: string; ValueName: ".exr"; ValueData: "{#AppProgId}"; Flags: uninsdeletekey
Root: HKCU; Subkey: "{#AppRegRoot}\Capabilities\FileAssociations"; ValueType: string; ValueName: ".dds"; ValueData: "{#AppProgId}"; Flags: uninsdeletekey
Root: HKCU; Subkey: "{#AppRegRoot}\Capabilities\FileAssociations"; ValueType: string; ValueName: ".avif"; ValueData: "{#AppProgId}"; Flags: uninsdeletekey
Root: HKCU; Subkey: "{#AppRegRoot}\Capabilities\FileAssociations"; ValueType: string; ValueName: ".heic"; ValueData: "{#AppProgId}"; Flags: uninsdeletekey
Root: HKCU; Subkey: "{#AppRegRoot}\Capabilities\FileAssociations"; ValueType: string; ValueName: ".heif"; ValueData: "{#AppProgId}"; Flags: uninsdeletekey

Root: HKCU; Subkey: "Software\RegisteredApplications"; ValueType: string; ValueName: "{#AppName}"; ValueData: "{#AppRegRoot}\Capabilities"; Flags: uninsdeletevalue uninsdeletekeyifempty

[UninstallDelete]
; Remove user-visible data created by the app. Keep nothing behind.
Type: filesandordirs; Name: "{localappdata}\image-view\thumbs"
Type: filesandordirs; Name: "{localappdata}\image-view\logs"
Type: dirifempty; Name: "{localappdata}\image-view"
Type: filesandordirs; Name: "{userappdata}\image-view"

[Code]
const
  DefaultViewerTask = 'setdefault';
  BackupKey = '{#AppBackupKey}';

function Extensions: TArrayOfString;
begin
  Result := [
    '.png', '.jpg', '.jpeg', '.gif', '.webp', '.bmp', '.tif', '.tiff', '.ico',
    '.qoi', '.tga', '.pnm', '.hdr', '.exr', '.dds', '.avif', '.heic', '.heif'
  ];
end;

procedure BackupDefaultAssociation(const Ext: String);
var
  Current: String;
begin
  if RegQueryStringValue(HKCU, 'Software\Classes\' + Ext, '', Current)
     and (CompareText(Current, '{#AppProgId}') <> 0) then
    RegWriteStringValue(HKCU, BackupKey + '\' + Ext, 'value', Current);
end;

procedure BackupAssociations();
var
  Exts: TArrayOfString;
  Index: Integer;
begin
  if not WizardIsTaskSelected(DefaultViewerTask) then
    Exit;
  Exts := Extensions();
  for Index := 0 to GetArrayLength(Exts) - 1 do
    BackupDefaultAssociation(Exts[Index]);
end;

procedure RestoreDefaultAssociation(const Ext: String);
var
  Current: String;
  Backup: String;
begin
  if not RegQueryStringValue(HKCU, BackupKey + '\' + Ext, 'value', Backup) then
    Exit;
  { Do not overwrite a viewer the user picked after installing. }
  if (not RegQueryStringValue(HKCU, 'Software\Classes\' + Ext, '', Current))
     or (CompareText(Current, '{#AppProgId}') = 0) then
    RegWriteStringValue(HKCU, 'Software\Classes\' + Ext, '', Backup);
end;

procedure RestoreAssociations();
var
  Exts: TArrayOfString;
  Index: Integer;
begin
  Exts := Extensions();
  for Index := 0 to GetArrayLength(Exts) - 1 do
    RestoreDefaultAssociation(Exts[Index]);
  RegDeleteKeyIncludingSubkeys(HKCU, BackupKey);
  RegDeleteKeyIfEmpty(HKCU, '{#AppRegRoot}');
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssInstall then
    BackupAssociations();
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
    RestoreAssociations();
end;
