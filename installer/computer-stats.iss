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
english.GetPawnIO=Open the PawnIO website (needed only for CPU Temperature)
brazilianportuguese.GetPawnIO=Abrir o site do PawnIO (necessário só para a temperatura da CPU)
english.DesktopIcon=Create a desktop shortcut
brazilianportuguese.DesktopIcon=Criar atalho na área de trabalho

[Tasks]
Name: "desktopicon"; Description: "{cm:DesktopIcon}"; Flags: unchecked

[Files]
Source: "{#ExePath}"; DestDir: "{app}"; Flags: ignoreversion
; CPU Temperature module (PawnIO.Modules, LGPL-2.1); the PawnIO driver itself is installed
; separately by the user.
Source: "{#PackageDir}\module\AMDFamily17.bin"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PackageDir}\THIRD-PARTY-NOTICES.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PackageDir}\licenses\*"; DestDir: "{app}\licenses"; Flags: ignoreversion recursesubdirs

[Icons]
Name: "{autoprograms}\Computer Stats"; Filename: "{app}\computer-stats.exe"
Name: "{autodesktop}\Computer Stats"; Filename: "{app}\computer-stats.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\computer-stats.exe"; Description: "{cm:LaunchApp}"; Flags: nowait postinstall skipifsilent
Filename: "https://pawnio.eu"; Description: "{cm:GetPawnIO}"; Flags: shellexec postinstall skipifsilent unchecked

[UninstallRun]
; Stop the app and remove the Start with Windows task it may have created.
Filename: "{sys}\taskkill.exe"; Parameters: "/IM computer-stats.exe /F"; Flags: runhidden; RunOnceId: "StopApp"
Filename: "{sys}\schtasks.exe"; Parameters: "/Delete /TN ""Computer Stats"" /F"; Flags: runhidden; RunOnceId: "DeleteStartupTask"
