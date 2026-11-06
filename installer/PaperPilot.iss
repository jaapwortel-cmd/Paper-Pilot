; Paper Pilot - installatieprogramma (Inno Setup 6). Nederlands en Engels.
; Installeert in Program Files (beheerdersrechten, één vraag van Windows).
; Een nieuwe versie ruimt eerst de oude programmabestanden op ([InstallDelete]) en zet de nieuwe ervoor in de plaats.
; Eigen gegevens staan in %APPDATA%\Paper Pilot en blijven bewaard; bij verwijderen wordt gevraagd of die ook weg moeten.
; Bouwen: iscc /DVersie=0.0.0.1 PaperPilot.iss

#ifndef Versie
  #define Versie "0.0.0.0"
#endif

[Setup]
AppId={{80376DE0-80AC-4578-A65A-937637589B0B}
AppName=Paper Pilot
AppVersion={#Versie}
AppVerName=Paper Pilot {#Versie}
AppPublisher=Jaap Wortel
AppPublisherURL=https://github.com/jaapwortel-cmd/Paper-Pilot
AppSupportURL=mailto:jaap.wortel@gmail.com
VersionInfoVersion={#Versie}
VersionInfoProductName=Paper Pilot
VersionInfoDescription=Paper Pilot installatie / setup
VersionInfoCompany=Jaap Wortel
DefaultDirName={autopf}\Paper Pilot
DisableDirPage=yes
DisableProgramGroupPage=yes
DisableWelcomePage=yes
PrivilegesRequired=admin
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
OutputDir=..\uitvoer
OutputBaseFilename=Paper Pilot {#Versie} installatie
SetupIconFile=..\res\paperpilot.ico
UninstallDisplayIcon={app}\PaperPilot.exe
UninstallDisplayName=Paper Pilot
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes
RestartApplications=no
ShowLanguageDialog=yes
UsePreviousLanguage=no
LanguageDetectionMethod=uilanguage

[Languages]
Name: "dutch"; MessagesFile: "compiler:Languages\Dutch.isl"; InfoBeforeFile: "..\teksten\privacy-nl.txt"
Name: "english"; MessagesFile: "compiler:Default.isl"; InfoBeforeFile: "..\teksten\privacy-en.txt"

[CustomMessages]
dutch.Bureaublad=Snelkoppeling op het bureaublad
english.Bureaublad=Shortcut on the desktop
dutch.Starten=Paper Pilot starten
english.Starten=Start Paper Pilot
dutch.Verwijderen=Paper Pilot verwijderen
english.Verwijderen=Uninstall Paper Pilot
dutch.GegevensWeg=Wil je ook je eigen gegevens en instellingen van Paper Pilot verwijderen?%n%nKies Nee als je Paper Pilot later weer wilt gebruiken met dezelfde gegevens.
dutch.NogOpen=Paper Pilot is nog geopend. Sluit Paper Pilot en kies dan Opnieuw.
english.NogOpen=Paper Pilot is still open. Close Paper Pilot and then choose Retry.
english.GegevensWeg=Do you also want to remove your own Paper Pilot data and settings?%n%nChoose No if you want to use Paper Pilot again later with the same data.

[Tasks]
Name: "bureaublad"; Description: "{cm:Bureaublad}"

[InstallDelete]
; Oude programmabestanden opruimen voordat de nieuwe komen (nooit de eigen gegevens: die staan niet hier).
Type: files; Name: "{app}\*.exe"
Type: files; Name: "{app}\*.dll"
Type: files; Name: "{app}\*.txt"
Type: filesandordirs; Name: "{app}\onderdelen"

[Files]
Source: "..\target\release\PaperPilot.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion
Source: "..\target\release\onderdelen\pdfium.dll"; DestDir: "{app}\onderdelen"; Flags: ignoreversion
Source: "..\res\pdfium-licentie.txt"; DestDir: "{app}\onderdelen"; Flags: ignoreversion

[Registry]
; "Openen met" in de Verkenner voor PDF, Word en tekst (Paper Pilot wordt niet het standaardprogramma).
Root: HKA; Subkey: "Software\Classes\Applications\PaperPilot.exe"; ValueType: string; ValueName: "FriendlyAppName"; ValueData: "Paper Pilot"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\Applications\PaperPilot.exe\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\PaperPilot.exe"" ""%1"""
Root: HKA; Subkey: "Software\Classes\Applications\PaperPilot.exe\SupportedTypes"; ValueType: string; ValueName: ".pdf"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\PaperPilot.exe\SupportedTypes"; ValueType: string; ValueName: ".docx"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\PaperPilot.exe\SupportedTypes"; ValueType: string; ValueName: ".txt"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\PaperPilot.exe\SupportedTypes"; ValueType: string; ValueName: ".jpg"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\PaperPilot.exe\SupportedTypes"; ValueType: string; ValueName: ".jpeg"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\PaperPilot.exe\SupportedTypes"; ValueType: string; ValueName: ".png"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\PaperPilot.exe\SupportedTypes"; ValueType: string; ValueName: ".tif"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\PaperPilot.exe\SupportedTypes"; ValueType: string; ValueName: ".tiff"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\PaperPilot.exe\SupportedTypes"; ValueType: string; ValueName: ".bmp"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\PaperPilot.exe\SupportedTypes"; ValueType: string; ValueName: ".gif"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\PaperPilot.exe\SupportedTypes"; ValueType: string; ValueName: ".heic"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\PaperPilot.exe\SupportedTypes"; ValueType: string; ValueName: ".webp"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\.pdf\OpenWithList\PaperPilot.exe"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\.docx\OpenWithList\PaperPilot.exe"; Flags: uninsdeletekey

[Icons]
Name: "{autoprograms}\Paper Pilot"; Filename: "{app}\PaperPilot.exe"
Name: "{autoprograms}\{cm:Verwijderen}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\Paper Pilot"; Filename: "{app}\PaperPilot.exe"; Tasks: bureaublad

[Run]
Filename: "{app}\PaperPilot.exe"; Description: "{cm:Starten}"; Flags: nowait postinstall runasoriginaluser

[UninstallDelete]
Type: filesandordirs; Name: "{app}\onderdelen"

[Code]
// Bij een update sluit Paper Pilot zichzelf net na het starten van dit installatieprogramma.
// Daarom eerst even wachten tot Paper Pilot echt dicht is (hooguit 15 seconden).
function WachtOpPaperPilot(): Boolean;
var
  Pogingen: Integer;
begin
  Pogingen := 0;
  while CheckForMutexes('PaperPilotDraait') and (Pogingen < 30) do
  begin
    Sleep(500);
    Pogingen := Pogingen + 1;
  end;
  Result := not CheckForMutexes('PaperPilotDraait');
end;

function InitializeSetup(): Boolean;
begin
  WachtOpPaperPilot();
  Result := True;
end;

function InitializeUninstall(): Boolean;
begin
  Result := True;
  if not WachtOpPaperPilot() then
    if not UninstallSilent then
      Result := MsgBox(CustomMessage('NogOpen'), mbError, MB_RETRYCANCEL) = IDRETRY;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if (CurUninstallStep = usPostUninstall) and (not UninstallSilent) then
    if MsgBox(CustomMessage('GegevensWeg'), mbConfirmation, MB_YESNO or MB_DEFBUTTON2) = IDYES then
      DelTree(ExpandConstant('{userappdata}\Paper Pilot'), True, True, True);
end;
