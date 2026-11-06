# Paper Pilot

**Brieven lezen en formulieren invullen, helemaal zelf.**
*Read letters and fill in forms, all by yourself.*

Paper Pilot is een gratis programma voor Windows voor blinden en slechtzienden. Scan of open een brief en
lees hem met je schermlezer. Vul formulieren in, vraag voor vraag, onderteken ze en verstuur ze.
Ziende mensen kunnen met Paper Pilot controleren of hun PDF, Word-bestand of website goed te lezen is met een schermlezer.

Paper Pilot is a free Windows program for blind and visually impaired people. Scan or open a letter and read
it with your screen reader. Fill in forms question by question, sign them and send them.
Sighted people can use Paper Pilot to check whether their PDF, Word document or website works well with a screen reader.

Werkt met / works with NVDA, JAWS en Narrator. Gemaakt door / made by Jaap Wortel.

Dit programma is nog in ontwikkeling. Zie [PLAN.md](PLAN.md). Licentie / license: GPL-3.0.

## Downloaden / Download

Het nieuwste installatieprogramma staat op de tak [`exe-builds`](../../tree/exe-builds).
The latest setup program is on the [`exe-builds`](../../tree/exe-builds) branch.

## Bouwen / Building

Paper Pilot is geschreven in Rust. `cargo build --release` maakt `PaperPilot.exe` (Windows 10 en 11, 64-bit).
Het installatieprogramma wordt gemaakt met Inno Setup 6 (`installer/PaperPilot.iss`). GitHub Actions bouwt en test alles
(`.github/workflows/build.yml`).

## Ondertekening / Code signing policy

Free code signing provided by SignPath.io, certificate by SignPath Foundation.
(Wordt aangevraagd na de eerste versie. / Will be applied for after the first release.)
