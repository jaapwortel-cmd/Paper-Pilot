// Paper Pilot - formulieren zonder invulvelden: zelf de plekken vinden om in te vullen, en ze daarna invullen.
//
// Werkt op een pagina als afbeelding (grijs) met de tekstregels en hun woorden (uit de PDF zelf, of uit de
// tekstherkenning bij een scan). Paper Pilot zoekt:
// 1. een vraag met een dubbele punt en lege ruimte erachter ("Voorletters:      ");
// 2. hokjes om aan te vinken: het teken □ in de tekst, of een getekend vierkantje;
// 3. lijntjes: getekende streepjes, of ____ en ..... in de tekst.
// Alles in beeldpunten van de afbeelding (y van boven naar beneden). Bij een PDF met tekst schrijft Paper Pilot het
// antwoord daarna op de pagina zelf (PDFium); bij een scan maakt Paper Pilot een nieuwe PDF met de scan en de antwoorden.

use crate::herkennen::Regel;
use std::path::Path;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Plaats {
    Tekst,
    Vinkje,
}

/// Een gevonden plek om in te vullen (in beeldpunten van de paginabeeld).
#[derive(Clone, Debug)]
pub struct Plek {
    pub pagina: usize,
    pub soort: Plaats,
    pub vraag: String,
    pub x: f32,
    /// Onderkant van de plek (waar de letters op staan).
    pub onder: f32,
    pub breedte: f32,
    pub hoogte: f32,
}

/// Een pagina als grijze afbeelding: 0 = zwart, 255 = wit.
pub struct Grijs {
    pub b: usize,
    pub h: usize,
    pub p: Vec<u8>,
}

impl Grijs {
    pub fn van(beeld: &crate::beeld::Beeld) -> Grijs {
        let p = beeld.punten.chunks_exact(4).map(|c| ((c[0] as u32 * 29 + c[1] as u32 * 150 + c[2] as u32 * 77) >> 8) as u8).collect();
        Grijs { b: beeld.breed as usize, h: beeld.hoog as usize, p }
    }
    fn donker(&self, x: usize, y: usize) -> bool {
        x < self.b && y < self.h && self.p[y * self.b + x] < 140
    }
    /// Deel donkere punten in een rechthoek.
    fn deel_donker(&self, x0: f32, y0: f32, x1: f32, y1: f32) -> f32 {
        let (x0, x1) = (x0.max(0.0) as usize, (x1.max(0.0) as usize).min(self.b));
        let (y0, y1) = (y0.max(0.0) as usize, (y1.max(0.0) as usize).min(self.h));
        if x1 <= x0 || y1 <= y0 {
            return 0.0;
        }
        let mut n = 0usize;
        for y in y0..y1 {
            for x in x0..x1 {
                if self.donker(x, y) {
                    n += 1;
                }
            }
        }
        n as f32 / ((x1 - x0) * (y1 - y0)) as f32
    }
}

const HOKJES: [char; 6] = ['□', '☐', '❑', '○', '◯', '❍'];

fn schoon(s: &str) -> String {
    s.trim().trim_end_matches(['.', '…', '_', ' ']).trim().trim_end_matches(':').trim().to_string()
}

