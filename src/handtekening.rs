// Paper Pilot - Mijn handtekening: één keer scannen of fotograferen, daarna zet Paper Pilot hem op formulieren.
//
// Van de scan of foto blijft alleen de handtekening over: Paper Pilot zoekt de inkt, snijdt de rest weg en maakt het
// papier doorzichtig. De handtekening staat versleuteld (DPAPI) in %APPDATA%\Paper Pilot\handtekening.dat.
// Bij het invullen: een vraag over de handtekening krijgt "[mijn handtekening]"; bij het opslaan komt daar het plaatje.

use crate::beeld::Beeld;
use crate::teksten::{t, tf};
use std::path::Path;

const BESTAND: &str = "handtekening.dat";

/// Is dit een vraag naar een handtekening?
pub fn is_vraag(vraag: &str) -> bool {
    let v = vraag.to_lowercase();
    ["handtekening", "ondertekening", "signature", "sign here", "paraaf"].iter().any(|w| v.contains(w))
}

/// De tekst in het invulvak die betekent: hier komt mijn handtekening.
pub fn merkteken() -> String {
    t("HandtekeningMerk")
}

pub fn is_merkteken(waarde: &str) -> bool {
    let w = waarde.trim();
    !w.is_empty() && (w == t("HandtekeningMerk") || w == crate::teksten::en().get("HandtekeningMerk").map(|s| s.as_str()).unwrap_or("")
        || w == crate::teksten::nl().get("HandtekeningMerk").map(|s| s.as_str()).unwrap_or(""))
}

pub fn heb() -> bool {
    crate::instellingen::map().join(BESTAND).exists()
}

pub fn wis() {
    let _ = std::fs::remove_file(crate::instellingen::map().join(BESTAND));
}

fn tijdelijk_png() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("paperpilot-hand-{}.png", std::process::id()))
}

pub fn laad() -> Option<Beeld> {
    let data = crate::gegevens::lees_versleuteld(BESTAND)?;
    let pad = tijdelijk_png();
    std::fs::write(&pad, data).ok()?;
    let b = crate::beeld::lees(&pad).ok().and_then(|mut v| if v.is_empty() { None } else { Some(v.remove(0)) });
    let _ = std::fs::remove_file(&pad);
    b
}

pub fn bewaar(b: &Beeld) -> Result<(), String> {
    let pad = tijdelijk_png();
    crate::beeld::schrijf(&pad, std::slice::from_ref(b))?;
    let data = std::fs::read(&pad).map_err(|e| e.to_string());
    let _ = std::fs::remove_file(&pad);
    crate::gegevens::schrijf_versleuteld(BESTAND, &data?)
}

/// Haalt de handtekening uit een scan of foto: alleen de inkt, met een doorzichtige achtergrond.
pub fn uit_beeld(bron: &Beeld) -> Option<Beeld> {
    let (b, h) = (bron.breed as usize, bron.hoog as usize);
    if b < 20 || h < 20 {
        return None;
    }
    let grijs: Vec<u8> = bron.punten.chunks_exact(4).map(|p| ((p[0] as u32 * 29 + p[1] as u32 * 150 + p[2] as u32 * 77) >> 8) as u8).collect();
    // Het papier: de lichtste helft. De inkt: duidelijk donkerder dan het papier.
    let mut gesorteerd = grijs.clone();
    gesorteerd.sort_unstable();
    let papier = gesorteerd[gesorteerd.len() * 3 / 4] as i32;
    let drempel = (papier as f32 * 0.6) as i32;
    // Een rand van 2% overslaan (schaduwen van de scanner of de rand van het papier).
    let (rx, ry) = (b / 50, h / 50);
    let mut rijen = vec![0usize; h];
    let mut kolommen = vec![0usize; b];
    for y in ry..h - ry {
        for x in rx..b - rx {
            if (grijs[y * b + x] as i32) < drempel {
                rijen[y] += 1;
                kolommen[x] += 1;
            }
        }
    }
    // Losse stipjes tellen niet: een rij of kolom telt mee bij minstens 2 inktpunten.
    let y0 = rijen.iter().position(|&n| n >= 2)?;
    let y1 = rijen.iter().rposition(|&n| n >= 2)?;
    let x0 = kolommen.iter().position(|&n| n >= 2)?;
    let x1 = kolommen.iter().rposition(|&n| n >= 2)?;
    if x1 <= x0 + 10 || y1 <= y0 + 5 {
        return None;
    }
    let marge = ((x1 - x0).max(y1 - y0) / 30).max(4);
    let (x0, y0) = (x0.saturating_sub(marge), y0.saturating_sub(marge));
    let (x1, y1) = ((x1 + marge).min(b - 1), (y1 + marge).min(h - 1));
    let (nb, nh) = (x1 - x0 + 1, y1 - y0 + 1);
    let mut uit = Beeld::nieuw(nb as u32, nh as u32);
    let inkt = gesorteerd[gesorteerd.len() / 100] as i32; // het donkerste stukje
    for y in 0..nh {
        for x in 0..nb {
            let g = grijs[(y0 + y) * b + x0 + x] as i32;
            let dekking = if papier - inkt <= 0 { 0.0 } else { ((papier - 15 - g) as f32 / (papier - 15 - inkt).max(1) as f32).clamp(0.0, 1.0) };
            let i = (y * nb + x) * 4;
            let bi = ((y0 + y) * b + x0 + x) * 4;
            // De kleur van de inkt houden (blauw of zwart), het papier doorzichtig.
            uit.punten[i] = bron.punten[bi].min(120);
            uit.punten[i + 1] = bron.punten[bi + 1].min(120);
            uit.punten[i + 2] = bron.punten[bi + 2].min(120);
            uit.punten[i + 3] = (dekking * 255.0) as u8;
        }
    }
    // Hooguit 900 punten breed: groot genoeg voor een scherpe afdruk.
    Some(if nb > 900 { uit.verklein(900) } else { uit })
}

