# Paper Pilot – het plan

*Brieven lezen en formulieren invullen, helemaal zelf.*

Paper Pilot is een gratis programma voor Windows, gemaakt door Jaap Wortel. Het heeft twee delen:

1. **Lezen en invullen** – voor blinden en slechtzienden. Brieven scannen of openen en voorlezen,
   formulieren vraag voor vraag invullen, ondertekenen en versturen.
2. **Controleren** – voor ziende mensen die een PDF, Word-bestand of website maken. Paper Pilot zegt
   in eenvoudige taal wat een schermlezer niet goed kan lezen, en hoe je dat oplost.

Voorbeeld en proef: de formulieren van zorgkantoren (CZ, Zilveren Kruis, Menzis, Anderzorg enz.),
bijvoorbeeld het wijzigingsformulier pgb en het budgetplan pgb Wlz.

## Belangrijke afspraken

- **Toegankelijk:** NVDA, JAWS en Verteller. Gewone Windows-onderdelen, alles met het toetsenbord,
  een menubalk, duidelijke namen voor elk onderdeel, meldingen via UI Automation.
- **Muis en toetsenbord:** alles werkt met de muis én met het toetsenbord. De mooie opmaak mag de
  schermlezer nooit hinderen (versieringen zijn niet focusbaar en hebben geen naam).
- **Dicteren in elk invulveld:** andere dicteerprogramma's (Spraak naar tekst, Wispr Flow, Dragon
  NaturallySpeaking, Windows-dicteren met Windows+H en andere) moeten in elk invulveld kunnen schrijven.
  Daarom: echte Windows-invulvakken (Edit/RichEdit, met TSF voor Dragon), plakken (Ctrl+V, WM_PASTE)
  en getypte tekens (SendInput, ook Unicode) altijd toegestaan, UI Automation Value- en TextPattern,
  geen globale sneltoetsen die met dicteerprogramma's botsen, en nooit vanzelf naar het volgende veld
  springen terwijl iemand dicteert. De proef op GitHub test plakken en typen in de velden.
- **Mooi voor zienden:** rustige kleuren, ronde hoeken, grote duidelijke knoppen, licht en donker,
  hoog contrast van Windows wordt gevolgd. GitHub maakt bij elke bouwronde schermafbeeldingen.
- **Snel:** het programma start in een fractie van een seconde.
- **Privé:** alles gebeurt op de eigen computer. Brieven en gegevens gaan nooit naar internet.
  Alleen het controleren van een website en het zoeken naar updates gebruiken internet.
  Dat staat ook in de privacyverklaring.
- **Twee talen:** Nederlands en Engels, alle teksten in een tekstbestand. Handleiding in beide talen.
- **Gratis en openbaar,** licentie GPL-3.0.

## Techniek

