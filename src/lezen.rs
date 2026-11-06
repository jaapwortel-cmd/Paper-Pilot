// Paper Pilot - documenten lezen: PDF (met PDFium), Word (.docx) en gewone tekst.
//
// Het resultaat is nette tekst voor een gewoon Windows-tekstvak: per pagina een regel "Pagina 1 van 3",
// alinea's met een lege regel ertussen, en koppen op een eigen regel. De plaats van elke kop en elke pagina
// wordt bewaard (in UTF-16-tekens, zoals het tekstvak telt), zodat je er met Ctrl+K en Ctrl+G naartoe gaat.
//
// Koppen in een PDF: regels met een duidelijk grotere letter dan de gewone tekst, of korte vette regels
// die alleen staan. Afgebroken woorden aan het eind van een regel ("contro-" + "leren") worden weer heel.

use pdfium_render::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[derive(Clone, Default)]
pub struct Document {
    pub naam: String,
    /// De tekst met \r\n als regeleinde.
    pub tekst: String,
    /// Koppen: (tekst, begin, eind) in UTF-16-tekens.
    pub koppen: Vec<(String, usize, usize)>,
    /// Begin van elke pagina in UTF-16-tekens (leeg bij Word en tekst).
    pub paginas: Vec<usize>,
    /// Paginanummers (vanaf 1) zonder tekst: waarschijnlijk een scan.
    pub zonder_tekst: Vec<usize>,
    pub invulvelden: usize,
    /// Wat Paper Pilot de gebruiker wil vertellen (scheef, op zijn kop, geen tekst gevonden).
    pub meldingen: Vec<String>,
    /// Gescande pagina's en foto's, rechtgezet (om als afbeelding op te slaan).
    pub beelden: Vec<crate::beeld::Beeld>,
}

/// Bouwt de tekst op en houdt de plaatsen bij.
pub(crate) struct Bouwer {
    doc: Document,
    lengte: usize,
}

impl Bouwer {
    fn nieuw(naam: &str) -> Self {
        Bouwer { doc: Document { naam: naam.to_string(), ..Default::default() }, lengte: 0 }
    }
    fn regel(&mut self, s: &str) -> (usize, usize) {
        let begin = self.lengte;
        self.doc.tekst.push_str(s);
        self.lengte += s.encode_utf16().count();
        let eind = self.lengte;
        self.doc.tekst.push_str("\r\n");
        self.lengte += 2;
        (begin, eind)
    }
    fn leeg(&mut self) {
        if !self.doc.tekst.is_empty() && !self.doc.tekst.ends_with("\r\n\r\n") {
            self.regel("");
        }
    }
    fn kop(&mut self, s: &str) {
        self.leeg();
        let (b, e) = self.regel(s);
        self.doc.koppen.push((s.to_string(), b, e));
        self.regel("");
    }
    fn alinea(&mut self, s: &str) {
        let s = s.trim();
        if s.is_empty() {
            return;
        }
        self.regel(s);
        self.regel("");
    }
    fn pagina(&mut self, nr: usize, totaal: usize) {
        self.leeg();
        let begin = self.lengte;
        self.doc.paginas.push(begin);
        self.regel(&crate::teksten::tf("PaginaVan", &[&nr.to_string(), &totaal.to_string()]));
        self.regel("");
    }
    fn klaar(mut self) -> Document {
        while self.doc.tekst.ends_with("\r\n\r\n") {
            self.doc.tekst.truncate(self.doc.tekst.len() - 2);
        }
        self.doc
    }
}

/// Opent een document. `voortgang` krijgt korte berichten tijdens het herkennen van scans.
pub fn open(pad: &Path, voortgang: &dyn Fn(String)) -> Result<Document, String> {
    let ext = pad.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let naam = pad.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    if crate::beeld::is_afbeelding(pad) {
        return afbeelding(pad, &naam, voortgang);
    }
    match ext.as_str() {
        "pdf" => pdf(pad, &naam, voortgang),
        "docx" => docx(pad, &naam),
        "txt" | "text" | "md" | "csv" => txt(pad, &naam),
        _ => Err(crate::teksten::t("LezenSoortOnbekend")),
    }
}

