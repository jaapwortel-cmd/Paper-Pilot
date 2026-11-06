# Paper Pilot

Gratis, open source programma voor Windows (Rust), gemaakt door Jaap Wortel: brieven lezen en formulieren
invullen voor blinden en slechtzienden, en controleren of documenten en websites toegankelijk zijn.
Het volledige plan staat in `PLAN.md`; volg dat stap voor stap.

## Met Jaap praten

- Nederlands, in eenvoudige taal, met "je" en "jij" (geen "u").
- Korte uitleg in duidelijke, genummerde stappen. Jaap schrijft zelf geen code.

## Afspraken

- Deze repository is OPENBAAR. Zet hier niets persoonlijks of geheims in (geen wachtwoorden, sleutels,
  persoonlijke gegevens of privézaken van Jaap), ook niet in commentaar of commitberichten.
- Volledig toegankelijk (NVDA, JAWS, Verteller): gewone Windows-onderdelen, alles met het toetsenbord,
  menubalk, AccessibleName op alles. Mooi voor zienden zonder hinder voor de schermlezer.
- Talen: Nederlands en Engels; nieuwe teksten altijd in allebei, handleiding in allebei.
- Privé: brieven en gegevens blijven op de computer. Geen Microsoft Edge of WebView2; iets met een
  browser gaat naar Firefox of de standaardbrowser.
- Sneltoetsen: NOOIT de Insert-toets of Caps Lock gebruiken (toetsen van NVDA en JAWS). Zolang Insert of
  Caps Lock ingedrukt is, reageert Paper Pilot op geen enkele sneltoets (Jaap, 6 nov 2026).
- Licentie: GPL-3.0.
- Niet ondertekenen (geen SignPath, geen certificaat): Jaap vindt dat te veel handelingen (6 nov 2026).
- Alleen de naam Jaap Wortel als maker in het programma (geen andere namen of organisaties).
- "Mail de maker" gaat naar jaap.wortel@gmail.com (Jaaps keuze, 6 nov 2026; kan later een ander adres worden).
- Updates: Dropbox-map `/Updates van mijn programma's/Paper Pilot updates` 
  Link (alleen bekijken, 6 nov 2026): https://www.dropbox.com/scl/fo/dgj0om0qkz4al43owg91c/AENyj6dSYBH7sdMj1VOE96I?rlkey=vhtmwomkhbzgvhb0kaffpyeex&dl=0
  Nooit een link met bewerkrechten gebruiken: updates draaien met beheerdersrechten.
- Versies: de eerste is 0.0.0.1, daarna 0.0.0.2, 0.0.0.3 enz. Geen "PROEF" in de naam (Jaap, 6 nov 2026).
- Werkwijze: Jaap test elke versie; pas verder als Jaap zegt dat het werkt. Bij elke stap de handleiding
  en de wijzigingen bijwerken.
- Wijzigingen verzamelen en samen pushen; altijd pushen voordat Jaap moet testen of de sessie stopt.
- Tekstbestanden: UTF-8 met BOM en CRLF (zie `.gitattributes`).

## Bouwen

- Rust (windows-sys 0.61, zip 2). `versie.txt` is het enige versienummer (vier delen); build.rs zet het in de exe
  (versie-informatie), het manifest (comctl32 v6, PerMonitorV2) en `PP_VERSIE`. Bij elke nieuwe versie: versie.txt
  ophogen en een blok "Versie x.x.x.x" bovenaan in teksten/wijzigingen-nl.txt en -en.txt.
- Code in `src/`: hoofdvenster (tegels zelf getekend, BS_OWNERDRAW), tekstvenster (alleen-lezen tekstvak),
  dialogen (TaskDialog: rondleiding, meer programma's), bijwerken (Dropbox-zip, versie uit het installatieprogramma,
  runas /SILENT), thema, teksten, instellingen (%APPDATA%\Paper Pilot), logboek, proef.
- In de staat van het hoofdvenster (`met`) nooit Windows-functies aanroepen die berichten sturen; gebruik `lees()`.
- Teksten in `teksten/nl.txt` en `en.txt` (Sleutel = tekst); de proefstart faalt als een sleutel in één taal ontbreekt.
- Lokaal: `cargo build --release --target x86_64-pc-windows-gnu` (mingw-w64) en proberen met Wine + Xvfb
  (`wine PaperPilot.exe --proefstart proef.txt`, `--schermafbeelding foto.bmp --thema licht --taal nl`).
  Wine heeft geen Segoe MDL2 Assets (symbolen op de tegels zijn daar vierkantjes).
- GitHub (`.github/workflows/build.yml`, windows-latest, gratis voor openbare repo's): bouwen, proefstart,
  `tools/toegankelijkheid.ps1` (UI Automation: elk focusbaar onderdeel heeft een naam), schermafbeeldingen,
  Inno Setup (`installer/PaperPilot.iss`), proef installeren/bijwerken/verwijderen, daarna naar de tak `exe-builds`.
- Toegankelijkheidsproef: op de bouwcomputer van GitHub werken de UIA-proxy's niet (alles is "Pane"), MSAA wel.
  Daarom is MSAA (C# in tools/toegankelijkheid.ps1, Windows PowerShell 5.1) de harde controle.
- Installatieproef: niet `Start-Process -Wait` op het installatieprogramma (dat wacht ook op het gestarte Paper Pilot).

## Stand van zaken

- 0.0.0.1 (6 nov 2026): stap 1, de basis. Bouwronde helemaal groen (proefstart, MSAA, schermafbeeldingen,
  installeren/bijwerken/verwijderen). Download: tak exe-builds. Jaap test nu.
  Updates zoeken werkte eerst niet (URLDownloadToFile: 0x800c0008 bij Dropbox); nu WinHTTP, en de bouwronde
  controleert de Dropbox-map echt. Jaap kreeg daarna een nieuwe 0.0.0.1 (zelfde versienummer) voor Dropbox.
- 0.0.0.2 (6 nov 2026): stap 2, documenten lezen. `lezen.rs` (PDF via pdfium-render + pdfium.dll uit de PyPI-wheel
  pypdfium2 5.15.0, in {app}\onderdelen; koppen = grotere letter of korte vette regel; teken 2 = afbreekstreepje;
  Word via quick-xml; tekst), `leesvenster.rs` (Ctrl+O/F/K/G/S, F3, Escape; openen op de achtergrond),
  `keuzevensters.rs` (vraag_tekst, kies_uit_lijst). Testbestanden in `tests/` (gemaakt met reportlab en python-docx).
  "Openen met" voor .pdf/.docx/.txt in de installer. Insert/Caps Lock: geen sneltoetsen (Jaap).