- Taal: **Rust**, voor Windows 10 en 11 (64-bit). Eén .exe, geen .NET nodig.
- Vensters: de crate `windows` (Win32-onderdelen, zelf mooi getekend; menu's, lijsten, invulvakken).
- PDF: **PDFium** (crate `pdfium-render` met `pdfium.dll` van pdfium-binaries): tekst lezen, pagina's
  tekenen, invulvelden lezen en invullen, tekst en handtekening op de pagina zetten, opslaan.
- Tekstherkenning (OCR): de ingebouwde tekstherkenning van Windows (`Windows.Media.Ocr`), zonder internet.
  Valt die tegen of ontbreekt de taal, dan Tesseract als reserve.
- Scannen: Paper Pilot zoekt zelf naar scanners: aangesloten (USB) en in het netwerk.
  WIA (Windows Image Acquisition) voor alle scanners die Windows kent, ook netwerkscanners (WSD).
  Daarnaast eSCL/AirScan (mDNS `_uscan._tcp`): de meeste netwerkprinters met scanner werken daarmee
  zonder stuurprogramma. Vond hij er één, dan wordt die onthouden.
- Gegevens versleutelen: DPAPI van Windows (alleen leesbaar voor de eigen gebruiker).
- Mailen met bijlage: Simple MAPI (het eigen mailprogramma), anders map openen en mailto.
- Bouwen: GitHub Actions op Windows (openbare repository: geen kosten voor bouwminuten).
  De workflow bouwt, doet een proef met echte zorgkantoorformulieren en maakt schermafbeeldingen.
- Verspreiden: een installatieprogramma (Inno Setup, Nederlands en Engels) op GitHub (Releases).
  Later misschien winget. Niet ondertekenen (Jaap, 6 nov 2026): de SmartScreen-waarschuwing blijft
  ("Meer info", "Toch uitvoeren").
  Microsoft Store: voorlopig niet (Jaap, 6 nov 2026).
- Installeren in Program Files (met beheerdersrechten, één UAC-vraag). Een eigen verwijderprogramma
  (Startmenu en Windows Instellingen, Apps); bij verwijderen de vraag of je eigen gegevens ook weg moeten.
- Geen zipversie: alleen een installatieprogramma (Jaap, 6 nov 2026).
- Updates via Jaaps Dropbox-map `Paper Pilot updates` (in `/Updates van mijn programma's`): daarin zet Jaap
  alleen het installatieprogramma. Paper Pilot haalt de map op (Dropbox geeft een map als zip, `dl=1`),
  leest het versienummer uit het installatieprogramma zelf en kiest het nieuwste, als er meer staan. Paper Pilot kijkt of er een nieuwere versie is, haalt het
  installatieprogramma op en start het met beheerdersrechten. Het installatieprogramma ruimt eerst alle
  oude programmabestanden op ([InstallDelete]) en zet de nieuwe ervoor in de plaats; eigen gegevens blijven.
  Daarna start Paper Pilot opnieuw.
- Welkomstscherm met rondleiding: alleen bij de allereerste start (nieuwe gebruikers). Later terug te
  vinden in het menu Help. Na een update: één keer "Wat is er nieuw", met de wijzigingen van de nieuwe
  versie(s) sinds de vorige. Het menu Help heeft altijd het onderdeel Wijzigingen.
- Altijd in het programma: handleiding (F1), privacyverklaring (ook in het installatieprogramma),
  wijzigingen, "Mail de maker" (mail aan Jaap, met de versie erbij) en "Meer programma's van Jaap"
  (korte naam en omschrijving van Jaaps andere programma's, met een knop om Jaap ernaar te mailen).
  Alles in het Nederlands en Engels. De privacyverklaring schrijft Claude in eenvoudige taal; Jaap leest hem na.

## De stappen

Elke stap eindigt met een nieuwe versie voor Jaap (0.0.0.1, 0.0.0.2 enz.). Pas als Jaap zegt dat het werkt, gaan we verder.

1. **De basis.** Venster met menubalk, startscherm met de twee delen, mooie kleuren (licht, donker,
   hoog contrast), meldingen voor de schermlezer, Nederlands en Engels (menu Taal), handleiding (F1),
   privacyverklaring, Mail de maker, logboek, instellingen. Installatieprogramma (Program Files),
   verwijderprogramma en updates via Dropbox. GitHub bouwt alles, doet een proefstart en maakt
   schermafbeeldingen. Zo is elke versie meteen klaar om te verspreiden.
2. **PDF openen en lezen.** Een PDF met tekst wordt nette tekst met koppen en pagina's, in een
   leesvak waar je met de schermlezer doorheen gaat. Ook Word (.docx) en gewone tekst. Zoeken in de tekst.
3. **Tekstherkenning en foto's (Jaap, 6 nov 2026).** Gescande PDF's en fotobestanden worden tekst, zonder internet.
   - Gedrukte tekst: de tekstherkenning van Windows (`Windows.Media.Ocr`).
   - Paper Pilot zegt het als een scan scheef ligt (hoek van de tekst) of op zijn kop (herkenning op 0 en 180 graden
     vergelijken), en draait hem zelf recht.
   - Alle gangbare fotobestanden lezen en schrijven via WIC van Windows: JPG, PNG, BMP, GIF, TIFF (ook met meer
     pagina's), HEIC en WebP (als de extensie van Windows er is), JPEG XR.
   - Handschrift (boodschappenlijstje, ansichtkaart) en beschrijven wat er op een tekening of foto staat: een
     beeldtaalmodel op de eigen computer (zoals de assistent van Computerhulp Maatje: llama.cpp + een open model met
     beeld), pas downloaden als de gebruiker dat kiest. Netjes handschrift gaat goed, slordig handschrift niet altijd;
     Paper Pilot zegt eerlijk als het twijfelt.
4. **Scannen en afdrukken.** Paper Pilot zoekt zelf scanners: aangesloten (USB), netwerkscanners die Windows kent
   (WIA, ook de scanner in een printer), en netwerkprinters met scanner zonder stuurprogramma (eSCL/AirScan).
   Rechtstreeks scannen, ook meerdere pagina's; bewaren als PDF met tekst erin of als foto. Afdrukken op elke
   printer die Windows kent (gewoon afdrukvenster van Windows).
5. **Formulieren invullen (invulbare PDF).** Paper Pilot vindt alle velden en vraagt ze één voor één:
   "Wat is je naam?". Vakjes aanvinken, keuzes maken. Overzicht van alles wat je hebt ingevuld,
   daarna opslaan als nieuwe PDF.
6. **Mijn gegevens.** Naam, adres, geboortedatum, BSN, telefoon, e-mail, IBAN en dergelijke één keer
   invullen (versleuteld). Paper Pilot stelt ze voor bij het juiste veld.
7. **Formulieren zonder invulvelden.** Platte PDF's en gescande formulieren: Paper Pilot zoekt de
   lijntjes, hokjes en vragen, en zet je antwoorden op de goede plek.
8. **Handtekening.** Eén keer een handtekening scannen of fotograferen; Paper Pilot zet hem op de
   plek voor de handtekening, met de datum.
9. **Versturen en afdrukken.** Mailen met de PDF als bijlage, of afdrukken.
10. **Controleren: PDF.** Voor zienden: heeft de PDF tekst, koppen, een titel en een taal? Hebben
    plaatjes een beschrijving en invulvelden een naam? Klopt de leesvolgorde? Verslag in eenvoudige taal,
    met per punt hoe je het oplost.
11. **Controleren: Word.** Koppen, beschrijvingen bij plaatjes, taal, tabellen, kleurcontrast.
12. **Controleren: website.** Adres invullen: plaatjes, knoppen, links, formulieren, koppen, taal, contrast.
13. **Verspreiden.** winget. Jaap brengt Paper Pilot zelf onder
    de aandacht op sociale media.

## Later misschien

- Een versie voor Android (Google Play-account nodig) of iPhone (Apple-account nodig).
  Het hart van het programma (Rust) kan dan mee.
- (Gedaan in 0.0.0.7: Wat moet ik doen?, herinneringen, Mijn post, handtekening, mailen, en Controleren voor PDF, Word en websites.)

## Meer programma's van Jaap (Jaaps tekst, 6 nov 2026)

In het menu Help, met een knop om Jaap ernaar te mailen. Alleen deze programma's noemen.

- **Spraak naar tekst** – heel gemakkelijk dicteren, en je dictaten blijven op je eigen computer.
  *Speech to Text – dictate with ease, and your dictations stay on your own computer.*
- **Computerhulp Maatje** – een hulp die je vragen in gewone taal beantwoordt: radio, tv-gids, mail,
  agenda, weer en nog veel meer.
  *Computer Buddy – a helper that answers your questions in plain language: radio, TV guide, mail,
  calendar, weather and much more.*
- **Jaaps Audio Editor** – geluid bewerken, internetradio luisteren en opnemen. Je bewerkt geluid net
  zo makkelijk als tekst in een tekstverwerker: tekst weghalen of ergens anders neerzetten, en het
  geluid verandert mee.
  *Jaap's Audio Editor – edit audio, listen to and record internet radio. Edit sound as easily as text in a
  word processor: remove text or move it elsewhere, and the sound follows.*
- **Taxi Publiek Vervoer** – voor wie reist met Publiek Vervoer: een taxi boeken, wijzigen en in je
  agenda zetten.
  *Taxi Publiek Vervoer – for people who travel with Publiek Vervoer: book or change a taxi ride and add it
  to your calendar.*
