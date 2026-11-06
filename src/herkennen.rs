// Paper Pilot - tekstherkenning (OCR) met de ingebouwde tekstherkenning van Windows, zonder internet.
//
// Paper Pilot zoekt zelf uit hoe de scan ligt:
// - rechtop, op zijn kop, of op zijn kant: de herkenning wordt (zo nodig) in vier standen geprobeerd en de
//   stand met de meeste echte woorden wint;
// - scheef: Windows geeft de hoek van de tekst; boven 1,5 graad draait Paper Pilot de scan recht en probeert opnieuw.
// Wat er gebeurde staat in `stand`, zodat Paper Pilot het de gebruiker kan vertellen.

use crate::beeld::Beeld;
use windows::Globalization::Language;
use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
use windows::Media::Ocr::OcrEngine;
use windows::Storage::Streams::DataWriter;

#[derive(Clone)]
pub struct Regel {
    pub tekst: String,
    pub boven: f32,
    pub hoogte: f32,
}

pub struct Uitkomst {
    pub regels: Vec<Regel>,
    /// Kwartslagen die Paper Pilot heeft gedraaid (0 = lag goed, 2 = lag op zijn kop, 1 of 3 = op zijn kant).
    pub kwartslagen: u32,
    /// Hoek in graden waarover de scan scheef lag (0 = recht).
    pub scheef: f64,
    /// De rechtgezette afbeelding.
    pub beeld: Beeld,
}

fn engine() -> Result<OcrEngine, String> {
    crate::beeld::com_start();
    let voorkeur = if crate::teksten::taal() == "en" { ["en-US", "en", "nl-NL", "nl"] } else { ["nl-NL", "nl", "en-US", "en"] };
    for code in voorkeur {
        if let Ok(taal) = Language::CreateLanguage(&code.into()) {
            if OcrEngine::IsLanguageSupported(&taal).unwrap_or(false) {
                if let Ok(e) = OcrEngine::TryCreateFromLanguage(&taal) {
                    return Ok(e);
                }
            }
        }
    }
    OcrEngine::TryCreateFromUserProfileLanguages().map_err(|_| crate::teksten::t("OcrGeenTaal"))
}

/// Eén keer herkennen, zonder te draaien.
fn een_keer(e: &OcrEngine, beeld: &Beeld) -> Result<(Vec<Regel>, f64), String> {
    let max = OcrEngine::MaxImageDimension().unwrap_or(4000).max(1000);
    let klein = beeld.verklein(max);
    let schaal = beeld.breed as f32 / klein.breed as f32;
    let fout = |e: windows::core::Error| format!("OCR: {e}");
    let schrijver = DataWriter::new().map_err(fout)?;
    schrijver.WriteBytes(&klein.punten).map_err(fout)?;
    let buffer = schrijver.DetachBuffer().map_err(fout)?;
    let bitmap = SoftwareBitmap::CreateCopyFromBuffer(&buffer, BitmapPixelFormat::Bgra8, klein.breed as i32, klein.hoog as i32)
        .map_err(fout)?;
    let uitkomst = e.RecognizeAsync(&bitmap).map_err(fout)?.join().map_err(fout)?;
    let hoek = uitkomst.TextAngle().ok().and_then(|h| h.Value().ok()).unwrap_or(0.0);
    let mut regels = Vec::new();
    for regel in uitkomst.Lines().map_err(fout)? {
        let tekst = regel.Text().map(|t| t.to_string()).unwrap_or_default();
        let (mut boven, mut onder) = (f32::MAX, f32::MIN);
        if let Ok(woorden) = regel.Words() {
            for w in woorden {
                if let Ok(r) = w.BoundingRect() {
                    boven = boven.min(r.Y);
                    onder = onder.max(r.Y + r.Height);
                }
            }
        }
        if !tekst.trim().is_empty() {
            regels.push(Regel { tekst, boven: boven * schaal, hoogte: (onder - boven).max(1.0) * schaal });
        }
    }
    Ok((regels, hoek))
}