/// Zoekt de invulplekken op één pagina.
pub fn zoek(pagina: usize, g: &Grijs, regels: &[Regel]) -> Vec<Plek> {
    let mut uit: Vec<Plek> = Vec::new();
    if regels.is_empty() {
        return uit;
    }
    let mut hoogtes: Vec<f32> = regels.iter().map(|r| r.hoogte).collect();
    hoogtes.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let lh = hoogtes[hoogtes.len() / 2].max(4.0); // gewone letterhoogte
    let rechterrand = g.b as f32 * 0.93;

    for (ri, r) in regels.iter().enumerate() {
        let w = &r.woorden;
        // --- 2. hokjes in de tekst (□ Man □ Vrouw)
        let hokjes: Vec<usize> = (0..w.len()).filter(|&i| w[i].0.chars().next().map(|c| HOKJES.contains(&c)).unwrap_or(false)).collect();
        if !hokjes.is_empty() {
            // De vraag bij de groep: tekst vóór het eerste hokje, anders de regel erboven.
            let voor = w[..hokjes[0]].iter().map(|x| x.0.as_str()).collect::<Vec<_>>().join(" ");
            let groep = if !schoon(&voor).is_empty() { schoon(&voor) } else if ri > 0 && regels[ri - 1].tekst.trim_end().ends_with(':') {
                schoon(&regels[ri - 1].tekst) } else { String::new() };
            for (k, &i) in hokjes.iter().enumerate() {
                let tot = hokjes.get(k + 1).copied().unwrap_or(w.len());
                let mut label: Vec<String> = Vec::new();
                let eerste = w[i].0.chars().skip(1).collect::<String>();
                if !eerste.is_empty() {
                    label.push(eerste);
                }
                for x in &w[i + 1..tot] {
                    label.push(x.0.clone());
                }
                let label = schoon(&label.join(" "));
                let vraag = if groep.is_empty() { label.clone() } else if label.is_empty() { groep.clone() } else { format!("{groep}: {label}") };
                let maat = r.hoogte.min(w[i].2.max(r.hoogte * 0.6));
                uit.push(Plek { pagina, soort: Plaats::Vinkje, vraag, x: w[i].1, onder: r.boven + r.hoogte, breedte: maat, hoogte: maat });
            }
            continue;
        }
        // --- 3a. ____ of ..... in de tekst
        for (i, woord) in w.iter().enumerate() {
            let t = woord.0.as_str();
            if t.chars().count() >= 4 && t.chars().all(|c| c == '_' || c == '.' || c == '…') {
                let label = schoon(&w[..i].iter().map(|x| x.0.as_str()).collect::<Vec<_>>().join(" "));
                let label = if label.is_empty() && ri > 0 { schoon(&regels[ri - 1].tekst) } else { label };
                // Iets boven de puntjes of streepjes, zodat het antwoord er niet overheen staat.
                uit.push(Plek { pagina, soort: Plaats::Tekst, vraag: label, x: woord.1, onder: r.boven + r.hoogte * 0.75, breedte: woord.2, hoogte: lh });
            }
        }
        // --- 1. vraag met dubbele punt en lege ruimte erachter
        for (i, woord) in w.iter().enumerate() {
            if !woord.0.ends_with(':') || woord.0.contains("://") {
                continue;
            }
            let eind = woord.1 + woord.2;
            let volgende = w.get(i + 1).map(|x| x.1).unwrap_or(rechterrand);
            let ruimte = volgende - eind;
            if ruimte < lh * 4.0 {
                continue; // het antwoord staat er al, of er is geen plek
            }
            // Vraag: de woorden vanaf de vorige dubbele punt (of het begin van de regel) tot hier.
            let begin = (0..i).rev().find(|&k| w[k].0.ends_with(':')).map(|k| k + 1).unwrap_or(0);
            let vraag = schoon(&w[begin..=i].iter().map(|x| x.0.as_str()).collect::<Vec<_>>().join(" "));
            if vraag.is_empty() || vraag.split_whitespace().count() > 10 {
                continue;
            }
            // Staat er een hokje (kader) in de lege ruimte? Dan komt het antwoord daarin.
            let midden = (r.boven + r.hoogte * 0.5) as usize;
            let staand = |xx: usize| (midden.saturating_sub((lh * 1.0) as usize)..midden + (lh * 1.0) as usize)
                .filter(|&yy| g.donker(xx, yy)).count() as f32 >= lh * 1.1;
            let mut x = eind + lh * 0.6;
            let mut breedte = ruimte - lh * 1.2;
            if let Some(kl) = (eind as usize + 2..(eind + ruimte) as usize).find(|&xx| g.donker(xx, midden) && staand(xx)) {
                let kr = (kl + 6..(eind + ruimte) as usize).find(|&xx| g.donker(xx, midden) && staand(xx)).unwrap_or((eind + ruimte) as usize);
                x = kl as f32 + lh * 0.5;
                breedte = kr as f32 - x - lh * 0.5;
            }
            uit.push(Plek { pagina, soort: Plaats::Tekst, vraag, x, onder: r.boven + r.hoogte, breedte, hoogte: lh });
        }
    }

    // --- 3b. getekende lijntjes (en 2b. getekende vierkantjes)
    // Staat er een woord in deze rechthoek? (Per woord, niet per regel: tussen woorden kan een lijntje staan.)
    let tekst_op = |x0: f32, y0: f32, x1: f32, y1: f32| regels.iter().any(|r| {
        r.boven < y1 && r.boven + r.hoogte > y0 && r.woorden.iter().any(|w| w.1 < x1 && w.1 + w.2 > x0)
    });
    // De woorden direct links van x op dezelfde hoogte (tot een groot gat): de vraag bij een lijntje of hokje.
    let links_van = |x: f32, y: f32| -> String {
        let mut woorden: Vec<&(String, f32, f32)> = regels.iter()
            .filter(|r| (r.boven + r.hoogte - y).abs() < lh * 1.2)
            .flat_map(|r| r.woorden.iter()).filter(|w| w.1 + w.2 <= x + 2.0).collect();
        woorden.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        let mut gekozen: Vec<&str> = Vec::new();
        let mut grens = x;
        for w in woorden.iter().rev() {
            // Het eerste woord mag verder weg staan (een lijntje begint vaak een eind na de vraag).
            let max_gat = if gekozen.is_empty() { g.b as f32 * 0.3 } else { lh * 2.5 };
            if grens - (w.1 + w.2) > max_gat {
                break;
            }
            gekozen.insert(0, &w.0);
            grens = w.1;
        }
        schoon(&gekozen.join(" "))
    };
    let min_lijn = (g.b as f32 * 0.10).max(lh * 6.0) as usize;
    let mut y = 0usize;
    while y < g.h {
        let mut x = 0usize;
        let mut gevonden = false;
        while x < g.b {
            if !g.donker(x, y) {
                x += 1;
                continue;
            }
            let begin = x;
            while x < g.b && g.donker(x, y) {
                x += 1;
            }
            let lengte = x - begin;
            if lengte < min_lijn {
                continue;
            }
            // Dun (geen blok of balk) en geen onderstreepte tekst, en erboven leeg.
            let dik = (1..6).take_while(|d| y + d < g.h && g.donker(begin + lengte / 2, y + d)).count();
            if dik > (lh * 0.3) as usize {
                continue;
            }
            let (x0, x1, yl) = (begin as f32, x as f32, y as f32);
            // De bovenrand van een hokje of kader: dan hoort de plek bij de onderrand (die komt zo vanzelf).
            let onderrand = ((lh * 0.5) as usize..(lh * 3.0) as usize).any(|d| {
                y + d < g.h && (begin..x).step_by(4).filter(|&xx| g.donker(xx, y + d)).count() * 10 >= ((x - begin) / 4) * 8
            });
            let zijkant = (2..(lh * 0.5) as usize).all(|d| y + d < g.h && (g.donker(begin, y + d) || g.donker(begin + 1, y + d)));
            if onderrand && zijkant {
                continue;
            }
            if tekst_op(x0 + 2.0, yl - lh * 0.9, x1 - 2.0, yl - 1.0) || g.deel_donker(x0, yl - lh * 1.2, x1, yl - 2.0) > 0.04 {
                continue;
            }
            // Zit er al een plek (dubbele punt) op deze hoogte en plaats? Dan niet nog een keer.
            if uit.iter().any(|p| p.pagina == pagina && (p.onder - yl).abs() < lh * 1.3 && p.x < x1 && p.x + p.breedte > x0) {
                continue;
            }
            // De vraag: tekst links op dezelfde hoogte, of net erboven.
            let boven = regels.iter().filter(|r| r.boven + r.hoogte <= yl - lh * 0.8 && r.boven + r.hoogte >= yl - lh * 3.0 && r.links < x1 && r.links + r.breedte > x0)
                .max_by(|a, b| a.boven.partial_cmp(&b.boven).unwrap_or(std::cmp::Ordering::Equal));
            let links = links_van(x0, yl);
            let vraag = if !links.is_empty() { links } else { boven.map(|r| schoon(&r.tekst)).unwrap_or_default() };
            uit.push(Plek { pagina, soort: Plaats::Tekst, vraag, x: x0 + lh * 0.2, onder: yl - 2.0, breedte: x1 - x0 - lh * 0.4, hoogte: lh });
            gevonden = true;
        }
        y += if gevonden { (lh * 0.5) as usize + 1 } else { 1 };
    }

    // Getekende vierkantjes (hokjes) zonder teken in de tekst.
    let (min_z, max_z) = ((lh * 0.6) as usize, (lh * 2.0) as usize + 2);
    let mut y = 0usize;
    while y + min_z < g.h {
        let mut x = 0usize;
        while x + min_z < g.b {
            if !g.donker(x, y) || (x > 0 && g.donker(x - 1, y)) {
                x += 1;
                continue;
            }
            let mut z = 0;
            while x + z < g.b && g.donker(x + z, y) {
                z += 1;
            }
            if z >= min_z && z <= max_z {
                // Onderkant, linker- en rechterkant donker; binnenkant licht.
                let onder = (z.saturating_sub(3)..=z + 3).find(|&d| y + d < g.h && (0..z).filter(|&i| g.donker(x + i, y + d)).count() * 10 >= z * 8);
                if let Some(d) = onder {
                    let zijkant = |xx: usize| (0..d).filter(|&j| g.donker(xx, y + j) || g.donker(xx + 1, y + j)).count() * 10 >= d * 8;
                    let binnen = g.deel_donker(x as f32 + 3.0, y as f32 + 3.0, (x + z) as f32 - 3.0, (y + d) as f32 - 3.0);
                    if zijkant(x) && zijkant(x + z - 2) && binnen < 0.08 && !tekst_op(x as f32 + 2.0, y as f32 + 2.0, (x + z) as f32 - 2.0, (y + d) as f32 - 2.0) {
                        let (xf, yf, zf) = (x as f32, y as f32, z as f32);
                        let al = uit.iter().any(|p| p.pagina == pagina && (p.x - xf).abs() < zf * 1.5 && (p.onder - (yf + d as f32)).abs() < zf * 1.5);
                        if !al {
                            // De vraag: tekst rechts ernaast op dezelfde hoogte.
                            let label = regels.iter().filter(|r| (r.boven + r.hoogte / 2.0 - (yf + d as f32 / 2.0)).abs() < lh * 0.8)
                                .flat_map(|r| r.woorden.iter()).filter(|w| w.1 >= xf + zf && w.1 < xf + zf + lh * 12.0)
                                .map(|w| w.0.clone()).collect::<Vec<_>>();
                            let label = schoon(&label.into_iter().take(8).collect::<Vec<_>>().join(" "));
                            uit.push(Plek { pagina, soort: Plaats::Vinkje, vraag: label, x: xf, onder: yf + d as f32, breedte: zf, hoogte: d as f32 });
                        }
                        x += z;
                        continue;
                    }
                }
            }
            x += z.max(1);
        }
        y += 1;
    }

    // Hokjes naast elkaar op één regel: de vraag bij een hokje stopt bij het volgende hokje.
    let kopie = uit.clone();
    for p in uit.iter_mut().filter(|p| p.soort == Plaats::Vinkje) {
        let volgende = kopie.iter().filter(|q| q.soort == Plaats::Vinkje && (q.onder - p.onder).abs() < lh && q.x > p.x + p.breedte)
            .map(|q| q.x).fold(f32::MAX, f32::min);
        if volgende < f32::MAX {
            let woorden: Vec<String> = regels.iter().filter(|r| (r.boven + r.hoogte / 2.0 - (p.onder - p.hoogte / 2.0)).abs() < lh * 0.8)
                .flat_map(|r| r.woorden.iter()).filter(|w| w.1 >= p.x + p.breedte - 2.0 && w.1 + w.2 <= volgende + 2.0)
                .filter(|w| !w.0.chars().next().map(|c| HOKJES.contains(&c)).unwrap_or(false))
                .map(|w| w.0.clone()).collect();
            let nieuw = schoon(&woorden.join(" "));
            if !nieuw.is_empty() {
                // Een groepsvraag ervoor ("Geslacht: ") blijft staan.
                p.vraag = match p.vraag.split_once(": ") {
                    Some((groep, _)) if p.vraag.contains(": ") => format!("{groep}: {nieuw}"),
                    _ => nieuw,
                };
            }
        }
    }
    // Plekken zonder vraag, of met een hele zin als "vraag", zijn bijna altijd geen echte invulplek.
    uit.retain(|p| !p.vraag.trim().is_empty() && p.vraag.split_whitespace().count() <= 10);

    // Leesvolgorde.
    uit.sort_by(|a, b| {
        if (a.onder - b.onder).abs() < lh * 0.6 {
            a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal)
        } else {
            a.onder.partial_cmp(&b.onder).unwrap_or(std::cmp::Ordering::Equal)
        }
    });
    uit
}

