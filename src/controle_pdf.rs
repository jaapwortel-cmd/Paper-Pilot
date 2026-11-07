// Paper Pilot - Controleren: is deze PDF goed te lezen met een schermlezer?
//
// Met PDFium: is er echte tekst (geen scan), zijn de tekens goed (geen vreemde lettertypen), is de PDF getagd
// (structuur met koppen, alinea's, lijsten, tabellen), titel en taal, beschrijvingen bij plaatjes, kopcellen in
// tabellen, bladwijzers bij een lange PDF, namen bij invulvelden, en of beveiliging de schermlezer niet tegenhoudt.

use crate::controleren::{Ernst, Verslag};
use crate::teksten::t;
use std::ffi::c_void;
use std::path::Path;

fn utf16(f: impl Fn(*mut u16, u32) -> u32) -> String {
    let n = f(std::ptr::null_mut(), 0);
    if n <= 2 {
        return String::new();
    }
    let mut buf = vec![0u16; (n as usize).div_ceil(2)];
    f(buf.as_mut_ptr(), n);
    while buf.last() == Some(&0) {
        buf.pop();
    }
    String::from_utf16_lossy(&buf).trim().to_string()
}

#[derive(Default)]
struct Structuur {
    koppen: Vec<(usize, u8)>, // (pagina, niveau; 0 = H zonder nummer)
    plaatjes: usize,
    plaatjes_zonder: Vec<usize>,
    tabellen: usize,
    tabellen_zonder_kop: Vec<usize>,
    lijsten: usize,
    links: usize,
}

unsafe fn loop_door(p: &crate::formulier_pdf::Pdfium, e: *mut c_void, pagina: usize, s: &mut Structuur, diepte: usize) {
    if e.is_null() || diepte > 60 {
        return;
    }
    let soort = utf16(|b, n| (p.FPDF_StructElement_GetType)(e, b, n));
    match soort.as_str() {
        "H" => s.koppen.push((pagina, 0)),
        "H1" | "H2" | "H3" | "H4" | "H5" | "H6" => s.koppen.push((pagina, soort.as_bytes()[1] - b'0')),
        "Figure" => {
            s.plaatjes += 1;
            let alt = utf16(|b, n| (p.FPDF_StructElement_GetAltText)(e, b, n));
            let echt = utf16(|b, n| (p.FPDF_StructElement_GetActualText)(e, b, n));
            if alt.is_empty() && echt.is_empty() {
                s.plaatjes_zonder.push(pagina);
            }
        }
        "Table" => {
            s.tabellen += 1;
            if !heeft_kind(p, e, "TH", 0) {
                s.tabellen_zonder_kop.push(pagina);
            }
        }
        "L" => s.lijsten += 1,
        "Link" => s.links += 1,
        _ => {}
    }
    for i in 0..(p.FPDF_StructElement_CountChildren)(e).max(0) {
        loop_door(p, (p.FPDF_StructElement_GetChildAtIndex)(e, i), pagina, s, diepte + 1);
    }
}

unsafe fn heeft_kind(p: &crate::formulier_pdf::Pdfium, e: *mut c_void, soort: &str, diepte: usize) -> bool {
    if diepte > 8 {
        return false;
    }
    for i in 0..(p.FPDF_StructElement_CountChildren)(e).max(0) {
        let k = (p.FPDF_StructElement_GetChildAtIndex)(e, i);
        if k.is_null() {
            continue;
        }
        if utf16(|b, n| (p.FPDF_StructElement_GetType)(k, b, n)) == soort || heeft_kind(p, k, soort, diepte + 1) {
            return true;
        }
    }
    false
}

fn paginas(lijst: &[usize]) -> Vec<String> {
    let mut l: Vec<usize> = lijst.to_vec();
    l.sort();
    l.dedup();
    l.iter().map(|n| crate::teksten::tf("ControlePagina", &[&n.to_string()])).collect()
}