/// Hoe groot de handtekening is (in centimeters, bij 300 dpi), voor de melding na het instellen.
pub fn maat_cm(b: &Beeld, dpi: f32) -> (f32, f32) {
    (b.breed as f32 / dpi * 2.54, b.hoog as f32 / dpi * 2.54)
}

/// Waar de handtekening moet komen: een vak in PDF-punten (oorsprong linksonder).
#[derive(Clone, Copy, Debug)]
pub struct Vak {
    pub pagina: i32,
    pub links: f32,
    pub onder: f32,
    pub breedte: f32,
    pub hoogte: f32,
    /// Het invulveld (nummer van de annotatie op de pagina) dat verborgen moet worden, anders ligt het over de handtekening.
    pub veld: Option<i32>,
}

/// De handtekening passend in het vak: (links, onder, breedte, hoogte). Een lijntje krijgt ruimte erboven.
fn passend(vak: &Vak, b: &Beeld) -> (f32, f32, f32, f32) {
    let hoogte = vak.hoogte.max(32.0);
    let breedte = (vak.breedte * 0.9).max(60.0);
    let verhouding = b.hoog as f32 / b.breed.max(1) as f32;
    let (mut w, mut h) = (breedte, breedte * verhouding);
    if h > hoogte {
        h = hoogte;
        w = h / verhouding.max(0.01);
    }
    (vak.links + 2.0, vak.onder + 1.0, w, h)
}

/// Zet de handtekening op de pagina's van een opgeslagen PDF (en slaat hem weer op).
pub fn op_pdf(pad: &Path, vakken: &[Vak], hand: &Beeld) -> Result<(), String> {
    use crate::formulier_pdf::{schrijf_blok, FileWrite};
    if vakken.is_empty() {
        return Ok(());
    }
    let p = crate::formulier_pdf::pdfium()?;
    let _slot = crate::formulier_pdf::SLOT.lock().unwrap();
    let data = std::fs::read(pad).map_err(|e| e.to_string())?;
    unsafe {
        let doc = (p.FPDF_LoadMemDocument)(data.as_ptr() as *const _, data.len() as i32, std::ptr::null());
        if doc.is_null() {
            return Err(t("LezenKapot"));
        }
        // Het plaatje als PDFium-bitmap met doorzichtigheid (BGRA).
        let bm = (p.FPDFBitmap_Create)(hand.breed as i32, hand.hoog as i32, 1);
        if bm.is_null() {
            (p.FPDF_CloseDocument)(doc);
            return Err(t("OpslaanMislukt"));
        }
        let buf = (p.FPDFBitmap_GetBuffer)(bm) as *mut u8;
        let stride = (p.FPDFBitmap_GetStride)(bm) as usize;
        for y in 0..hand.hoog as usize {
            std::ptr::copy_nonoverlapping(hand.punten.as_ptr().add(y * hand.breed as usize * 4), buf.add(y * stride), hand.breed as usize * 4);
        }
        let mut paginas: Vec<i32> = vakken.iter().map(|v| v.pagina).collect();
        paginas.sort();
        paginas.dedup();
        for nr in paginas {
            let mut page = (p.FPDF_LoadPage)(doc, nr);
            if page.is_null() {
                continue;
            }
            for vak in vakken.iter().filter(|v| v.pagina == nr) {
                if let Some(i) = vak.veld {
                    let a = (p.FPDFPage_GetAnnot)(page, i);
                    if !a.is_null() {
                        (p.FPDFAnnot_SetFlags)(a, (p.FPDFAnnot_GetFlags)(a) | 2); // 2 = verborgen
                        (p.FPDFPage_CloseAnnot)(a);
                    }
                }
                let obj = (p.FPDFPageObj_NewImageObj)(doc);
                if obj.is_null() {
                    continue;
                }
                if (p.FPDFImageObj_SetBitmap)(&mut page, 1, obj, bm) == 0 {
                    continue;
                }
                let (x, y, w, h) = passend(vak, hand);
                (p.FPDFImageObj_SetMatrix)(obj, w as f64, 0.0, 0.0, h as f64, x as f64, y as f64);
                (p.FPDFPage_InsertObject)(page, obj);
            }
            (p.FPDFPage_GenerateContent)(page);
            (p.FPDF_ClosePage)(page);
        }
        (p.FPDFBitmap_Destroy)(bm);
        let mut schrijver = FileWrite { version: 1, write_block: schrijf_blok, data: Vec::new() };
        let ok = (p.FPDF_SaveAsCopy)(doc, &mut schrijver, 0) != 0;
        (p.FPDF_CloseDocument)(doc);
        if !ok {
            return Err(t("OpslaanMislukt"));
        }
        std::fs::write(pad, &schrijver.data).map_err(|e| e.to_string())
    }
}

