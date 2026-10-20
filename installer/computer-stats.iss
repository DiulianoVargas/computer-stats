; Computer Stats installer (Inno Setup 6). Built by scripts/package.ps1, which defines AppVersion,
; ExePath and PackageDir; do not compile this file directly.

#ifndef AppVersion
  #error Run scripts/package.ps1 instead of compiling this script directly.
#endif

[Setup]
AppId={{6C1E2B7A-3F4D-4E9A-9B2C-8D5F0A1E7C34}
AppName=Computer Stats
AppVersion={#AppVersion}
AppVerName=Computer Stats {#AppVersion}
AppPublisher=Diuliano Vargas
AppPublisherURL=https://github.com/DiulianoVargas/computer-stats
AppSupportURL=https://github.com/DiulianoVargas/computer-stats/issues
DefaultDirName={autopf}\Computer Stats
DefaultGroupName=Computer Stats
DisableProgramGroupPage=yes
; The app always runs elevated (ADR-0002), and uninstalling must remove its elevated
; Start with Windows task, so installation is per machine.
PrivilegesRequired=admin
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.17763
OutputDir={#PackageDir}
OutputBaseFilename=ComputerStats-Setup-{#AppVersion}
SetupIconFile=..\assets\app.ico
UninstallDisplayIcon={app}\computer-stats.exe
UninstallDisplayName=Computer Stats
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
; Ask to close a running Computer Stats before installing or uninstalling (its single-instance
; mutex lives in the session namespace, "Local\ComputerStats.SingleInstance").
AppMutex=ComputerStats.SingleInstance
CloseApplications=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "brazilianportuguese"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"

[CustomMessages]
english.LaunchApp=Launch Computer Stats
brazilianportuguese.LaunchApp=Abrir o Computer Stats
english.DesktopIcon=Create a desktop shortcut
brazilianportuguese.DesktopIcon=Criar atalho na área de trabalho
english.InstallPawnIO=Install PawnIO (needed for CPU Temperature; downloaded from its publisher)
brazilianportuguese.InstallPawnIO=Instalar o PawnIO (necessário para a temperatura da CPU; baixado do site oficial)
english.PawnIODownloading=Downloading PawnIO...
brazilianportuguese.PawnIODownloading=Baixando o PawnIO...
english.PawnIOFailed=PawnIO could not be installed (%1). Computer Stats works without it, except CPU Temperature.%n%nOpen the PawnIO website to install it manually?
brazilianportuguese.PawnIOFailed=Não foi possível instalar o PawnIO (%1). O Computer Stats funciona sem ele, exceto a temperatura da CPU.%n%nAbrir o site do PawnIO para instalar manualmente?

[Tasks]
Name: "desktopicon"; Description: "{cm:DesktopIcon}"; Flags: unchecked
; Offered, ticked, only when PawnIO is not installed (004 FR-005).
Name: "pawnio"; Description: "{cm:InstallPawnIO}"; Check: not PawnIOInstalled

[Files]
Source: "{#ExePath}"; DestDir: "{app}"; Flags: ignoreversion
; CPU Temperature modules (PawnIO.Modules, LGPL-2.1). The PawnIO driver is not bundled (its setup
; is proprietary freeware); the "pawnio" task downloads the official setup instead (004 FR-006).
Source: "{#PackageDir}\module\AMDFamily17.bin"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PackageDir}\module\IntelMSR.bin"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PackageDir}\THIRD-PARTY-NOTICES.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PackageDir}\licenses\*"; DestDir: "{app}\licenses"; Flags: ignoreversion recursesubdirs

[Icons]
Name: "{autoprograms}\Computer Stats"; Filename: "{app}\computer-stats.exe"
Name: "{autodesktop}\Computer Stats"; Filename: "{app}\computer-stats.exe"; Tasks: desktopicon

[Registry]
; The Desktop Widget's on/off and position (005 research R5): removed on uninstall, never created here.
Root: HKCU; Subkey: "Software\Computer Stats"; Flags: uninsdeletekey dontcreatekey

[Run]
; shellexec: postinstall entries run as the original, non-elevated user, and CreateProcess cannot
; start an exe whose manifest asks for elevation (error 740). ShellExecute shows the UAC prompt,
; like the Start menu shortcut does.
Filename: "{app}\computer-stats.exe"; Description: "{cm:LaunchApp}"; Flags: shellexec nowait postinstall skipifsilent

[UninstallRun]
; Stop the app and remove the Start with Windows task it may have created. PawnIO stays installed:
; other programs may use it.
Filename: "{sys}\taskkill.exe"; Parameters: "/IM computer-stats.exe /F"; Flags: runhidden; RunOnceId: "StopApp"
Filename: "{sys}\schtasks.exe"; Parameters: "/Delete /TN ""Computer Stats"" /F"; Flags: runhidden; RunOnceId: "DeleteStartupTask"

[Code]
// PawnIO setup 2.2.0 as published by namazso (also winget's namazso.PawnIO): fixed URL, pinned
// SHA-256, silent switches, success codes 0 and 3010 (restart needed). 004 research R3.
const
  PawnIOUrl = 'https://github.com/namazso/PawnIO.Setup/releases/download/2.2.0/PawnIO_setup.exe';
  PawnIOSha256 = '1f519a22e47187f70a1379a48ca604981c4fcf694f4e65b734aaa74a9fba3032';
  PawnIOWebsite = 'https://pawnio.eu';

var
  DownloadPage: TDownloadWizardPage;
  PawnIODownloaded: Boolean;
  PawnIONeedsRestart: Boolean;

function PawnIOInstalled: Boolean;
begin
  Result := RegKeyExists(HKLM, 'SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\PawnIO');
end;

procedure OfferPawnIOWebsite(const Why: String);
var
  Code: Integer;
begin
  Log('PawnIO not installed: ' + Why);
  if not WizardSilent and
     (MsgBox(FmtMessage(CustomMessage('PawnIOFailed'), [Why]), mbConfirmation, MB_YESNO) = IDYES) then
    ShellExec('open', PawnIOWebsite, '', '', SW_SHOWNORMAL, ewNoWait, Code);
end;

procedure InitializeWizard;
begin
  DownloadPage := CreateDownloadPage(SetupMessage(msgWizardPreparing), CustomMessage('PawnIODownloading'), nil);
end;

// Download and verify before installing, with a progress page. A failure never stops the
// installation of Computer Stats (004 FR-007).
function NextButtonClick(CurPageID: Integer): Boolean;
begin
  Result := True;
  if (CurPageID = wpReady) and WizardIsTaskSelected('pawnio') then
  begin
    DownloadPage.Clear;
    DownloadPage.Add(PawnIOUrl, 'PawnIO_setup.exe', PawnIOSha256);
    DownloadPage.Show;
    try
      try
        DownloadPage.Download;
        PawnIODownloaded := True;
      except
        PawnIODownloaded := False;
        Log('PawnIO download failed: ' + GetExceptionMessage);
      end;
    finally
      DownloadPage.Hide;
    end;
  end;
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  Code: Integer;
begin
  if (CurStep <> ssPostInstall) or not WizardIsTaskSelected('pawnio') then
    Exit;
  if not PawnIODownloaded then
  begin
    OfferPawnIOWebsite('download failed');
    Exit;
  end;
  if not Exec(ExpandConstant('{tmp}\PawnIO_setup.exe'), '-install -silent', '', SW_HIDE,
              ewWaitUntilTerminated, Code) then
    OfferPawnIOWebsite('setup could not start')
  else if Code = 3010 then
    PawnIONeedsRestart := True
  else if Code <> 0 then
    OfferPawnIOWebsite('setup error ' + IntToStr(Code));
end;

function NeedRestart: Boolean;
begin
  Result := PawnIONeedsRestart;
end;
