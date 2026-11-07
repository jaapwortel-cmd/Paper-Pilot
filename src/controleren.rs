// Paper Pilot - Controleren: voor ziende mensen die een PDF, Word-bestand of website maken.
//
// Paper Pilot kijkt of een schermlezer alles goed kan lezen, en zegt in gewone taal wat er mis is en hoe je het oplost.
// Het verslag komt in het leesvenster: met Ctrl+K spring je van punt naar punt, met Ctrl+S sla je het op.
// De controles zelf staan in controle_pdf.rs, controle_word.rs en controle_web.rs.

use crate::dialogen;
use crate::teksten::{t, tf};
use std::path::{Path, PathBuf};
use windows_sys::Win32::Foundation::HWND;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ernst {
    /// Een schermlezer kan iets niet goed lezen.
    Probleem,
    /// Kan beter; soms een probleem.
    LetOp,
    /// Dit is goed.
    Goed,
}

#[derive(Clone, Debug)]
pub struct Punt {
    pub ernst: Ernst,
    pub kop: String,
    pub uitleg: String,
    pub oplossing: String,
    /// Voorbeelden van waar het is ("pagina 3", "plaatje logo.png").
    pub waar: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct Verslag {
    pub naam: String,
    pub punten: Vec<Punt>,
    /// Extra zinnen bovenaan (bijvoorbeeld: "Paper Pilot ziet alleen de HTML").
    pub opmerkingen: Vec<String>,
}

impl Verslag {
    pub fn nieuw(naam: &str) -> Verslag {
        Verslag { naam: naam.to_string(), ..Default::default() }
    }

    /// Een punt uit de teksten: "Kop || Uitleg || Oplossing" (bij Goed alleen de kop).
    pub fn punt(&mut self, ernst: Ernst, sleutel: &str, waarden: &[&str], waar: Vec<String>) {
        let tekst = tf(sleutel, waarden);
        let mut delen = tekst.split("||").map(|d| d.trim().to_string());
        let kop = delen.next().unwrap_or_default();
        let uitleg = delen.next().unwrap_or_default();
        let oplossing = delen.next().unwrap_or_default();
        self.punten.push(Punt { ernst, kop, uitleg, oplossing, waar });
    }

    pub fn goed(&mut self, sleutel: &str, waarden: &[&str]) {
        self.punt(Ernst::Goed, sleutel, waarden, Vec::new());
    }

    pub fn aantal(&self, e: Ernst) -> usize {
        self.punten.iter().filter(|p| p.ernst == e).count()
    }