// ------------------------------------------------------------------ PDF met tekst (zoals de CZ-formulieren)

/// Tekstregels met woorden van een PDF-pagina, in beeldpunten bij `schaal` (beeldpunten per punt).
fn pdf_regels(p: &crate::formulier_pdf::Pdfium, page: crate::formulier_pdf::Page, schaal: f32, hoogte_pt: f32) -> Vec<Regel> {
    let mut regels: Vec<Regel> = Vec::new();
    unsafe {
        let tp = (p.FPDFText_LoadPage)(page);
        if tp.is_null() {
            return regels;
        }
        let n = (p.FPDFText_CountChars)(tp);
        // (teken, links, rechts, boven, onder) in beeldpunten
        let mut tekens: Vec<(char, f32, f32, f32, f32)> = Vec::new();
        for i in 0..n {
            let c = char::from_u32((p.FPDFText_GetUnicode)(tp, i)).unwrap_or(' ');
            let (mut l, mut r, mut o, mut b) = (0f64, 0f64, 0f64, 0f64);
            (p.FPDFText_GetCharBox)(tp, i, &mut l, &mut r, &mut o, &mut b);
            tekens.push((c, l as f32 * schaal, r as f32 * schaal, (hoogte_pt - b as f32) * schaal, (hoogte_pt - o as f32) * schaal));
        }
        (p.FPDFText_ClosePage)(tp);
        // Regels: tot een regeleinde; woorden: tot een spatie of een groot gat.
        let mut huidig: Vec<(char, f32, f32, f32, f32)> = Vec::new();
        let mut sluit = |huidig: &mut Vec<(char, f32, f32, f32, f32)>| {
            let zichtbaar: Vec<_> = huidig.iter().filter(|t| !t.0.is_whitespace() && t.2 > t.1).cloned().collect();
            if !zichtbaar.is_empty() {
                let boven = zichtbaar.iter().map(|t| t.3).fold(f32::MAX, f32::min);
                let onder = zichtbaar.iter().map(|t| t.4).fold(f32::MIN, f32::max);
                let links = zichtbaar.iter().map(|t| t.1).fold(f32::MAX, f32::min);
                let rechts = zichtbaar.iter().map(|t| t.2).fold(f32::MIN, f32::max);
                let hoogte = (onder - boven).max(1.0);
                let mut woorden: Vec<(String, f32, f32)> = Vec::new();
                let mut w = String::new();
                let (mut wl, mut wr) = (0f32, 0f32);
                for t in huidig.iter() {
                    let gat = !w.is_empty() && t.1 - wr > hoogte * 0.6;
                    if t.0.is_whitespace() || gat || HOKJES.contains(&t.0) && !w.is_empty() {
                        if !w.is_empty() {
                            woorden.push((std::mem::take(&mut w), wl, wr - wl));
                        }
                        if t.0.is_whitespace() {
                            continue;
                        }
                    }
                    if t.2 <= t.1 {
                        continue;
                    }
                    if w.is_empty() {
                        wl = t.1;
                    }
                    w.push(t.0);
                    wr = t.2;
                }
                if !w.is_empty() {
                    woorden.push((w, wl, wr - wl));
                }
                let tekst = woorden.iter().map(|w| w.0.as_str()).collect::<Vec<_>>().join(" ");
                regels.push(Regel { tekst, boven, hoogte, links, breedte: rechts - links, woorden });
            }
            huidig.clear();
        };
        for t in tekens {
            if t.0 == '\r' || t.0 == '\n' {
                sluit(&mut huidig);
                continue;
            }
            huidig.push(t);
        }
        sluit(&mut huidig);
    }
    regels
}

