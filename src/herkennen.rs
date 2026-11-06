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

/// Hoeveel woorden lijken echte woorden? (Op zijn kop gelezen tekst geeft vooral rommel.)
pub fn goede_woorden(regels: &[Regel]) -> usize {
    regels
        .iter()
        .flat_map(|r| r.tekst.split_whitespace())
        .filter(|w| {
            let letters = w.chars().filter(|c| c.is_alphabetic()).count();
            let totaal = w.chars().count();
            let klinker = w.chars().any(|c| "aeiouyAEIOUYàáâäèéêëìíîïòóôöùúûüÀÁÂÄÈÉÊËÌÍÎÏÒÓÔÖÙÚÛÜ".contains(c));
            let cijfers = w.chars().filter(|c| c.is_ascii_digit()).count();
            (letters >= 2 && klinker && letters * 10 >= totaal * 7) || (cijfers >= 2 && cijfers * 10 >= totaal * 6)
        })
        .count()
}

/// Herkent de tekst op een afbeelding en zet de afbeelding zo nodig recht.
pub fn herken(beeld: &Beeld) -> Result<Uitkomst, String> {
    let e = engine()?;
    let (regels, hoek) = een_keer(&e, beeld)?;
    let mut beste = (regels, hoek, 0u32, beeld.clone());
    let mut score = goede_woorden(&beste.0);
    let totaal: usize = beste.0.iter().map(|r| r.tekst.split_whitespace().count()).sum();
    // Weinig echte woorden: de andere drie standen proberen.
    if score < 8 || score * 10 < totaal * 6 {
        for k in [2u32, 1, 3] {
            let gedraaid = beeld.kwartslag(k);
            let (r, h) = een_keer(&e, &gedraaid)?;
            let s = goede_woorden(&r);
            if s > score + 2 && s * 10 > score * 15 {
                score = s;
                beste = (r, h, k, gedraaid);
            }
        }
    }
    let (mut regels, hoek, kwartslagen, mut recht) = beste;
    // Scheef: rechtzetten en opnieuw herkennen; houden als dat beter is.
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