    /// Het verslag als document voor het leesvenster (met koppen om naartoe te springen).
    pub fn als_document(&self) -> crate::lezen::Document {
        let mut b = crate::lezen::Bouwer::nieuw(&tf("ControleVerslagNaam", &[&self.naam]));
        b.kop(&tf("ControleVerslagKop", &[&self.naam]));
        let (p, l, g) = (self.aantal(Ernst::Probleem), self.aantal(Ernst::LetOp), self.aantal(Ernst::Goed));
        let tel = |n: usize, een: &str, meer: &str| if n == 1 { t(een) } else { tf(meer, &[&n.to_string()]) };
        b.alinea(&tf("ControleSamenvatting", &[&tel(p, "ControleEenProbleem", "ControleProblemen"),
            &tel(l, "ControleEenLetOp", "ControleLetOpMeer"), &tel(g, "ControleEenGoed", "ControleGoedMeer")]));
        if p == 0 && l == 0 {
            b.alinea(&t("ControleAllesGoed"));
        }
        for o in &self.opmerkingen {
            b.alinea(o);
        }
        for (ernst, kop) in [(Ernst::Probleem, "ControleKopProblemen"), (Ernst::LetOp, "ControleKopLetOp")] {
            let punten: Vec<&Punt> = self.punten.iter().filter(|x| x.ernst == ernst).collect();
            if punten.is_empty() {
                continue;
            }
            b.kop(&tf(kop, &[&punten.len().to_string()]));
            for (i, x) in punten.iter().enumerate() {
                let soort = if ernst == Ernst::Probleem { t("ControleProbleem") } else { t("ControleLetOp") };
                b.kop(&format!("{soort} {}: {}", i + 1, x.kop));
                if !x.uitleg.is_empty() {
                    b.alinea(&x.uitleg);
                }
                if !x.waar.is_empty() {
                    let mut lijst: Vec<String> = x.waar.iter().take(12).cloned().collect();
                    if x.waar.len() > 12 {
                        lijst.push(tf("ControleEnMeer", &[&(x.waar.len() - 12).to_string()]));
                    }
                    b.alinea(&tf("ControleWaar", &[&lijst.join("; ")]));
                }
                if !x.oplossing.is_empty() {
                    b.alinea(&tf("ControleOplossing", &[&x.oplossing]));
                }
            }
        }
        let goed: Vec<&Punt> = self.punten.iter().filter(|x| x.ernst == Ernst::Goed).collect();
        if !goed.is_empty() {
            b.kop(&tf("ControleKopGoed", &[&goed.len().to_string()]));
            for x in goed {
                b.alinea(&x.kop);
            }
        }
        b.kop(&t("ControleKopTot"));
        b.alinea(&t("ControleNietAlles"));
        b.klaar()
    }
}

// ------------------------------------------------------------------ kleurcontrast (WCAG)

/// De relatieve helderheid van een kleur (0 = zwart, 1 = wit).
pub fn helderheid(r: u8, g: u8, b: u8) -> f64 {
    let kanaal = |c: u8| {
        let c = c as f64 / 255.0;
        if c <= 0.03928 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * kanaal(r) + 0.7152 * kanaal(g) + 0.0722 * kanaal(b)
}

/// Contrast tussen twee kleuren: 1 (geen) tot 21 (zwart op wit). Gewone tekst moet minstens 4,5 hebben.
pub fn contrast(a: (u8, u8, u8), b: (u8, u8, u8)) -> f64 {
    let (la, lb) = (helderheid(a.0, a.1, a.2), helderheid(b.0, b.1, b.2));
    let (licht, donker) = if la > lb { (la, lb) } else { (lb, la) };
    (licht + 0.05) / (donker + 0.05)
}

/// "#1a2b3c", "1a2b3c" of "#abc" naar een kleur.
pub fn hex_kleur(s: &str) -> Option<(u8, u8, u8)> {
    let h = s.trim().trim_start_matches('#');
    let h: String = if h.len() == 3 { h.chars().flat_map(|c| [c, c]).collect() } else { h.to_string() };
    if h.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(&h, 16).ok()?;
    Some(((v >> 16) as u8, (v >> 8) as u8, v as u8))
}

pub fn getal(x: f64) -> String {
    format!("{x:.1}").replace('.', if crate::teksten::taal() == "en" { "." } else { "," })
}

// ------------------------------------------------------------------ het venster

/// De tegel Controleren: kies wat je wilt controleren.
pub fn start(eigenaar: HWND) {
    let titel = t("ControleTitel");
    let kop = t("ControleKeuzeKop");
    let tekst = t("ControleKeuzeTekst");
    let mut v = dialogen::Vraag::nieuw(&titel, &kop, &tekst);
    v.opdrachtlinks = true;
    v.knoppen.push(dialogen::Knop { id: 5001, tekst: t("ControleKeuzePdf") });
    v.knoppen.push(dialogen::Knop { id: 5002, tekst: t("ControleKeuzeWord") });
    v.knoppen.push(dialogen::Knop { id: 5003, tekst: t("ControleKeuzeWeb") });
    v.gewone_knoppen = windows_sys::Win32::UI::Controls::TDCBF_CANCEL_BUTTON;
    v.standaard = 5001;
    match v.toon(eigenaar) {
        5001 => {
            if let Some(pad) = kies_bestand(eigenaar, "PDF\0*.pdf\0") {
                controleer_bestand(eigenaar, pad);
            }
        }
        5002 => {
            if let Some(pad) = kies_bestand(eigenaar, "Word (.docx)\0*.docx;*.docm;*.doc\0") {
                controleer_bestand(eigenaar, pad);
            }
        }
        5003 => controleer_website(eigenaar),
        _ => {}
    }
}

fn kies_bestand(eigenaar: HWND, filter: &str) -> Option<PathBuf> {
    use windows_sys::Win32::UI::Controls::Dialogs::*;
    let filter: Vec<u16> = format!("{filter}{}\0*.*\0\0", t("FilterAlles")).encode_utf16().collect();
    let mut buf = vec![0u16; 32768];
    let titel = crate::win::w(&t("ControleKiesBestand"));
    let map = crate::win::w(&crate::instellingen::get("controle_map").unwrap_or_default());
    let mut ofn: OPENFILENAMEW = unsafe { std::mem::zeroed() };
    ofn.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    ofn.hwndOwner = eigenaar;
    ofn.lpstrFilter = filter.as_ptr();
    ofn.lpstrFile = buf.as_mut_ptr();
    ofn.nMaxFile = buf.len() as u32;
    ofn.lpstrTitle = titel.as_ptr();
    ofn.lpstrInitialDir = map.as_ptr();
    ofn.Flags = OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_EXPLORER;
    if unsafe { GetOpenFileNameW(&mut ofn) } == 0 {
        return None;
    }
    let n = buf.iter().position(|&c| c == 0).unwrap_or(0);
    let pad = PathBuf::from(String::from_utf16_lossy(&buf[..n]));
    if let Some(m) = pad.parent() {
        crate::instellingen::zet("controle_map", &m.to_string_lossy());
    }
    Some(pad)
}

/// Controleert een bestand (PDF of Word) op de achtergrond en toont het verslag.
pub fn controleer_bestand(eigenaar: HWND, pad: PathBuf) {
    let p = pad.clone();
    let uitkomst = dialogen::werk(eigenaar, &t("ControleTitel"), &t("ControleBezig"), move |_| {
        crate::beeld::com_start();
        controleer(&p)
    });
    toon(eigenaar, uitkomst);
}

/// Een bestand controleren (ook voor de proef).
pub fn controleer(pad: &Path) -> Result<Verslag, String> {
    let ext = pad.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "pdf" => crate::controle_pdf::controleer(pad),
        "docx" | "docm" | "dotx" => crate::controle_word::controleer(pad),
        "doc" => Err(t("ControleOudWord")),
        _ => Err(t("ControleSoortOnbekend")),
    }
}

fn controleer_website(eigenaar: HWND) {
    let vorige = crate::instellingen::get("controle_website").unwrap_or_else(|| "https://".into());
    let Some(adres) = crate::keuzevensters::vraag_tekst(eigenaar, &t("ControleTitel"), &t("ControleWebAdres"), &vorige) else { return };
    let adres = adres.trim().to_string();
    if adres.is_empty() || adres == "https://" {
        return;
    }
    let adres = if adres.contains("://") { adres } else { format!("https://{adres}") };
    crate::instellingen::zet("controle_website", &adres);
    let a = adres.clone();
    let uitkomst = dialogen::werk(eigenaar, &t("ControleTitel"), &tf("ControleWebBezig", &[&adres]), move |_| crate::controle_web::controleer(&a));
    toon(eigenaar, uitkomst);
}

fn toon(eigenaar: HWND, uitkomst: Option<Result<Verslag, String>>) {
    match uitkomst {
        Some(Ok(v)) => {
            crate::logboek::schrijf(&format!("Gecontroleerd: {} ({} problemen, {} let op).", v.naam, v.aantal(Ernst::Probleem), v.aantal(Ernst::LetOp)));
            crate::leesvenster::open_verslag(eigenaar, v.als_document());
        }
        Some(Err(e)) => dialogen::melding(eigenaar, &t("ControleMislukt"), &e, dialogen::ICOON_WAARSCHUWING),
        None => {}
    }
}
