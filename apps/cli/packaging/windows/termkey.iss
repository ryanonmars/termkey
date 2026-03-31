#define MyAppName "TermKey"
#define MyAppExeName "termkey.exe"
#define IconFileName "termkey.ico"
#define ScriptDir AddBackslash(SourcePath)
#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef SourceDir
  #define SourceDir "."
#endif
#ifndef OutputDir
  #define OutputDir "."
#endif

[Setup]
AppId=TermKey
AppName={#MyAppName}
AppVersion={#AppVersion}
AppPublisher=RyanOnMars
AppPublisherURL=https://github.com/ryanonmars/termkey
AppSupportURL=https://github.com/ryanonmars/termkey/issues
AppUpdatesURL=https://github.com/ryanonmars/termkey/releases
DefaultDirName={localappdata}\termkey
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
ChangesEnvironment=yes
Compression=lzma
SolidCompression=yes
OutputDir={#OutputDir}
OutputBaseFilename=TermKey-Setup
SetupIconFile={#ScriptDir + IconFileName}
UninstallDisplayIcon={app}\{#IconFileName}
WizardStyle=modern

[Files]
Source: "{#SourceDir}\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#ScriptDir + IconFileName}"; DestDir: "{app}"; Flags: ignoreversion

[Code]
function NormalizePath(const Value: String): String;
begin
  Result := RemoveBackslashUnlessRoot(Trim(Value));
end;

function PathEntryEquals(const A, B: String): Boolean;
begin
  Result := CompareText(NormalizePath(A), NormalizePath(B)) = 0;
end;

function NextPathEntry(var Remaining: String): String;
var
  SeparatorPos: Integer;
begin
  SeparatorPos := Pos(';', Remaining);
  if SeparatorPos = 0 then
  begin
    Result := Trim(Remaining);
    Remaining := '';
  end
  else
  begin
    Result := Trim(Copy(Remaining, 1, SeparatorPos - 1));
    Delete(Remaining, 1, SeparatorPos);
  end;
end;

function PathContainsDir(const PathValue, Dir: String): Boolean;
var
  Remaining: String;
  Entry: String;
begin
  Result := False;
  Remaining := PathValue;

  while Remaining <> '' do
  begin
    Entry := NextPathEntry(Remaining);
    if (Entry <> '') and PathEntryEquals(Entry, Dir) then
    begin
      Result := True;
      Exit;
    end;
  end;
end;

function AddDirToPath(const PathValue, Dir: String): String;
begin
  if PathValue = '' then
    Result := Dir
  else if PathContainsDir(PathValue, Dir) then
    Result := PathValue
  else
    Result := PathValue + ';' + Dir;
end;

function RemoveDirFromPath(const PathValue, Dir: String): String;
var
  Remaining: String;
  Entry: String;
begin
  Result := '';
  Remaining := PathValue;

  while Remaining <> '' do
  begin
    Entry := NextPathEntry(Remaining);
    if (Entry <> '') and not PathEntryEquals(Entry, Dir) then
    begin
      if Result <> '' then
        Result := Result + ';';
      Result := Result + Entry;
    end;
  end;
end;

procedure UpdateUserPath(const AddValue: Boolean);
var
  PathValue: String;
  NewPathValue: String;
  AppDir: String;
begin
  AppDir := ExpandConstant('{app}');
  if not RegQueryStringValue(HKCU, 'Environment', 'Path', PathValue) then
    PathValue := '';

  if AddValue then
    NewPathValue := AddDirToPath(PathValue, AppDir)
  else
    NewPathValue := RemoveDirFromPath(PathValue, AppDir);

  if NewPathValue = PathValue then
    Exit;

  if NewPathValue = '' then
    RegDeleteValue(HKCU, 'Environment', 'Path')
  else
    RegWriteExpandStringValue(HKCU, 'Environment', 'Path', NewPathValue);
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then
    UpdateUserPath(True);
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usUninstall then
    UpdateUserPath(False);
end;
