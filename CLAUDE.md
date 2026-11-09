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
- Geen eigen voorleesstem in Paper Pilot: de schermlezer leest al voor (Jaap, 7 nov 2026). Een gebouwde versie met F5 is weer weggehaald.
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
- 0.0.0.3 (6 nov 2026): stap 3a, tekstherkenning. `beeld.rs` (WIC lezen/schrijven, EXIF-stand, kwartslag, draaien),
  `herkennen.rs` (Windows.Media.Ocr via de `windows`-crate; vier standen proberen bij weinig echte woorden; TextAngle >= 1,5
  graad = rechtzetten). Gescande PDF-pagina's via pdfium renderen (300 dpi). Testscans in `tests/` (PIL). Wine heeft geen OCR;
  de bouwcomputer alleen Engelse OCR. Volgende: 0.0.0.4 beeldtaalmodel (handschrift, beschrijven), 0.0.0.5 scannen en afdrukken.
- 0.0.0.4 (6 nov 2026): stap 3b, beeldtaalmodel. `beeldmodel.rs` (llama.cpp b11669 Vulkan + Qwen3-VL-4B Q4_K_M + mmproj,
  bestanden gekozen via de Hugging Face-API, in %LOCALAPPDATA%\Paper Pilot\Taalmodel; motor in een taakgroep, stopt na
  10 min; -ngl 99, anders 0), `http.rs` (WinHTTP, hervatten met Range), `dialogen::werk` (TaskDialog met voortgangsbalk en
  Annuleren). Leesvenster: Ctrl+H handschrift, Ctrl+D beschrijven; menu Instellingen, Taalmodel. Proef: `--proef-beeldmodel`
  met SmolVLM-256M bij elke bouwronde; het echte model via Run workflow met groot_model aangevinkt.
- 0.0.0.5 (6 nov 2026): stap 4, scannen en afdrukken. `scanner.rs` (Windows.Devices.Scanners: ScanFilesToFolderAsync,
  300 dpi grijs, glasplaat/invoer; eSCL via eigen mDNS-zoeker _uscan._tcp en POST ScanJobs/NextDocument), `afdrukken.rs`
  (PrintDlgW + GDI: tekst met marges en voetregel, afbeeldingen passend), `pdf_maken.rs` (eigen PDF-schrijver: JPG per
  pagina + onzichtbare OCR-tekst, Tr 3). Leesvenster: Ctrl+N, Ctrl+P; Ctrl+Shift+S met PDF als eerste keuze; tegel vraagt
  openen of scannen. Niet te testen op GitHub: echte scanners en printers (Jaap test).
- 0.0.0.6 (6 nov 2026, Jaaps wens): enveloppen (`envelop.rs`: adresblok aan de postcode, afzender uit bekende namen,
  retouradres of linksboven; vanzelf bij liggend + weinig tekst + postcode, of als de gebruiker "envelop" kiest) en kleine
  letters (`herkennen.rs`: vergroot2 en contrast als extra poging, beste score wint). Scannen: keuze brief/envelop/klein
  (600 dpi). Testbeelden envelop.jpg, theezakje.png, bijsluiter.png.