// ---------------------------------------------------------------- PDF

/// De map met onderdelen naast PaperPilot.exe (pdfium.dll).
pub fn onderdelen() -> PathBuf {
    std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("onderdelen"))).unwrap_or_default()
}

pub fn pdfium() -> Result<&'static Pdfium, String> {
    static P: OnceLock<Result<Pdfium, String>> = OnceLock::new();
    P.get_or_init(|| {
        let bib = Pdfium::pdfium_platform_library_name_at_path(&onderdelen());
        Pdfium::bind_to_library(&bib).map(Pdfium::new).map_err(|e| {
            crate::logboek::schrijf(&format!("PDFium laden lukte niet ({}): {e:?}", bib.display()));
            crate::teksten::t("LezenGeenPdfium")
        })
    })
    .as_ref()
    .map_err(|e| e.clone())
}

struct Regel {
    tekst: String,
    grootte: f32,
    vet: f32,
    boven: f32,
    onder: f32,
    rechts: f32,
}

fn pdf(pad: &Path, naam: &str, voortgang: &dyn Fn(String)) -> Result<Document, String> {
    let pdfium = pdfium()?;
    let doc = pdfium.load_pdf_from_file(pad, None).map_err(|e| match e {
        PdfiumError::PdfiumLibraryInternalError(PdfiumInternalError::PasswordError) => crate::teksten::t("LezenWachtwoord"),
        e => {
            crate::logboek::schrijf(&format!("PDF openen: {e:?}"));
            crate::teksten::t("LezenKapot")
        }
    })?;
    // Eerst alle regels van alle pagina's, dan pas weten we hoe groot de gewone letter is.
    let mut paginas: Vec<Vec<Regel>> = Vec::new();
    let mut velden = 0usize;
    for pagina in doc.pages().iter() {
        velden += pagina.annotations().iter().filter(|a| a.annotation_type() == PdfPageAnnotationType::Widget).count();
        let mut regels: Vec<Regel> = Vec::new();
        if let Ok(tekst) = pagina.text() {
            let mut huidig = String::new();
            let (mut som, mut aantal, mut vet) = (0f32, 0f32, 0f32);
            let (mut boven, mut onder, mut rechts) = (f32::MIN, f32::MAX, f32::MIN);
            let mut sluit = |huidig: &mut String, som: &mut f32, aantal: &mut f32, vet: &mut f32, boven: &mut f32, onder: &mut f32, rechts: &mut f32| {
                let t = huidig.split_whitespace().collect::<Vec<_>>().join(" ");
                if !t.is_empty() && *aantal > 0.0 {
                    regels.push(Regel { tekst: t, grootte: *som / *aantal, vet: *vet / *aantal, boven: *boven, onder: *onder, rechts: *rechts });
                }
                huidig.clear();
                *som = 0.0;
                *aantal = 0.0;
                *vet = 0.0;
                *boven = f32::MIN;
                *onder = f32::MAX;
                *rechts = f32::MIN;
            };
            for ch in tekst.chars().iter() {
                let Some(c) = ch.unicode_char() else { continue };
                if c == '\r' || c == '\n' {
                    sluit(&mut huidig, &mut som, &mut aantal, &mut vet, &mut boven, &mut onder, &mut rechts);
                    continue;
                }
                // PDFium geeft een afbreekstreepje aan het eind van een regel als teken 2 en plakt de regels
                // dan zelf aan elkaar: dan het streepje weglaten ("contro-" + "leren" = "controleren").
                if c == '\u{2}' {
                    continue;
                }
                huidig.push(c);
                if c.is_whitespace() {
                    continue;
                }
                let g = ch.scaled_font_size().value;
                som += g;
                aantal += 1.0;
                let zwaar = ch.font_weight().map(|w| matches!(w, PdfFontWeight::Weight600 | PdfFontWeight::Weight700Bold
                    | PdfFontWeight::Weight800 | PdfFontWeight::Weight900)).unwrap_or(false)
                    || ch.font_name().to_lowercase().contains("bold");
                if zwaar {
                    vet += 1.0;
                }
                if let Ok(r) = ch.loose_bounds() {
                    boven = boven.max(r.top().value);
                    onder = onder.min(r.bottom().value);
                    rechts = rechts.max(r.right().value);
                }
            }
            sluit(&mut huidig, &mut som, &mut aantal, &mut vet, &mut boven, &mut onder, &mut rechts);
        }
        paginas.push(regels);
    }
    // De gewone lettergrootte: die met de meeste tekens.
    let mut tellingen: Vec<(f32, usize)> = Vec::new();
    for r in paginas.iter().flatten() {
        let g = (r.grootte * 2.0).round() / 2.0;
        match tellingen.iter_mut().find(|(k, _)| *k == g) {
            Some(t) => t.1 += r.tekst.len(),
            None => tellingen.push((g, r.tekst.len())),
        }
    }
    let gewoon = tellingen.iter().max_by_key(|(_, n)| *n).map(|(g, _)| *g).unwrap_or(11.0);

    let mut b = Bouwer::nieuw(naam);
    let totaal = paginas.len();
    for (i, regels) in paginas.iter().enumerate() {
        b.pagina(i + 1, totaal);
        if regels.is_empty() {
            // Een gescande pagina: als afbeelding maken (ongeveer 300 dpi) en de tekst herkennen.
            voortgang(crate::teksten::tf("OcrBezig", &[&(i + 1).to_string(), &totaal.to_string()]));
            let beeld = doc.pages().get(i as i32).ok().and_then(|p| {
                let breed = ((p.width().value / 72.0 * 300.0) as i32).clamp(800, 3600);
                p.render_with_config(&PdfRenderConfig::new().set_target_width(breed).render_form_data(true)).ok().map(|bm| {
                    crate::beeld::Beeld { breed: bm.width() as u32, hoog: bm.height() as u32, punten: bm.as_raw_bytes() }
                })
            });
            match beeld {
                Some(beeld) => scan_pagina(&mut b, &beeld, i + 1),
                None => {
                    b.doc.zonder_tekst.push(i + 1);
                    b.alinea(&crate::teksten::t("LezenPaginaZonderTekst"));
                }
            }
            continue;
        }
        let mut alinea = String::new();
        let mut vorige: Option<&Regel> = None;
        // Een regel die ver voor de rechterkant ophoudt (adres, opsomming) blijft een eigen regel.
        let max_rechts = regels.iter().map(|r| r.rechts).fold(f32::MIN, f32::max);
        for (j, r) in regels.iter().enumerate() {
            let hoog = (r.boven - r.onder).max(1.0);
            let groot = r.grootte >= gewoon * 1.18 && r.tekst.chars().count() <= 150;
            let ruimte_voor = vorige.map(|v| v.onder - r.boven > hoog * 0.6).unwrap_or(true);
            let ruimte_na = regels.get(j + 1).map(|n| r.onder - n.boven > hoog * 0.6).unwrap_or(true);
            let vet_alleen = r.vet > 0.8 && r.tekst.chars().count() <= 80 && ruimte_voor && ruimte_na
                && !r.tekst.ends_with(':') && regels.len() > 1;
            if groot || vet_alleen {
                b.alinea(&alinea);
                alinea.clear();
                b.kop(&r.tekst);
                vorige = Some(r);
                continue;
            }
            if ruimte_voor && !alinea.is_empty() {
                b.alinea(&alinea);
                alinea.clear();
            }
            if alinea.is_empty() {
                alinea.push_str(&r.tekst);
            } else if alinea.ends_with('-') && r.tekst.chars().next().map(|c| c.is_lowercase()).unwrap_or(false)
                && alinea[..alinea.len() - 1].chars().last().map(|c| c.is_alphabetic()).unwrap_or(false)
            {
                // Afgebroken woord weer heel maken.
                alinea.pop();
                alinea.push_str(&r.tekst);
            } else if vorige.map(|v| v.rechts < max_rechts - 90.0).unwrap_or(false) {
                alinea.push_str("\r\n");
                alinea.push_str(&r.tekst);
            } else {
                alinea.push(' ');
                alinea.push_str(&r.tekst);
            }
            vorige = Some(r);
        }
        b.alinea(&alinea);
    }
    let mut d = b.klaar();
    d.invulvelden = velden;
    Ok(d)
}

