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
- **Mooi voor zienden:** rustige kleuren, ronde hoeken, grote duidelijke knoppen, licht en donker,
  hoog contrast van Windows wordt gevolgd. GitHub maakt bij elke bouwronde schermafbeeldingen.
- **Snel:** het programma start in een fractie van een seconde.
- **Privé:** alles gebeurt op de eigen computer. Brieven en gegevens gaan nooit naar internet.
  Alleen het controleren van een website gebruikt internet.
- **Twee talen:** Nederlands en Engels, alle teksten in een tekstbestand. Handleiding in beide talen.
- **Gratis en openbaar,** licentie GPL-3.0.

## Techniek

- Taal: **Rust**, voor Windows 10 en 11 (64-bit). Eén .exe, geen .NET nodig.
- Vensters: de crate `windows` (Win32-onderdelen, zelf mooi getekend; menu's, lijsten, invulvakken).
- PDF: **PDFium** (crate `pdfium-render` met `pdfium.dll` van pdfium-binaries): tekst lezen, pagina's
  tekenen, invulvelden lezen en invullen, tekst en handtekening op de pagina zetten, opslaan.
- Tekstherkenning (OCR): de ingebouwde tekstherkenning van Windows (`Windows.Media.Ocr`), zonder internet.
  Valt die tegen of ontbreekt de taal, dan Tesseract als reserve.
- Scannen: WIA (Windows Image Acquisition), werkt met bijna elke scanner.
- Gegevens versleutelen: DPAPI van Windows (alleen leesbaar voor de eigen gebruiker).
- Mailen met bijlage: Simple MAPI (het eigen mailprogramma), anders map openen en mailto.
- Bouwen: GitHub Actions op Windows (openbare repository: geen kosten voor bouwminuten).
  De workflow bouwt, doet een proef met echte zorgkantoorformulieren en maakt schermafbeeldingen.
- Verspreiden: installatieprogramma (Inno Setup) en een losse zip; later Microsoft Store, winget en een
  eigen website (GitHub Pages). Ondertekenen later via SignPath (gratis voor open source).

## De stappen

Elke stap eindigt met een testversie voor Jaap. Pas als Jaap zegt dat het werkt, gaan we verder.

1. **De basis.** Venster met menubalk, startscherm met de twee delen, mooie kleuren (licht, donker,
   hoog contrast), meldingen voor de schermlezer, Nederlands en Engels, handleiding (F1), logboek,
   instellingen. GitHub bouwt de .exe, doet een proefstart en maakt schermafbeeldingen.
2. **PDF openen en lezen.** Een PDF met tekst wordt nette tekst met koppen en pagina's, in een
   leesvak waar je met de schermlezer doorheen gaat. Ook Word (.docx) en gewone tekst. Zoeken in de tekst.
3. **Tekstherkenning.** Gescande PDF's en foto's (jpg, png) worden tekst. Paper Pilot ziet zelf of
   een pagina een plaatje is.
4. **Scannen.** Rechtstreeks van de scanner, ook meerdere pagina's. Bewaren als PDF met tekst erin.
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
13. **Verspreiden.** Ondertekenen (SignPath), Microsoft Store, winget, website met download en handleiding.

## Later misschien

- Een versie voor Android (Google Play-account nodig) of iPhone (Apple-account nodig).
  Het hart van het programma (Rust) kan dan mee.
- Brieven samenvatten of uitleggen in eenvoudige taal, met een taalmodel op de eigen computer.