/// Zet de handtekening op een scan (in beeldpunten; `onder` is de onderkant, van boven gemeten).
pub fn op_beeld(pagina: &mut Beeld, links: f32, onder: f32, breedte: f32, hoogte: f32, hand: &Beeld) {
    // Zelfde verhoudingen als bij een PDF: 300 dpi is ongeveer 4,2 beeldpunten per PDF-punt.
    let per_punt = pagina.breed as f32 / 595.0;
    let vak = Vak { pagina: 0, links: links / per_punt, onder: 0.0, breedte: breedte / per_punt, hoogte: hoogte / per_punt, veld: None };
    let (_, _, w, h) = passend(&vak, hand);
    let (w, h) = ((w * per_punt) as i64, (h * per_punt) as i64);
    let x0 = (links + 2.0 * per_punt) as i64;
    let y0 = (onder - per_punt) as i64 - h;
    for y in 0..h.max(0) {
        for x in 0..w.max(0) {
            let (px, py) = (x0 + x, y0 + y);
            if px < 0 || py < 0 || px >= pagina.breed as i64 || py >= pagina.hoog as i64 {
                continue;
            }
            let sx = (x * hand.breed as i64 / w.max(1)).min(hand.breed as i64 - 1) as usize;
            let sy = (y * hand.hoog as i64 / h.max(1)).min(hand.hoog as i64 - 1) as usize;
            let s = (sy * hand.breed as usize + sx) * 4;
            let a = hand.punten[s + 3] as u32;
            if a == 0 {
                continue;
            }
            let d = ((py as usize) * pagina.breed as usize + px as usize) * 4;
            for k in 0..3 {
                pagina.punten[d + k] = ((hand.punten[s + k] as u32 * a + pagina.punten[d + k] as u32 * (255 - a)) / 255) as u8;
            }
        }
    }
}

/// Instellingen, Mijn handtekening: scannen, uit een bestand halen, of weghalen.
pub fn instellen(eigenaar: windows_sys::Win32::Foundation::HWND) {
    use crate::dialogen;
    let mut keuzes = vec![t("HandtekeningScannen"), t("HandtekeningBestand")];
    if heb() {
        keuzes.push(t("HandtekeningWeg"));
    }
    let uitleg = if heb() { t("HandtekeningUitlegHeb") } else { t("HandtekeningUitleg") };
    let Some(keuze) = crate::keuzevensters::kies_uit_lijst(eigenaar, &t("HandtekeningTitel"), &uitleg, &keuzes, 0) else { return };
    let beeld = match keuze {
        0 => crate::leesvenster::scan_een_beeld(eigenaar).map(|(b, dpi)| (b, Some(dpi as f32))),
        1 => crate::leesvenster::kies_afbeelding(eigenaar).and_then(|p| match crate::beeld::lees(&p) {
            Ok(mut v) if !v.is_empty() => Some((v.remove(0), None)),
            Ok(_) => None,
            Err(e) => {
                dialogen::melding(eigenaar, &t("HandtekeningTitel"), &e, dialogen::ICOON_WAARSCHUWING);
                None
            }
        }),
        _ => {
            if dialogen::ja_nee(eigenaar, &t("HandtekeningTitel"), &t("HandtekeningWegVraag"), false) {
                wis();
                dialogen::melding(eigenaar, &t("HandtekeningTitel"), &t("HandtekeningWegKlaar"), dialogen::ICOON_INFO);
            }
            return;
        }
    };
    let Some((beeld, dpi)) = beeld else { return };
    match uit_beeld(&beeld) {
        Some(hand) => {
            let zin = match dpi {
                Some(dpi) => {
                    let (b, h) = maat_cm(&hand, dpi);
                    tf("HandtekeningKlaarMaat", &[&format!("{b:.1}").replace('.', ","), &format!("{h:.1}").replace('.', ",")])
                }
                None => t("HandtekeningKlaar"),
            };
            match bewaar(&hand) {
                Ok(_) => {
                    crate::logboek::schrijf("Handtekening bewaard.");
                    dialogen::melding(eigenaar, &t("HandtekeningTitel"), &zin, dialogen::ICOON_INFO);
                }
                Err(e) => dialogen::melding(eigenaar, &t("OpslaanMislukt"), &e, dialogen::ICOON_WAARSCHUWING),
            }
        }
        None => dialogen::melding(eigenaar, &t("HandtekeningTitel"), &t("HandtekeningNiets"), dialogen::ICOON_WAARSCHUWING),
    }
}