/// Een PDF-pagina als grijze afbeelding (wit papier).
fn pdf_grijs(p: &crate::formulier_pdf::Pdfium, page: crate::formulier_pdf::Page, b: i32, h: i32) -> Grijs {
    unsafe {
        let bm = (p.FPDFBitmap_Create)(b, h, 0);
        (p.FPDFBitmap_FillRect)(bm, 0, 0, b, h, 0xFFFF_FFFF);
        (p.FPDF_RenderPageBitmap)(bm, page, 0, 0, b, h, 0, 0x01);
        let buf = (p.FPDFBitmap_GetBuffer)(bm) as *const u8;
        let stride = (p.FPDFBitmap_GetStride)(bm) as usize;
        let mut grijs = vec![255u8; (b * h) as usize];
        for y in 0..h as usize {
            let rij = std::slice::from_raw_parts(buf.add(y * stride), (b * 4) as usize);
            for x in 0..b as usize {
                let c = &rij[x * 4..x * 4 + 3];
                grijs[y * b as usize + x] = ((c[0] as u32 * 29 + c[1] as u32 * 150 + c[2] as u32 * 77) >> 8) as u8;
            }
        }
        (p.FPDFBitmap_Destroy)(bm);
        Grijs { b: b as usize, h: h as usize, p: grijs }
    }
}

