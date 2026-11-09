// Paper Pilot - gezichten op een foto vinden, met de gezichtsherkenning van Windows (Windows.Media.FaceAnalysis).
//
// Alles gebeurt op de eigen computer. Paper Pilot zoekt alleen WAAR een gezicht staat, niet WIE het is.
// Daarmee zegt Paper Pilot of een gezicht helemaal of maar half in beeld is, waar het staat en of het ver weg
// of heel dichtbij is. Hoe iemand eruitziet, vertelt het taalmodel (beeldmodel.rs, Soort::Foto).
//
// Links en rechts: bij de camera aan de voorkant (boven het scherm) kijkt de camera naar jou. Wat op de foto links
// staat, is dan aan jouw rechterkant. Bij een camera aan de achterkant (tablet) is links op de foto ook links.

use crate::beeld::Beeld;
use crate::teksten::{t, tf};

/// Een gevonden gezicht, in beeldpunten van de foto.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gezicht {
    pub x: u32,
    pub y: u32,
    pub breed: u32,
    pub hoog: u32,
}

/// Een kant van de foto.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rand {
    Boven,
    Onder,
    /// Links op de foto.
    Links,
    /// Rechts op de foto.
    Rechts,
}

/// Zoekt gezichten op een foto. Geeft een fout als deze computer geen gezichtsherkenning heeft.
pub fn zoek(beeld: &Beeld) -> Result<Vec<Gezicht>, String> {
    use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
    use windows::Media::FaceAnalysis::FaceDetector;
    use windows::Storage::Streams::DataWriter;
    let fout = |e: windows::core::Error| format!("{}", e.message());
    if !FaceDetector::IsSupported().unwrap_or(false) {
        return Err(t("GezichtNietMogelijk"));
    }
    // Verkleind (sneller), in grijs: de gezichtsherkenning van Windows werkt met Gray8.
    let klein = beeld.verklein(1280);
    let factor = beeld.breed as f64 / klein.breed.max(1) as f64;
    let grijs: Vec<u8> = klein.punten.chunks_exact(4)
        .map(|p| ((p[2] as u32 * 299 + p[1] as u32 * 587 + p[0] as u32 * 114) / 1000) as u8)
        .collect();
    let schrijver = DataWriter::new().map_err(fout)?;
    schrijver.WriteBytes(&grijs).map_err(fout)?;
    let buffer = schrijver.DetachBuffer().map_err(fout)?;
    let bitmap = SoftwareBitmap::CreateCopyFromBuffer(&buffer, BitmapPixelFormat::Gray8, klein.breed as i32, klein.hoog as i32).map_err(fout)?;
    let zoeker = FaceDetector::CreateAsync().map_err(fout)?.join().map_err(fout)?;
    let lijst = zoeker.DetectFacesAsync(&bitmap).map_err(fout)?.join().map_err(fout)?;
    let mut uit = Vec::new();
    for i in 0..lijst.Size().unwrap_or(0) {
        if let Ok(g) = lijst.GetAt(i) {
            if let Ok(b) = g.FaceBox() {
                uit.push(Gezicht {
                    x: (b.X as f64 * factor) as u32,
                    y: (b.Y as f64 * factor) as u32,
                    breed: (b.Width as f64 * factor) as u32,
                    hoog: (b.Height as f64 * factor) as u32,
                });
            }
        }
    }
    // Van links naar rechts op de foto.
    uit.sort_by_key(|g| g.x);
    Ok(uit)
}

/// Aan welke kanten raakt het gezicht de rand van de foto? Dan valt het daar (deels) buiten beeld.
/// De gezichtsherkenning tekent een vak om het gezicht zelf; raakt dat vak de rand, dan is er een stuk weg.
pub fn randen(g: &Gezicht, breed: u32, hoog: u32) -> Vec<Rand> {
    let marge_x = (breed as f64 * 0.01).max(2.0) as u32;
    let marge_y = (hoog as f64 * 0.01).max(2.0) as u32;
    let mut uit = Vec::new();
    if g.y <= marge_y {
        uit.push(Rand::Boven);
    }
    if g.y + g.hoog + marge_y >= hoog {
        uit.push(Rand::Onder);
    }
    if g.x <= marge_x {
        uit.push(Rand::Links);
    }
    if g.x + g.breed + marge_x >= breed {
        uit.push(Rand::Rechts);
    }
    uit
}