// ---------------------------------------------------------------- Scans en foto's

/// Herkent de tekst op een gescande pagina of foto en zet die in de tekst, met wat er met de stand was.
fn scan_pagina(b: &mut Bouwer, beeld: &crate::beeld::Beeld, nr: usize) {
    use crate::teksten::{t, tf};
    let u = match crate::herkennen::herken(beeld) {
        Ok(u) => u,
        Err(e) => {
            crate::logboek::schrijf(&format!("Tekstherkenning: {e}"));
            b.doc.zonder_tekst.push(nr);
            b.alinea(&e);
            b.doc.beelden.push(beeld.clone());
            return;
        }
    };
    let mut stand = Vec::new();
    match u.kwartslagen {
        2 => stand.push(t("StandOpKop")),
        1 | 3 => stand.push(t("StandOpKant")),
        _ => {}
    }
    if u.scheef.abs() >= 1.5 {
        stand.push(tf("StandScheef", &[&format!("{:.0}", u.scheef.abs())]));
    }
    if !stand.is_empty() {
        let zin = tf("StandMelding", &[&nr.to_string(), &stand.join(" en ")]);
        b.alinea(&zin);
        b.doc.meldingen.push(zin);
    }
    if u.regels.is_empty() {
        b.doc.zonder_tekst.push(nr);
        b.alinea(&t("OcrNiets"));
    } else {
        ocr_alineas(b, &u.regels);
    }
    b.doc.beelden.push(u.beeld);
}