/// Beeldpunten per punt bij het zoeken in een PDF (2 = 144 dpi).
pub const PDF_SCHAAL: f32 = 2.0;

/// Zoekt invulplekken in een PDF met tekst (zonder invulvelden).
pub fn zoek_pdf(pad: &Path) -> Result<Vec<Plek>, String> {
    let p = crate::formulier_pdf::pdfium()?;
    let _slot = crate::formulier_pdf::SLOT.lock().unwrap();
    let data = std::fs::read(pad).map_err(|e| e.to_string())?;
    let mut uit = Vec::new();
    unsafe {
        let doc = (p.FPDF_LoadMemDocument)(data.as_ptr() as *const _, data.len() as i32, std::ptr::null());
        if doc.is_null() {
            return Err(crate::teksten::t("LezenKapot"));
        }
        for nr in 0..(p.FPDF_GetPageCount)(doc) {
            let page = (p.FPDF_LoadPage)(doc, nr);
            if page.is_null() {
                continue;
            }
            let (bp, hp) = ((p.FPDF_GetPageWidthF)(page), (p.FPDF_GetPageHeightF)(page));
            let regels = pdf_regels(p, page, PDF_SCHAAL, hp);
            let g = pdf_grijs(p, page, (bp * PDF_SCHAAL) as i32, (hp * PDF_SCHAAL) as i32);
            uit.extend(zoek(nr as usize, &g, &regels));
            (p.FPDF_ClosePage)(page);
        }
        (p.FPDF_CloseDocument)(doc);
    }
    Ok(uit)
}