- 0.0.0.7 (6 nov 2026): stap 5 en 6, formulieren invullen en Mijn gegevens. `formulier_pdf.rs` (eigen FFI naar pdfium.dll:
  formulier-omgeving, FORM_SetFocusedAnnot + SelectAllText + ReplaceSelection voor tekst, klikken voor vinkjes/rondjes,
  FORM_SetIndexSelected voor lijsten, FPDF_SaveAsCopy; vraag = TU, anders tekst links/rechts/boven het veld, anders de naam),
  `invulvenster.rs` (vraag voor vraag; Enter, Alt+pijlen, Ctrl+L overzicht, Ctrl+S), `gegevens.rs` (DPAPI, soort_van op
  woorden in de vraag), `keuzevensters::formulier`. Proef `--proef-invullen`. Let op: pypdfium2 tekent velden pas na init_forms().
  Ook in 0.0.0.7 (Jaap: "nog niks getest, zelfde versienummer"): formulieren zonder invulvelden. `plat.rs`: zoekt op een
  paginabeeld + regels met woorden (PDF: eigen FFI FPDFText_GetCharBox; scan: OCR-woorden in Regel.woorden) naar
  dubbele punt + lege ruimte (met kader: antwoord in het kader), □-tekens, ____/....., getekende lijnen (geen bovenrand van
  een kader, erboven leeg) en getekende hokjes. Invullen: PDF met tekst = FPDFPageObj_NewTextObj(Helvetica) op de pagina;
  scan = pdf_maken met zichtbare tekst. invulvenster::Bron (Velden/PlatPdf/PlatScan). Proef `--proef-plat`, tests/plat-*.
  Ook in 0.0.0.7 (Jaaps wens, nog niet getest): meer formaten in `formaten.rs` (RTF met kopstijlen uit de stylesheet, oud Word
  .doc via cfb + piece table, OpenDocument content.xml, xlsx met bladnamen uit workbook.xml, pptx, html, epub via container.xml + OPF-spine; DRM = melding); geen braille (Jaap wilde eerst geen epub, later toch wel).
  `qr.rs` (rxing 0.9.3 zonder image-crate: QR, streepjescodes; EPC-betaal-QR "BCD", links nooit vanzelf openen; PDF-pagina's met
  tekst worden op 150 dpi gerenderd voor QR, hooguit 20), Ctrl+Q. `taken.rs` "Wat moet ik doen?" (Ctrl+T): IBAN met mod-97,
  bedragen (belangrijkste vooraan), betalingskenmerk, uiterste datums, telefoon; samenvatting via `beeldmodel::vraag_over_tekst`
  (tekst-only, max 12000 tekens). `win::kopieer` (klembord). Testbestanden tests/voorbeeld-brief.{rtf,doc,odt,html},
  voorbeeld-tabel.*, voorbeeld-dia.*, qr-*, streepjescode.png, voorbeeld-aanslag.txt (gemaakt met LibreOffice, segno, PIL).
  Ook in 0.0.0.7 (Jaap: herinneringen en de andere ideeën, zelfde versie): `herinneringen.rs` (DPAPI-JSON herinneringen.dat;
  datums lezen/rekenen zonder crate; vanaf 3 dagen vooraf 1x per dag melden; geplande taak "Paper Pilot herinneringen" via
  schtasks /XML (UTF-16, dagelijks 10:00, StartWhenAvailable) die `--herinneringen` start; weg bij lege lijst en bij verwijderen
  ([UninstallRun])), `post.rs` (Mijn post, DPAPI mijn-post.dat, max 300, onthoud in leesvenster::geladen; menu Bestand),
  `handtekening.rs` (inkt eruit + doorzichtig, DPAPI; op PDF via FPDFPageObj_NewImageObj + veld verbergen met annot-vlag 2,
  op scans via op_beeld; "[mijn handtekening]" als waarde in het invulvenster), `versturen.rs` (MAPISendMailW uit mapi32.dll,
  anders CF_HDROP op het klembord + mailto). gegevens::lees/schrijf_versleuteld. Proeven --proef-herinneringen,
  --proef-handtekening (tests/handtekening.png, tests/formulier-handtekening.pdf). Wine: schtasks /Delete werkt daar niet.
  Ook in 0.0.0.7 (Jaap: "en nu dat gedeelte voor ziende mensen"): stap 10-12 Controleren. `controleren.rs` (keuze PDF/Word/
  website, Verslag met Punt Probleem/LetOp/Goed uit teksten "Kop || Uitleg || Oplossing", verslag als Document in het
  leesvenster via leesvenster::open_verslag (niet in Mijn post), WCAG-contrast), `controle_pdf.rs` (pdfium: tekst/scan, vreemde
  tekens, IsTagged, titel (ook nep-titels), taal, StructTree: H/H1-6, Figure zonder Alt, Table zonder TH; bladwijzers, velden
  zonder TU, rechten), `controle_word.rs` (docx-XML: titel, taal, kopstijlen + nep-koppen, docPr descr/decorative, tblHeader,
  samengevoegde cellen, contrast, sz<18, lege alinea's, vage links, handlijsten, tekstvakken), `controle_web.rs` (eigen HTML-
  tokenizer, geen JavaScript: lang, title, alt, links/knoppen/velden zonder naam, koppen, main, viewport-zoom, iframes, dubbele
  id's, tabindex, autoplay, inline contrast). Proef `--proef-controleren <bestand of url> <verslag>`, tests/controle-*.
  Jaap: "kijk naar de WCAG-richtlijnen bij Bartimeus.nl": daar staat geen eigen lijst (Bartiméus volgt WCAG via Stichting Accessibility).
  Daarom aangevuld met WCAG 2.2 AA: web taalcode geldig, th in tabellen, fieldset bij rondjes, autocomplete (1.3.5), track-ondertiteling,
  marquee/blink, meta refresh (2.2.1), onclick zonder rol/tabindex (2.1.1), aria-hidden op focusbaar (4.1.2), svg role=img zonder naam;
  Word: uitgevulde tekst (jc both) en hoofdletters (caps). bartimeus.nl staat in de toegestane hosts van de omgeving.
- 0.0.0.8 (7 nov 2026, Jaap: "maak er een echt versienummer van"): de eerste versie om te verspreiden (alles van 0.0.0.7).
  De bouwronde maakt ook "Paper Pilot x installatie.zip". Dropbox-MCP kan geen .exe/.zip uploaden en geen openbare link maken:
  Jaap zet de bestanden zelf in de updatemap; de openbare link is de bestaande link van de updatemap.
  Jaap: "als je iets vindt om het toegankelijker te maken, mag je dat gewoon doen". Gedaan: lettergrootte in het leesvenster
  (Ctrl+plus/min/0, instelling tekstgrootte, ook tekstvenster), Help, Sneltoetsen (SneltoetsenTekst), pdf_maken met /Lang, DisplayDocTitle en /Info /Title.
- 0.0.0.9 (8 nov 2026, Jaap: "bouw alle 5 er maar in"): `camera.rs` (MediaCapture-foto's ~1/s, `beoordeel`: Otsu-papier,
  randen geraakt -> links/rechts/hoger/lager/dichtbij (links op de foto = links van de gebruiker bij een camera boven het scherm),
  donker, wazig/geen tekst via donkere punten + spreiding; 2x Goed = foto; tests/camera-*.png gemaakt met PIL, echte webcam niet
  getest), `melden.rs` (UiaRaiseNotificationEvent via UiaHostProviderFromHwnd op het voortgangsvenster, Voortgang.venster),
  `agenda.rs` (datum + tijd + woorden als afspraak/uitgenodigd/Tijd:, Locatie:/Adres:; .ics met VALARM -P1D, openen met het
  standaardprogramma; Ctrl+E), `schrijven.rs` (soorten brieven, sjabloon met Mijn gegevens + kenmerk, of taalmodel via
  beeldmodel::vraag_vrij; schrijfvenster met eigen subclass: Enter = nieuwe regel, Tab/Escape niet; eigen docx met titel en taal;
  Ctrl+B), vertalen in leesvenster (stukken van 2500 tekens, Ctrl+Shift+T), Mijn post met soorten (post::soort_van) en zoeken.
  Proeven `--proef-camera tests`, `--proef-afspraken tests/voorbeeld-afspraak.txt`.
  Ook in 0.0.0.9: vraag bij afsluiten (WM_CLOSE, instelling vraag_afsluiten). 0.0.0.9 staat in de Dropbox-updatemap (8 nov 2026);
  Sinds 8 nov 2026 (avond) staat daar "Paper Pilot 0.0.0.9 installatie.zip" met een eigen link (dl=0, Dropbox-pagina met
  Downloaden; werkt beter op telefoons en in Facebook/WhatsApp); de zip van 0.0.0.8 is weg. Het bericht vermeldt: alleen Windows.
  Controle van links en de updatemap: workflow link-controleren.yml (Run workflow met de link; ?dl=1).
- 0.0.0.10 (9 nov 2026, Jaap: Brother-scanner niet gevonden, geen foutmelding; Meer programma's deed ogenschijnlijk niets):
  `scanner::zoek` geeft `Zoekuitkomst` (scanners + andere = printers zonder bruikbare scanner). Alles tegelijk op draden met
  tijdslimiet: WIA (Windows.Devices.Scanners), DNS-SD via Windows (AEP-service, _uscan/_uscans/_ipp/_ipps/_pdl-datastream/_printer),
  eigen mDNS op elke netwerkkaart (IP_MULTICAST_IF), printers uit EnumPrintersW (IP uit de poortnaam), en eSCL-proef op elk IP
  (http :80 en https :443 /eSCL/ScannerCapabilities). http.rs: lokale adressen zonder proxy en zonder certificaatcontrole.
  Geen scanner: melding met gevonden printers + Opnieuw zoeken / Printers en scanners (ms-settings:printers) / camera.
  Meer programma's: lijst -> uitleg (Prog*Uitleg) met knop Jaap mailen (eerst melding dat het mailprogramma opent) of terug;
  R-Wave Studio erbij. Andere programma's van Jaap (privé of persoonlijk) bewust niet in de lijst; Jaap beslist.
  Ook in 0.0.0.10 (Jaap: website controleren deed niets): `dialogen::werk` sluit het voortgangsvenster met WM_CLOSE; TaskDialog
  meldt dat als TDN_BUTTON_CLICKED IDCANCEL, dus gestopt = true en de uitkomst ging verloren (alle 16 werk-aanroepen). Nu telt de
  uitkomst altijd, en een paniek in het werk geeft een melding (WerkMislukt*). Proef `--proef-werk` (harde stap in build.yml);
  in Wine faalt de oude code en slaagt de nieuwe. Vraag bij afsluiten altijd (menu-vinkje weg, Jaap).
  Lokaal testen: apt wine64 xvfb mingw-w64 xdotool; pdfium.dll uit de wheel in target/x86_64-pc-windows-gnu/release/onderdelen.
  Wine toont geen command links in TaskDialog (alleen Cancel), dus die vensters zijn daar niet aan te klikken.
- 0.0.0.11 (9 nov 2026, Jaap: scannen gaf 'asynchrone bewerking niet correct gestart (0x80000019)'; wens: logboek mailen):
  `join()` van windows-future vraagt GetResults ook bij AsyncStatus::Error; dan geeft Windows E_ILLEGAL_METHOD_CALL en de echte
  WIA-fout gaat verloren. scanner.rs: `klaar()` pollt IAsyncInfo.Status en geeft ErrorCode; `uitleg()` zet WIA_ERROR_* om in
  ScanFout*-teksten. Scan: eerst eigen instellingen, dan standaardinstellingen (bron Default), dan eSCL-reserve (Scanner.reserve =
  netwerkadres van dezelfde scanner). Andere WinRT-.join() (camera, OCR) zijn niet aangepast.
  Logboek: Help, Logboek naar Jaap mailen (uitleg + Mailen/Eerst bekijken; versturen::mail_aan met MAPI-ontvanger SMTP:,
  anders klembord + mailto:) en Logboek bekijken (tekstvenster). logboek::inhoud = Windows-versie + laatste 300 kB vorige + huidig.
  Elke waarschuwing (dialogen::melding met ICOON_WAARSCHUWING) komt in het logboek. teksten.rs zet nu ook \t om (menu-sneltoetsen).
  Ook in 0.0.0.11 (Jaap: 'scannen moet echt werken'; eerste start printers/scanners zoeken en standaard onthouden):
  `wia.rs`: klassieke WIA Automation (WIA.DeviceManager, IDispatch late binding met eigen Var/roep, eigen STA-draad):
  scanners() (Type 1, 3086-capaciteiten) en scan() (3088 invoer/glas, 6146 grijs, 6147/6148 dpi, Transfer BMP, SaveFile).
  Scanner heeft `wia: Option<String>` (zelfde naam) en Soort::Wia. Volgorde: WinRT eigen -> WIA -> WinRT standaard -> eSCL.
  Proef `--proef-wia` (Scripting.Dictionary test de IDispatch-hulpjes; WIA zelf ter informatie) is een harde stap in build.yml.
  Standaardscanner: JSON in instelling standaard_scanner (Scanner::naar_tekst/van_tekst); kies_scanner gebruikt hem zonder
  zoeken; bij een scanfout Opnieuw proberen / Scanner opnieuw zoeken. Standaardprinter: instelling standaard_printer,
  afdrukken::kies_printer zet DEVNAMES vooraf en onthoudt de keuze. hoofdvenster::apparaten_instellen: bij instelling
  apparaten_gezocht = false (eerste start, of eerste keer na deze update), en menu Instellingen, Printer en scanner kiezen.
  Echte scanner/printer niet getest (Wine heeft geen WIA); Jaap test met zijn Brother.
  Jaap (9 nov 2026): 0.0.0.11 getest, alles werkt (ook scannen met zijn Brother); in main gezet. Hij zet hem zelf in de Dropbox-updatemap.