/// Herkende regels netjes in alinea's en koppen zetten.
fn ocr_alineas(b: &mut Bouwer, regels: &[crate::herkennen::Regel]) {
    let mut hoogtes: Vec<f32> = regels.iter().map(|r| r.hoogte).collect();
    hoogtes.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let gewoon = hoogtes[hoogtes.len() / 2].max(1.0);
    let mut alinea = String::new();
    let mut vorige: Option<&crate::herkennen::Regel> = None;
    for r in regels {
        let tekst = r.tekst.trim();
        let kop = r.hoogte >= gewoon * 1.35 && tekst.split_whitespace().count() <= 12;
        let gat = vorige.map(|v| r.boven - (v.boven + v.hoogte) > gewoon * 0.9 || r.boven < v.boven).unwrap_or(true);
        if kop {
            b.alinea(&alinea);
            alinea.clear();
            b.kop(tekst);
        } else {
            if gat && !alinea.is_empty() {
                b.alinea(&alinea);
                alinea.clear();
            }
            if alinea.is_empty() {
                alinea.push_str(tekst);
            } else if alinea.ends_with('-') && tekst.chars().next().map(|c| c.is_lowercase()).unwrap_or(false) {
                alinea.pop();
                alinea.push_str(tekst);
            } else {
                // Gescande brieven: elke regel blijft een regel (adressen, lijstjes), binnen dezelfde alinea.
                alinea.push_str("\r\n");
                alinea.push_str(tekst);
            }
        }
        vorige = Some(r);
    }
    b.alinea(&alinea);
}

fn afbeelding(pad: &Path, naam: &str, voortgang: &dyn Fn(String)) -> Result<Document, String> {
    let beelden = crate::beeld::lees(pad)?;
    let mut b = Bouwer::nieuw(naam);
    let totaal = beelden.len();
    for (i, beeld) in beelden.iter().enumerate() {
        voortgang(crate::teksten::tf("OcrBezig", &[&(i + 1).to_string(), &totaal.to_string()]));
        if totaal > 1 {
            b.pagina(i + 1, totaal);
        }
        scan_pagina(&mut b, beeld, i + 1);
    }
    Ok(b.klaar())
}