/// Veel voorkomende korte woorden (Nederlands en Engels). Tekst die op zijn kop wordt gelezen, bevat ze bijna nooit.
const GEWOON: [&str; 60] = [
    "de", "het", "een", "en", "van", "in", "is", "op", "te", "dat", "die", "voor", "met", "u", "uw", "wij", "we", "je",
    "niet", "aan", "er", "zijn", "of", "bij", "naar", "om", "ook", "als", "maar", "dan", "kunt", "wordt", "deze", "dit",
    "the", "and", "to", "a", "you", "your", "for", "on", "with", "that", "this", "are", "be", "at", "by", "from", "it",
    "datum", "naam", "adres", "geachte", "beste", "groet", "brief", "dear", "date",
];

/// Hoe goed lijkt deze tekst op echte tekst? Gewone woorden tellen zwaar, nette woorden een beetje,
/// woorden met letters en cijfers of rare tekens door elkaar tellen niet.
pub fn goede_woorden(regels: &[Regel]) -> usize {
    let mut score = 0;
    for w in regels.iter().flat_map(|r| r.tekst.split_whitespace()) {
        let kern = w.trim_matches(|c: char| !c.is_alphanumeric());
        if kern.is_empty() {
            continue;
        }
        let klein = kern.to_lowercase();
        if GEWOON.contains(&klein.as_str()) {
            score += 3;
            continue;
        }
        let letters: Vec<char> = kern.chars().collect();
        let alles_letters = letters.iter().all(|c| c.is_alphabetic());
        let klinker = klein.chars().any(|c| "aeiouyàáâäèéêëìíîïòóôöùúûü".contains(c));
        let net = alles_letters && letters.len() >= 2 && klinker
            && (letters[1..].iter().all(|c| c.is_lowercase()) || letters.iter().all(|c| c.is_uppercase()));
        let getal = kern.chars().any(|c| c.is_ascii_digit()) && kern.chars().all(|c| c.is_ascii_digit() || ".,:/-".contains(c));
        if net || getal {
            score += 1;
        }
    }
    score
}

/// Herkent de tekst op een afbeelding en zet de afbeelding zo nodig recht.
pub fn herken(beeld: &Beeld) -> Result<Uitkomst, String> {
    let e = engine()?;
    // Rechtop en op zijn kop: allebei proberen, de stand met de meeste echte tekst wint.
    let mut beste: Option<(Vec<Regel>, f64, u32, Beeld, usize)> = None;
    let probeer = |k: u32, beste: &mut Option<(Vec<Regel>, f64, u32, Beeld, usize)>| -> Result<(), String> {
        let gedraaid = beeld.kwartslag(k);
        let (r, h) = een_keer(&e, &gedraaid)?;
        let s = goede_woorden(&r);
        if beste.as_ref().map(|b| s > b.4).unwrap_or(true) {
            *beste = Some((r, h, k, gedraaid, s));
        }
        Ok(())
    };
    probeer(0, &mut beste)?;
    probeer(2, &mut beste)?;
    // Weinig tekst gevonden: misschien ligt hij op zijn kant.
    if beste.as_ref().map(|b| b.4 < 12).unwrap_or(true) {
        probeer(1, &mut beste)?;
        probeer(3, &mut beste)?;
    }
    let (mut regels, hoek, kwartslagen, mut recht, _) = beste.ok_or("OCR")?;
    // Scheef: rechtzetten en opnieuw herkennen; houden als dat niet slechter is.
    let mut scheef = 0.0;
    if hoek.abs() >= 1.5 && hoek.abs() <= 45.0 {
        scheef = hoek;
        for richting in [-1.0, 1.0] {
            let gedraaid = recht.draai(richting * hoek);
            let (r, _) = een_keer(&e, &gedraaid)?;
            if goede_woorden(&r) >= goede_woorden(&regels) {
                regels = r;
                recht = gedraaid;
                break;
            }
        }
    }
    Ok(Uitkomst { regels, kwartslagen, scheef, beeld: recht })
}

/// Kan deze computer tekst herkennen? (Voor de proefstart en de foutmelding.)
pub fn beschikbaar() -> Result<String, String> {
    let e = engine()?;
    Ok(e.RecognizerLanguage().and_then(|t| t.LanguageTag()).map(|t| t.to_string()).unwrap_or_default())
}