pub fn controleer(pad: &Path) -> Result<Verslag, String> {
    let naam = pad.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let mut v = Verslag::nieuw(&naam);
    let velden = crate::formulier_pdf::velden_met_beschrijving(pad).unwrap_or_default();
    let p = crate::formulier_pdf::pdfium()?;
    let _slot = crate::formulier_pdf::SLOT.lock().unwrap();
    let data = std::fs::read(pad).map_err(|e| e.to_string())?;
    unsafe {
        let doc = (p.FPDF_LoadMemDocument)(data.as_ptr() as *const c_void, data.len() as i32, std::ptr::null());
        if doc.is_null() {
            return Err(t("LezenKapot"));
        }
        let aantal = (p.FPDF_GetPageCount)(doc).max(0) as usize;
        let getagd = (p.FPDFCatalog_IsTagged)(doc) != 0;
        let titel = utf16(|b, n| (p.FPDF_GetMetaText)(doc, b"Title\0".as_ptr(), b, n));
        let taal = utf16(|b, n| (p.FPDFCatalog_GetLanguage)(doc, b, n));
        let bladwijzers = !(p.FPDFBookmark_GetFirstChild)(doc, std::ptr::null_mut()).is_null();
        let rechten = (p.FPDF_GetDocPermissions)(doc);
        let mut zonder_tekst = Vec::new();
        let mut tekens = 0usize;
        let mut vreemd = 0usize;
        let mut plaatjes_op = Vec::new();
        let mut s = Structuur::default();
        for nr in 0..aantal {
            let page = (p.FPDF_LoadPage)(doc, nr as i32);
            if page.is_null() {
                continue;
            }
            let tp = (p.FPDFText_LoadPage)(page);
            let mut hier = 0;
            if !tp.is_null() {
                for i in 0..(p.FPDFText_CountChars)(tp).max(0) {
                    let c = (p.FPDFText_GetUnicode)(tp, i);
                    if c <= 32 || c == 0xFFFE {
                        continue;
                    }
                    hier += 1;
                    if c == 0xFFFD || (0xE000..=0xF8FF).contains(&c) || c < 32 {
                        vreemd += 1;
                    }
                }
                (p.FPDFText_ClosePage)(tp);
            }
            tekens += hier;
            for i in 0..(p.FPDFPage_CountObjects)(page).max(0) {
                if (p.FPDFPageObj_GetType)((p.FPDFPage_GetObject)(page, i)) == 3 {
                    plaatjes_op.push(nr + 1);
                    break;
                }
            }
            if hier == 0 {
                zonder_tekst.push(nr + 1);
            }
            if getagd {
                let boom = (p.FPDF_StructTree_GetForPage)(page);
                if !boom.is_null() {
                    for i in 0..(p.FPDF_StructTree_CountChildren)(boom).max(0) {
                        loop_door(p, (p.FPDF_StructTree_GetChildAtIndex)(boom, i), nr + 1, &mut s, 0);
                    }
                    (p.FPDF_StructTree_Close)(boom);
                }
            }
            (p.FPDF_ClosePage)(page);
        }
        (p.FPDF_CloseDocument)(doc);

        // 1. Echte tekst?
        if aantal > 0 && zonder_tekst.len() == aantal {
            v.punt(Ernst::Probleem, "CPdfAllesScan", &[], Vec::new());
        } else if !zonder_tekst.is_empty() {
            v.punt(Ernst::Probleem, "CPdfScanPaginas", &[&zonder_tekst.len().to_string()], paginas(&zonder_tekst));
        } else {
            v.goed("CPdfTekstGoed", &[]);
        }
        // 2. Goede tekens?
        if tekens > 0 && vreemd * 50 > tekens {
            v.punt(Ernst::Probleem, "CPdfVreemdeTekens", &[&(vreemd * 100 / tekens).to_string()], Vec::new());
        } else if tekens > 0 {
            v.goed("CPdfTekensGoed", &[]);
        }
        // 3. Getagd?
        if !getagd {
            v.punt(Ernst::Probleem, "CPdfNietGetagd", &[], Vec::new());
        } else {
            v.goed("CPdfGetagd", &[]);
        }
        // 4. Titel en taal.
        let stam = pad.file_stem().map(|x| x.to_string_lossy().to_lowercase()).unwrap_or_default();
        let nep = {
            let l = titel.to_lowercase();
            ["untitled", "naamloos", "document", "document1", "titel", "title", "microsoft word", "presentation"].contains(&l.as_str())
                || l.starts_with("microsoft word -") || l.ends_with(".docx") || l.ends_with(".doc") || l.ends_with(".pdf") || l == stam
        };
        if titel.is_empty() {
            v.punt(Ernst::Probleem, "CPdfGeenTitel", &[], Vec::new());
        } else if nep {
            v.punt(Ernst::Probleem, "CPdfNepTitel", &[&titel], Vec::new());
        } else {
            v.goed("CPdfTitel", &[&titel]);
        }
        if taal.is_empty() {
            v.punt(Ernst::Probleem, "CPdfGeenTaal", &[], Vec::new());
        } else {
            v.goed("CPdfTaal", &[&taal]);
        }
        // 5. Koppen, plaatjes, tabellen (alleen in een getagde PDF te zien).
        if getagd {
            if s.koppen.is_empty() && (aantal > 1 || tekens > 1500) {
                v.punt(Ernst::Probleem, "CPdfGeenKoppen", &[], Vec::new());
            } else if !s.koppen.is_empty() {
                v.goed("CPdfKoppen", &[&s.koppen.len().to_string()]);
                // Overgeslagen niveaus (H1 naar H3).
                let mut vorig = 0u8;
                let mut gaten = Vec::new();
                for (pag, n) in &s.koppen {
                    if *n > 0 && vorig > 0 && *n > vorig + 1 {
                        gaten.push(*pag);
                    }
                    if *n > 0 {
                        vorig = *n;
                    }
                }
                if !gaten.is_empty() {
                    v.punt(Ernst::LetOp, "CPdfKopNiveaus", &[], paginas(&gaten));
                }
            }
            if !s.plaatjes_zonder.is_empty() {
                v.punt(Ernst::Probleem, "CPdfPlaatjesZonder", &[&s.plaatjes_zonder.len().to_string(), &s.plaatjes.to_string()], paginas(&s.plaatjes_zonder));
            } else if s.plaatjes > 0 {
                v.goed("CPdfPlaatjesGoed", &[&s.plaatjes.to_string()]);
            }
            if !s.tabellen_zonder_kop.is_empty() {
                v.punt(Ernst::Probleem, "CPdfTabelZonderKop", &[&s.tabellen_zonder_kop.len().to_string()], paginas(&s.tabellen_zonder_kop));
            } else if s.tabellen > 0 {
                v.goed("CPdfTabellenGoed", &[&s.tabellen.to_string()]);
            }
        } else if !plaatjes_op.is_empty() {
            v.punt(Ernst::Probleem, "CPdfPlaatjesOngetagd", &[], paginas(&plaatjes_op));
        }
        // 6. Bladwijzers bij een lange PDF.
        if aantal >= 6 && !bladwijzers {
            v.punt(Ernst::LetOp, "CPdfGeenBladwijzers", &[&aantal.to_string()], Vec::new());
        } else if bladwijzers {
            v.goed("CPdfBladwijzers", &[]);
        }
        // 7. Invulvelden met een naam voor de schermlezer.
        if !velden.is_empty() {
            let zonder: Vec<String> = velden.iter().filter(|(_, _, tu)| tu.is_empty())
                .map(|(pag, n, _)| format!("{} ({})", n, crate::teksten::tf("ControlePagina", &[&pag.to_string()]))).collect();
            if zonder.is_empty() {
                v.goed("CPdfVeldenGoed", &[&velden.len().to_string()]);
            } else {
                v.punt(Ernst::Probleem, "CPdfVeldenZonder", &[&zonder.len().to_string(), &velden.len().to_string()], zonder);
            }
        }
        // 8. Beveiliging die de schermlezer tegenhoudt.
        if rechten != 0xFFFF_FFFF && rechten & 0x200 == 0 && rechten & 0x10 == 0 {
            v.punt(Ernst::Probleem, "CPdfBeveiligd", &[], Vec::new());
        }
    }
    Ok(v)
}