/// Zoekt invulplekken in gescande pagina's (met de herkende regels).
pub fn zoek_beelden(beelden: &[crate::beeld::Beeld], regels: &[Vec<Regel>]) -> Vec<Plek> {
    let mut uit = Vec::new();
    for (i, b) in beelden.iter().enumerate() {
        let r = regels.get(i).cloned().unwrap_or_default();
        uit.extend(zoek(i, &Grijs::van(b), &r));
    }
    uit
}

/// Wat op een plek komt: tekst, of een kruisje bij een aangevinkt hokje.
fn inhoud(plek: &Plek, waarde: &str) -> Option<(String, f32)> {
    match plek.soort {
        Plaats::Tekst if !waarde.trim().is_empty() => {
            let tekst = waarde.replace(['\r', '\n'], " ");
            // Lettergrootte: passend in de hoogte en (zo nodig kleiner) in de breedte.
            let mut grootte = (plek.hoogte * 1.25).max(6.0);
            let nodig = 0.52 * grootte * tekst.chars().count() as f32;
            if plek.breedte > 0.0 && nodig > plek.breedte {
                grootte = (grootte * plek.breedte / nodig).max(plek.hoogte * 0.45);
            }
            Some((tekst, grootte))
        }
        Plaats::Vinkje if !waarde.is_empty() => Some(("X".into(), plek.hoogte * 0.95)),
        _ => None,
    }
}