/// De naam van een rand, vanuit de gebruiker gezien.
fn rand_naam(r: Rand, voorkant: bool) -> String {
    match (r, voorkant) {
        (Rand::Boven, _) => t("GezichtRandBoven"),
        (Rand::Onder, _) => t("GezichtRandOnder"),
        // Camera aan de voorkant: links op de foto is jouw rechterkant.
        (Rand::Links, true) => t("GezichtJouwRechts"),
        (Rand::Rechts, true) => t("GezichtJouwLinks"),
        (Rand::Links, false) => t("GezichtFotoLinks"),
        (Rand::Rechts, false) => t("GezichtFotoRechts"),
    }
}

/// Waar het gezicht staat: links, midden of rechts, en hoog of laag.
fn plek(g: &Gezicht, breed: u32, hoog: u32, voorkant: bool) -> String {
    let mx = (g.x + g.breed / 2) as f64 / breed.max(1) as f64;
    let my = (g.y + g.hoog / 2) as f64 / hoog.max(1) as f64;
    let mut delen = Vec::new();
    delen.push(if mx < 0.35 {
        rand_naam(Rand::Links, voorkant)
    } else if mx > 0.65 {
        rand_naam(Rand::Rechts, voorkant)
    } else {
        t("GezichtMidden")
    });
    if my < 0.3 {
        delen.push(t("GezichtHoog"));
    } else if my > 0.7 {
        delen.push(t("GezichtLaag"));
    }
    delen.join(", ")
}

/// Wat Paper Pilot over de gezichten zegt, in gewone zinnen.
/// `voorkant`: de foto komt van de camera aan de voorkant (die naar jou kijkt).
pub fn uitleg(gezichten: &[Gezicht], breed: u32, hoog: u32, voorkant: bool) -> String {
    if gezichten.is_empty() {
        return t("GezichtGeen");
    }
    // Vanuit de gebruiker van links naar rechts: bij de voorkant is dat op de foto van rechts naar links.
    let mut lijst: Vec<Gezicht> = gezichten.to_vec();
    if voorkant {
        lijst.reverse();
    }
    let mut regels = Vec::new();
    regels.push(if lijst.len() == 1 { t("GezichtEen") } else { tf("GezichtMeer", &[&lijst.len().to_string()]) });
    for (i, g) in lijst.iter().enumerate() {
        let r = randen(g, breed, hoog);
        let staat = if r.is_empty() {
            tf("GezichtHelemaal", &[&plek(g, breed, hoog, voorkant)])
        } else {
            let namen: Vec<String> = r.iter().map(|x| rand_naam(*x, voorkant)).collect();
            tf("GezichtHalf", &[&namen.join(&format!(" {} ", t("GezichtEn").trim()))])
        };
        let mut zin = if lijst.len() == 1 {
            tf("GezichtZinEen", &[&staat])
        } else {
            tf("GezichtZinMeer", &[&(i + 1).to_string(), &staat])
        };
        // Grootte: een klein gezicht is ver weg, een heel groot gezicht is heel dichtbij.
        let deel = g.breed as f64 / breed.max(1) as f64;
        if deel < 0.08 {
            zin.push(' ');
            zin.push_str(&t("GezichtVer"));
        } else if deel > 0.5 {
            zin.push(' ');
            zin.push_str(&t("GezichtDichtbij"));
        }
        regels.push(zin);
    }
    regels.join("\r\n")
}

#[cfg(test)]
mod proeven {
    use super::*;

    #[test]
    fn rand_geraakt() {
        let g = Gezicht { x: 500, y: 300, breed: 200, hoog: 200 };
        assert!(randen(&g, 1280, 720).is_empty());
        let boven = Gezicht { x: 500, y: 0, breed: 200, hoog: 150 };
        assert_eq!(randen(&boven, 1280, 720), vec![Rand::Boven]);
        let rechts = Gezicht { x: 1150, y: 300, breed: 130, hoog: 200 };
        assert_eq!(randen(&rechts, 1280, 720), vec![Rand::Rechts]);
    }
}