// ---------------------------------------------------------------- Word

fn docx(pad: &Path, naam: &str) -> Result<Document, String> {
    use quick_xml::events::Event;
    let bestand = std::fs::File::open(pad).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(bestand).map_err(|_| crate::teksten::t("LezenKapot"))?;
    let mut xml = String::new();
    {
        use std::io::Read;
        let mut deel = zip.by_name("word/document.xml").map_err(|_| crate::teksten::t("LezenKapot"))?;
        deel.read_to_string(&mut xml).map_err(|e| e.to_string())?;
    }
    let mut lezer = quick_xml::Reader::from_str(&xml);
    let mut b = Bouwer::nieuw(naam);
    let mut alinea = String::new();
    let mut is_kop = false;
    let mut in_tekst = false;
    let mut cel: Vec<String> = Vec::new();
    let mut tabel_diepte = 0;
    let lokaal = |n: &[u8]| -> String {
        let s = String::from_utf8_lossy(n).to_string();
        s.rsplit(':').next().unwrap_or("").to_string()
    };
    loop {
        match lezer.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let naam = lokaal(e.name().as_ref());
                match naam.as_str() {
                    "p" => {
                        alinea.clear();
                        is_kop = false;
                    }
                    "t" => in_tekst = true,
                    "tab" => alinea.push('\t'),
                    "br" | "cr" => alinea.push(' '),
                    "tbl" => tabel_diepte += 1,
                    "tr" => cel.clear(),
                    "pStyle" | "outlineLvl" => {
                        for a in e.attributes().flatten() {
                            if lokaal(a.key.as_ref()) == "val" {
                                let v = String::from_utf8_lossy(&a.value).to_lowercase();
                                if naam == "outlineLvl" || v.starts_with("heading") || v.starts_with("kop") || v == "title" || v == "titel" {
                                    is_kop = true;
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(t)) if in_tekst => {
                alinea.push_str(&t.unescape().map(|s| s.to_string()).unwrap_or_default());
            }
            Ok(Event::End(e)) => {
                let naam = lokaal(e.name().as_ref());
                match naam.as_str() {
                    "t" => in_tekst = false,
                    "p" => {
                        let t = alinea.trim().to_string();
                        if tabel_diepte > 0 {
                            if !t.is_empty() {
                                cel.push(t);
                            }
                        } else if is_kop && !t.is_empty() {
                            b.kop(&t);
                        } else {
                            b.alinea(&t);
                        }
                    }
                    "tr" => {
                        if !cel.is_empty() {
                            b.alinea(&cel.join(" | "));
                        }
                        cel.clear();
                    }
                    "tbl" => tabel_diepte -= 1,
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => return Err(crate::teksten::t("LezenKapot")),
            _ => {}
        }
    }
    Ok(b.klaar())
}

// ---------------------------------------------------------------- Tekst

/// Leest tekst als UTF-8; lukt dat niet, dan als Windows-1252 (oude Windows-tekstbestanden).
pub fn lees_tekstbestand(pad: &Path) -> Result<String, String> {
    let bytes = std::fs::read(pad).map_err(|e| e.to_string())?;
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF][..]).unwrap_or(&bytes);
    Ok(match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => bytes.iter().map(|&c| match c {
            0x80 => '€', 0x91 | 0x92 => '\'', 0x93 | 0x94 => '"', 0x96 | 0x97 => '-', 0x85 => '…',
            c => c as char,
        }).collect(),
    })
}

fn txt(pad: &Path, naam: &str) -> Result<Document, String> {
    let tekst = lees_tekstbestand(pad)?;
    let mut b = Bouwer::nieuw(naam);
    for regel in tekst.lines() {
        b.regel(regel.trim_end());
    }
    Ok(b.klaar())
}