/// Schrijft de antwoorden op de pagina's van een PDF met tekst en slaat op als nieuwe PDF.
pub fn vul_pdf_in(bron: &Path, plekken: &[(Plek, String)], doel: &Path) -> Result<(), String> {
    use crate::formulier_pdf::{schrijf_blok, FileWrite};
    let p = crate::formulier_pdf::pdfium()?;
    let _slot = crate::formulier_pdf::SLOT.lock().unwrap();
    let data = std::fs::read(bron).map_err(|e| e.to_string())?;
    unsafe {
        let doc = (p.FPDF_LoadMemDocument)(data.as_ptr() as *const _, data.len() as i32, std::ptr::null());
        if doc.is_null() {
            return Err(crate::teksten::t("LezenKapot"));
        }
        let font = b"Helvetica\0";
        let mut paginas: Vec<usize> = plekken.iter().map(|(pl, _)| pl.pagina).collect();
        paginas.sort();
        paginas.dedup();
        for nr in paginas {
            let page = (p.FPDF_LoadPage)(doc, nr as i32);
            if page.is_null() {
                continue;
            }
            let hp = (p.FPDF_GetPageHeightF)(page);
            for (plek, waarde) in plekken.iter().filter(|(pl, _)| pl.pagina == nr) {
                let Some((tekst, grootte_px)) = inhoud(plek, waarde) else { continue };
                let grootte = grootte_px / PDF_SCHAAL;
                let obj = (p.FPDFPageObj_NewTextObj)(doc, font.as_ptr() as *const i8, grootte);
                if obj.is_null() {
                    continue;
                }
                let w: Vec<u16> = tekst.encode_utf16().chain(std::iter::once(0)).collect();
                (p.FPDFText_SetText)(obj, w.as_ptr());
                let (x, y) = if plek.soort == Plaats::Vinkje {
                    // Kruisje midden in het hokje.
                    (plek.x / PDF_SCHAAL + (plek.breedte / PDF_SCHAAL - grootte * 0.62) / 2.0,
                     hp - plek.onder / PDF_SCHAAL + (plek.hoogte / PDF_SCHAAL - grootte * 0.72) / 2.0)
                } else {
                    (plek.x / PDF_SCHAAL, hp - plek.onder / PDF_SCHAAL + grootte * 0.18)
                };
                (p.FPDFPageObj_Transform)(obj, 1.0, 0.0, 0.0, 1.0, x as f64, y as f64);
                (p.FPDFPage_InsertObject)(page, obj);
            }
            (p.FPDFPage_GenerateContent)(page);
            (p.FPDF_ClosePage)(page);
        }
        let mut schrijver = FileWrite { version: 1, write_block: schrijf_blok, data: Vec::new() };
        let ok = (p.FPDF_SaveAsCopy)(doc, &mut schrijver, 0) != 0;
        (p.FPDF_CloseDocument)(doc);
        if !ok {
            return Err(crate::teksten::t("OpslaanMislukt"));
        }
        std::fs::write(doel, &schrijver.data).map_err(|e| e.to_string())
    }
}

/// De antwoorden als zichtbare tekst per scanpagina (voor pdf_maken).
pub fn zichtbaar(plekken: &[(Plek, String)], aantal: usize) -> Vec<Vec<(String, f32, f32, f32)>> {
    let mut uit = vec![Vec::new(); aantal];
    for (plek, waarde) in plekken {
        let Some((tekst, grootte)) = inhoud(plek, waarde) else { continue };
        let (x, onder) = if plek.soort == Plaats::Vinkje {
            (plek.x + (plek.breedte - grootte * 0.62) / 2.0, plek.onder - (plek.hoogte - grootte * 0.72) / 2.0)
        } else {
            (plek.x, plek.onder - grootte * 0.18)
        };
        if let Some(p) = uit.get_mut(plek.pagina) {
            p.push((tekst, x, onder, grootte));
        }
    }
    uit
}
