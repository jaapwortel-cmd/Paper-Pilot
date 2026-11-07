// Paper Pilot - meer soorten documenten lezen: RTF, oud Word (.doc), OpenDocument (.odt, .ods, .odp),
// webpagina's (.html), Excel (.xlsx) en PowerPoint (.pptx). Alles zonder extra programma's.

use crate::lezen::{Bouwer, Document};
use std::io::Read;
use std::path::Path;

fn kapot() -> String {
    crate::teksten::t("LezenKapot")
}

/// Windows-1252 naar tekst (oude bestanden).
fn cp1252(b: u8) -> char {
    match b {
        0x80 => '€', 0x82 => '‚', 0x84 => '„', 0x85 => '…', 0x91 => '‘', 0x92 => '’', 0x93 => '“', 0x94 => '”',
        0x95 => '•', 0x96 => '–', 0x97 => '—', 0x99 => '™', b => b as char,
    }
}

// ------------------------------------------------------------------ RTF

pub fn rtf(pad: &Path, naam: &str) -> Result<Document, String> {
    let data = std::fs::read(pad).map_err(|e| e.to_string())?;
    if !data.starts_with(b"{\\rtf") {
        return Err(kapot());
    }
    // Groepen die geen gewone tekst zijn, worden overgeslagen.
    const OVERSLAAN: [&str; 14] = ["fonttbl", "colortbl", "stylesheet", "info", "pict", "header", "footer", "headerl", "headerr",
        "footerl", "footerr", "listtable", "listoverridetable", "themedata"];
    let mut b = Bouwer::nieuw(naam);
    let mut alinea = String::new();
    let mut stapel: Vec<(bool, usize)> = vec![(false, 1)]; // (overslaan, \ucN)
    let mut overslaan_uc = 0usize;
    let mut kop = false;
    let kopstijlen = rtf_kopstijlen(&data);
    let mut i = 0;
    while i < data.len() {
        let c = data[i];
        let (skip, uc) = *stapel.last().unwrap_or(&(false, 1));
        match c {
            b'{' => {
                stapel.push((skip, uc));
                i += 1;
                // {\* ...} is een uitbreiding die we niet kennen: overslaan.
                if data.get(i..i + 2) == Some(b"\\*") {
                    if let Some(s) = stapel.last_mut() {
                        s.0 = true;
                    }
                }
            }
            b'}' => {
                stapel.pop();
                if stapel.is_empty() {
                    break;
                }
                i += 1;
            }
            b'\\' => {
                i += 1;
                let Some(&n) = data.get(i) else { break };
                if n == b'\'' {
                    // \'hh: een teken in Windows-1252.
                    let hex = std::str::from_utf8(data.get(i + 1..i + 3).unwrap_or(b"3f")).unwrap_or("3f");
                    if !skip {
                        if overslaan_uc > 0 {
                            overslaan_uc -= 1;
                        } else {
                            alinea.push(cp1252(u8::from_str_radix(hex, 16).unwrap_or(b'?')));
                        }
                    }
                    i += 3;
                    continue;
                }
                if !n.is_ascii_alphabetic() {
                    // \\ \{ \} en ~ (vaste spatie), - (zacht afbreekstreepje)
                    if !skip {
                        match n {
                            b'\\' | b'{' | b'}' => alinea.push(n as char),
                            b'~' => alinea.push(' '),
                            _ => {}
                        }
                    }
                    i += 1;
                    continue;
                }
                let begin = i;
                while i < data.len() && data[i].is_ascii_alphabetic() {
                    i += 1;
                }
                let woord = std::str::from_utf8(&data[begin..i]).unwrap_or("");
                let nb = i;
                if i < data.len() && (data[i] == b'-' || data[i].is_ascii_digit()) {
                    i += 1;
                    while i < data.len() && data[i].is_ascii_digit() {
                        i += 1;
                    }
                }
                let getal: Option<i32> = std::str::from_utf8(&data[nb..i]).ok().and_then(|g| g.parse().ok());
                if i < data.len() && data[i] == b' ' {
                    i += 1;
                }
                if OVERSLAAN.contains(&woord) {
                    if let Some(s) = stapel.last_mut() {
                        s.0 = true;
                    }
                    continue;
                }
                if skip {
                    continue;
                }
                match woord {
                    "par" | "line" | "sect" | "page" => {
                        if woord == "line" {
                            alinea.push_str("\r\n");
                        } else if kop {
                            b.kop(alinea.trim());
                            alinea.clear();
                        } else {
                            b.alinea(&alinea);
                            alinea.clear();
                        }
                    }
                    "pard" => kop = false,
                    "outlinelevel" => kop = true,
                    "s" if getal.is_some_and(|g| kopstijlen.contains(&g)) => kop = true,
                    "tab" => alinea.push('\t'),
                    "cell" => alinea.push_str(" | "),
                    "row" => {
                        b.alinea(alinea.trim_end_matches(" | "));
                        alinea.clear();
                    }
                    "emdash" => alinea.push('—'),
                    "endash" => alinea.push('–'),
                    "bullet" => alinea.push('•'),
                    "lquote" => alinea.push('‘'),
                    "rquote" => alinea.push('’'),
                    "ldblquote" => alinea.push('“'),
                    "rdblquote" => alinea.push('”'),
                    "uc" => {
                        if let Some(s) = stapel.last_mut() {
                            s.1 = getal.unwrap_or(1).max(0) as usize;
                        }
                    }
                    "u" => {
                        let mut g = getal.unwrap_or(63);
                        if g < 0 {
                            g += 65536;
                        }
                        if let Some(ch) = char::from_u32(g as u32) {
                            alinea.push(ch);
                        }
                        overslaan_uc = uc;
                    }
                    _ => {}
                }
            }
            b'\r' | b'\n' => i += 1,
            _ => {
                if !skip {
                    if overslaan_uc > 0 {
                        overslaan_uc -= 1;
                    } else {
                        alinea.push(c as char);
                    }
                }
                i += 1;
            }
        }
    }
    b.alinea(&alinea);
    Ok(b.klaar())
}

/// De nummers van de stijlen die een kop zijn ("Heading 1", "Kop 2", "Titel" ...).
fn rtf_kopstijlen(data: &[u8]) -> Vec<i32> {
    let tekst = String::from_utf8_lossy(data);
    let Some(begin) = tekst.find("{\\stylesheet") else { return Vec::new() };
    // Het einde van de groep {\stylesheet ...} zoeken.
    let mut diepte = 0;
    let mut eind_groep = tekst.len();
    for (i, c) in tekst[begin..].char_indices() {
        match c {
            '{' => diepte += 1,
            '}' => {
                diepte -= 1;
                if diepte == 0 {
                    eind_groep = begin + i;
                    break;
                }
            }
            _ => {}
        }
    }
    let mut uit = Vec::new();
    let mut rest = &tekst[begin + 1..eind_groep];
    while let Some(p) = rest.find("{\\s") {
        rest = &rest[p + 3..];
        let nr: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        let Ok(nr) = nr.parse::<i32>() else { continue };
        let Some(eind) = rest.find(';') else { break };
        // De naam staat na het laatste opmaakwoord: "...\\af8 Heading 1".
        let deel = &rest[..eind];
        let na = deel.rsplit('\\').next().unwrap_or("");
        let naam = na.split_once(' ').map(|x| x.1).unwrap_or("").trim_start_matches('}').trim().to_lowercase();
        if naam.starts_with("heading") || naam.starts_with("kop") || naam == "title" || naam == "titel" {
            uit.push(nr);
        }
    }
    uit
}

// ------------------------------------------------------------------ oud Word (.doc, Word 97-2003)

pub fn doc(pad: &Path, naam: &str) -> Result<Document, String> {
    let mut cf = cfb::open(pad).map_err(|_| kapot())?;
    let mut wd = Vec::new();
    cf.open_stream("/WordDocument").map_err(|_| kapot())?.read_to_end(&mut wd).map_err(|e| e.to_string())?;
    if wd.len() < 0x1AA {
        return Err(kapot());
    }
    let u16_op = |b: &[u8], i: usize| u16::from_le_bytes([b[i], b[i + 1]]);
    let u32_op = |b: &[u8], i: usize| u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
    let vlaggen = u16_op(&wd, 0x0A);
    if vlaggen & 0x0100 != 0 {
        return Err(crate::teksten::t("LezenWachtwoord"));
    }
    let tabel = if vlaggen & 0x0200 != 0 { "/1Table" } else { "/0Table" };
    let mut tb = Vec::new();
    cf.open_stream(tabel).map_err(|_| kapot())?.read_to_end(&mut tb).map_err(|e| e.to_string())?;
    // Het stuk met de stukjestabel (CLX): plaats en lengte staan in de FIB.
    let fc_clx = u32_op(&wd, 0x01A2) as usize;
    let lcb_clx = u32_op(&wd, 0x01A6) as usize;
    if fc_clx + lcb_clx > tb.len() || lcb_clx == 0 {
        return Err(kapot());
    }
    let clx = &tb[fc_clx..fc_clx + lcb_clx];
    let mut i = 0;
    while i < clx.len() && clx[i] == 0x01 {
        i += 3 + u16_op(clx, i + 1) as usize; // Prc overslaan
    }
    if i >= clx.len() || clx[i] != 0x02 {
        return Err(kapot());
    }
    let lcb = u32_op(clx, i + 1) as usize;
    let plc = &clx[i + 5..(i + 5 + lcb).min(clx.len())];
    let n = (plc.len() - 4) / 12; // (n+1) posities van 4 bytes + n stukjes van 8 bytes
    let mut tekst = String::new();
    for k in 0..n {
        let (cp0, cp1) = (u32_op(plc, k * 4) as usize, u32_op(plc, (k + 1) * 4) as usize);
        let pcd = (n + 1) * 4 + k * 8;
        let fc = u32_op(plc, pcd + 2);
        let lengte = cp1.saturating_sub(cp0);
        if fc & 0x4000_0000 != 0 {
            let start = ((fc & 0x3FFF_FFFF) / 2) as usize;
            for &c in wd.get(start..start + lengte).unwrap_or(&[]) {
                tekst.push(cp1252(c));
            }
        } else {
            let start = fc as usize;
            let stuk: Vec<u16> = wd.get(start..start + lengte * 2).unwrap_or(&[]).chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
            tekst.push_str(&String::from_utf16_lossy(&stuk));
        }
    }
    // Opmaaktekens van Word: \r = einde alinea, \x07 = einde cel, \x0B = regeleinde, \x0C = pagina, \x13-\x15 = velden.
    let mut b = Bouwer::nieuw(naam);
    let mut alinea = String::new();
    let mut in_veldcode = 0;
    // Tabellen: elke cel eindigt met \x07, een rij met nog een \x07 direct erachter.
    let mut cellen: Vec<String> = Vec::new();
    let mut vorige = ' ';
    for c in tekst.chars() {
        let deze = c;
        match c {
            '\u{13}' => in_veldcode += 1,
            '\u{14}' => in_veldcode = 0,
            '\u{15}' => in_veldcode = 0,
            _ if in_veldcode > 0 => {}
            '\r' if !cellen.is_empty() => alinea.push(' '),
            '\r' | '\u{0C}' => {
                b.alinea(&alinea);
                alinea.clear();
            }
            '\u{07}' if vorige == '\u{07}' && alinea.trim().is_empty() => {
                let rij: Vec<String> = cellen.drain(..).filter(|c| !c.is_empty()).collect();
                b.alinea(&rij.join(" | "));
                alinea.clear();
            }
            '\u{07}' => cellen.push(std::mem::take(&mut alinea).trim().to_string()),
            '\u{0B}' => alinea.push_str("\r\n"),
            '\u{01}' | '\u{08}' | '\u{02}' | '\u{05}' => {}
            c => alinea.push(c),
        }
        vorige = deze;
    }
    if !cellen.is_empty() {
        b.alinea(&cellen.join(" | "));
    }
    b.alinea(&alinea);
    Ok(b.klaar())
}

// ------------------------------------------------------------------ XML in een zip (OpenDocument, Excel, PowerPoint)

fn zip_deel(pad: &Path, deel: &str) -> Result<String, String> {
    let f = std::fs::File::open(pad).map_err(|e| e.to_string())?;
    let mut z = zip::ZipArchive::new(f).map_err(|_| kapot())?;
    let mut s = String::new();
    z.by_name(deel).map_err(|_| kapot())?.read_to_string(&mut s).map_err(|e| e.to_string())?;
    Ok(s)
}

fn zip_namen(pad: &Path) -> Vec<String> {
    std::fs::File::open(pad).ok().and_then(|f| zip::ZipArchive::new(f).ok())
        .map(|z| z.file_names().map(|s| s.to_string()).collect()).unwrap_or_default()
}

fn lokaal(n: &[u8]) -> String {
    String::from_utf8_lossy(n).rsplit(':').next().unwrap_or("").to_string()
}

/// OpenDocument (.odt, .ods, .odp): text:h = kop, text:p = alinea, tabelrijen op één regel.
pub fn opendocument(pad: &Path, naam: &str) -> Result<Document, String> {
    use quick_xml::events::Event;
    let xml = zip_deel(pad, "content.xml")?;
    let mut r = quick_xml::Reader::from_str(&xml);
    let mut b = Bouwer::nieuw(naam);
    let mut alinea = String::new();
    let mut is_kop = false;
    let mut diepte = 0; // geneste alinea's (bv. in een cel)
    let mut cellen: Vec<String> = Vec::new();
    let mut in_rij = false;
    let mut blad = 0;
    let mut dia = 0;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => match lokaal(e.name().as_ref()).as_str() {
                "h" | "p" => {
                    diepte += 1;
                    is_kop = lokaal(e.name().as_ref()) == "h";
                }
                "table-row" => {
                    in_rij = true;
                    cellen.clear();
                }
                "table" => {
                    // Rekenblad (.ods): de naam van het blad als kop.
                    blad += 1;
                    for a in e.attributes().flatten() {
                        if lokaal(a.key.as_ref()) == "name" && naam.to_lowercase().ends_with(".ods") {
                            b.kop(&String::from_utf8_lossy(&a.value));
                        }
                    }
                }
                "page" => {
                    // Dia (.odp)
                    dia += 1;
                    b.kop(&crate::teksten::tf("Dia", &[&dia.to_string()]));
                }
                _ => {}
            },
            Ok(Event::Empty(e)) => match lokaal(e.name().as_ref()).as_str() {
                "s" => alinea.push(' '),
                "tab" => alinea.push('\t'),
                "line-break" => alinea.push_str("\r\n"),
                _ => {}
            },
            Ok(Event::Text(t)) if diepte > 0 => alinea.push_str(&t.unescape().map(|s| s.to_string()).unwrap_or_default()),
            Ok(Event::End(e)) => match lokaal(e.name().as_ref()).as_str() {
                "h" | "p" => {
                    diepte -= 1;
                    let tekst = alinea.trim().to_string();
                    alinea.clear();
                    if in_rij {
                        if !tekst.is_empty() {
                            cellen.push(tekst);
                        }
                    } else if is_kop && !tekst.is_empty() {
                        b.kop(&tekst);
                    } else {
                        b.alinea(&tekst);
                    }
                }
                "table-row" => {
                    in_rij = false;
                    if !cellen.is_empty() {
                        b.alinea(&cellen.join(" | "));
                    }
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(_) => return Err(kapot()),
            _ => {}
        }
    }
    let _ = blad;
    Ok(b.klaar())
}

/// Excel (.xlsx): per blad de rijen, cellen gescheiden door |.
pub fn xlsx(pad: &Path, naam: &str) -> Result<Document, String> {
    use quick_xml::events::Event;
    // Gedeelde teksten.
    let mut gedeeld: Vec<String> = Vec::new();
    if let Ok(xml) = zip_deel(pad, "xl/sharedStrings.xml") {
        let mut r = quick_xml::Reader::from_str(&xml);
        let mut huidig = String::new();
        let mut in_t = false;
        loop {
            match r.read_event() {
                Ok(Event::Start(e)) if lokaal(e.name().as_ref()) == "t" => in_t = true,
                Ok(Event::Text(t)) if in_t => huidig.push_str(&t.unescape().map(|s| s.to_string()).unwrap_or_default()),
                Ok(Event::End(e)) => match lokaal(e.name().as_ref()).as_str() {
                    "t" => in_t = false,
                    "si" => gedeeld.push(std::mem::take(&mut huidig)),
                    _ => {}
                },
                Ok(Event::Eof) | Err(_) => break,
                _ => {}
            }
        }
    }
    let bladen = xlsx_bladen(pad);
    if bladen.is_empty() {
        return Err(kapot());
    }
    let mut b = Bouwer::nieuw(naam);
    for (nr, (blad, bladnaam)) in bladen.iter().enumerate() {
        if bladen.len() > 1 {
            match bladnaam {
                Some(n) if !n.trim().is_empty() => b.kop(n.trim()),
                _ => b.kop(&crate::teksten::tf("Blad", &[&(nr + 1).to_string()])),
            }
        }
        let xml = zip_deel(pad, blad)?;
        let mut r = quick_xml::Reader::from_str(&xml);
        let mut cellen: Vec<String> = Vec::new();
        let mut soort = String::new();
        let mut waarde = String::new();
        let mut in_v = false;
        loop {
            match r.read_event() {
                Ok(Event::Start(e)) => match lokaal(e.name().as_ref()).as_str() {
                    "row" => cellen.clear(),
                    "c" => {
                        soort.clear();
                        waarde.clear();
                        for a in e.attributes().flatten() {
                            if lokaal(a.key.as_ref()) == "t" {
                                soort = String::from_utf8_lossy(&a.value).to_string();
                            }
                        }
                    }
                    "v" | "t" => in_v = true,
                    _ => {}
                },
                Ok(Event::Text(t)) if in_v => waarde.push_str(&t.unescape().map(|s| s.to_string()).unwrap_or_default()),
                Ok(Event::End(e)) => match lokaal(e.name().as_ref()).as_str() {
                    "v" | "t" => in_v = false,
                    "c" => {
                        let w = if soort == "s" { waarde.trim().parse::<usize>().ok().and_then(|i| gedeeld.get(i).cloned()).unwrap_or_default() } else { waarde.clone() };
                        if !w.trim().is_empty() {
                            cellen.push(w.trim().to_string());
                        }
                    }
                    "row" => {
                        if !cellen.is_empty() {
                            b.alinea(&cellen.join(" | "));
                        }
                    }
                    _ => {}
                },
                Ok(Event::Eof) | Err(_) => break,
                _ => {}
            }
        }
    }
    Ok(b.klaar())
}

/// De bladen van een Excel-bestand in de goede volgorde, met hun naam (uit workbook.xml).
fn xlsx_bladen(pad: &Path) -> Vec<(String, Option<String>)> {
    use quick_xml::events::Event;
    let attr = |e: &quick_xml::events::BytesStart, sleutel: &str| -> Option<String> {
        e.attributes().flatten().find(|a| String::from_utf8_lossy(a.key.as_ref()) == sleutel)
            .map(|a| a.unescape_value().map(|v| v.to_string()).unwrap_or_default())
    };
    // Id -> doelbestand
    let mut doelen: Vec<(String, String)> = Vec::new();
    if let Ok(xml) = zip_deel(pad, "xl/_rels/workbook.xml.rels") {
        let mut r = quick_xml::Reader::from_str(&xml);
        loop {
            match r.read_event() {
                Ok(Event::Empty(e)) | Ok(Event::Start(e)) if lokaal(e.name().as_ref()) == "Relationship" => {
                    if let (Some(id), Some(doel)) = (attr(&e, "Id"), attr(&e, "Target")) {
                        let doel = doel.trim_start_matches('/').to_string();
                        let doel = if doel.starts_with("xl/") { doel } else { format!("xl/{doel}") };
                        doelen.push((id, doel));
                    }
                }
                Ok(Event::Eof) | Err(_) => break,
                _ => {}
            }
        }
    }
    let mut uit = Vec::new();
    if let Ok(xml) = zip_deel(pad, "xl/workbook.xml") {
        let mut r = quick_xml::Reader::from_str(&xml);
        loop {
            match r.read_event() {
                Ok(Event::Empty(e)) | Ok(Event::Start(e)) if lokaal(e.name().as_ref()) == "sheet" => {
                    let id = attr(&e, "r:id").unwrap_or_default();
                    if let Some((_, doel)) = doelen.iter().find(|(i, _)| *i == id) {
                        uit.push((doel.clone(), attr(&e, "name")));
                    }
                }
                Ok(Event::Eof) | Err(_) => break,
                _ => {}
            }
        }
    }
    let namen = zip_namen(pad);
    uit.retain(|(d, _)| namen.contains(d));
    if uit.is_empty() {
        let mut bladen: Vec<String> = namen.into_iter().filter(|n| n.starts_with("xl/worksheets/sheet") && n.ends_with(".xml")).collect();
        bladen.sort_by_key(|n| n.trim_start_matches("xl/worksheets/sheet").trim_end_matches(".xml").parse::<u32>().unwrap_or(0));
        uit = bladen.into_iter().map(|b| (b, None)).collect();
    }
    uit
}

/// PowerPoint (.pptx): per dia de teksten.
pub fn pptx(pad: &Path, naam: &str) -> Result<Document, String> {
    use quick_xml::events::Event;
    let mut dias: Vec<String> = zip_namen(pad).into_iter().filter(|n| n.starts_with("ppt/slides/slide") && n.ends_with(".xml")).collect();
    dias.sort_by_key(|n| n.trim_start_matches("ppt/slides/slide").trim_end_matches(".xml").parse::<u32>().unwrap_or(0));
    if dias.is_empty() {
        return Err(kapot());
    }
    let mut b = Bouwer::nieuw(naam);
    for (nr, dia) in dias.iter().enumerate() {
        b.kop(&crate::teksten::tf("Dia", &[&(nr + 1).to_string()]));
        let xml = zip_deel(pad, dia)?;
        let mut r = quick_xml::Reader::from_str(&xml);
        let mut alinea = String::new();
        let mut in_t = false;
        loop {
            match r.read_event() {
                Ok(Event::Start(e)) if lokaal(e.name().as_ref()) == "t" => in_t = true,
                Ok(Event::Text(t)) if in_t => alinea.push_str(&t.unescape().map(|s| s.to_string()).unwrap_or_default()),
                Ok(Event::End(e)) => match lokaal(e.name().as_ref()).as_str() {
                    "t" => in_t = false,
                    "p" => {
                        b.alinea(&alinea);
                        alinea.clear();
                    }
                    _ => {}
                },
                Ok(Event::Eof) | Err(_) => break,
                _ => {}
            }
        }
    }
    Ok(b.klaar())
}

// ------------------------------------------------------------------ webpagina (.html)

pub fn html(pad: &Path, naam: &str) -> Result<Document, String> {
    let ruw = crate::lezen::lees_tekstbestand(pad)?;
    let mut b = Bouwer::nieuw(naam);
    html_naar(&mut b, &ruw);
    Ok(b.klaar())
}

/// Zet de tekst van een webpagina (of een hoofdstuk van een e-boek) in de Bouwer.
fn html_naar(b: &mut Bouwer, ruw: &str) {
    let tekens: Vec<char> = ruw.chars().collect();
    let mut alinea = String::new();
    let mut kop = false;
    let mut overslaan: Option<String> = None;
    // In een tabelrij: de cellen; in een cel komt alle tekst in `alinea` en telt een <p> niet als nieuwe alinea.
    let mut rij: Option<Vec<String>> = None;
    let mut in_cel = false;
    let schoon = |s: &str| -> String {
        s.split('\n').map(|r| r.split_whitespace().collect::<Vec<_>>().join(" ")).filter(|r| !r.is_empty()).collect::<Vec<_>>().join("\r\n")
    };
    let entiteit = |e: &str| -> Option<char> {
        Some(match e {
            "amp" => '&', "lt" => '<', "gt" => '>', "quot" => '"', "apos" => '\'', "nbsp" => ' ', "euro" => '€',
            "eacute" => 'é', "euml" => 'ë', "iuml" => 'ï', "ouml" => 'ö', "uuml" => 'ü', "egrave" => 'è', "agrave" => 'à',
            "ccedil" => 'ç', "ndash" => '–', "mdash" => '—', "hellip" => '…', "copy" => '©',
            e if e.starts_with("#x") || e.starts_with("#X") => char::from_u32(u32::from_str_radix(&e[2..], 16).ok()?)?,
            e if e.starts_with('#') => char::from_u32(e[1..].parse().ok()?)?,
            _ => return None,
        })
    };
    let mut i = 0;
    while i < tekens.len() {
        let c = tekens[i];
        if c == '<' {
            // Commentaar <!-- ... -->
            if tekens[i..].iter().take(4).collect::<String>() == "<!--" {
                let rest: String = tekens[i..].iter().collect();
                i += rest.find("-->").map(|p| rest[..p].chars().count() + 3).unwrap_or(tekens.len() - i);
                continue;
            }
            let eind = tekens[i..].iter().position(|&x| x == '>').map(|p| i + p).unwrap_or(tekens.len() - 1);
            let label: String = tekens[i + 1..eind].iter().collect::<String>().to_lowercase();
            let sluit = label.starts_with('/');
            let lnaam: String = label.trim_start_matches('/').chars().take_while(|c| c.is_alphanumeric()).collect();
            i = eind + 1;
            if let Some(o) = &overslaan {
                if sluit && &lnaam == o {
                    overslaan = None;
                }
                continue;
            }
            match lnaam.as_str() {
                "script" | "style" | "head" | "noscript" | "svg" | "template" if !sluit => overslaan = Some(lnaam),
                "h1" | "h2" | "h3" | "h4" | "h5" | "h6" if !in_cel => {
                    if sluit {
                        let t = schoon(&alinea).replace("\r\n", " ");
                        if !t.is_empty() {
                            b.kop(&t);
                        }
                        kop = false;
                    } else {
                        b.alinea(&schoon(&alinea));
                        kop = true;
                    }
                    alinea.clear();
                }
                "tr" => {
                    if let Some(mut cellen) = rij.take() {
                        if in_cel && !schoon(&alinea).is_empty() {
                            cellen.push(schoon(&alinea).replace("\r\n", " "));
                        }
                        if !cellen.is_empty() {
                            b.alinea(&cellen.join(" | "));
                        }
                    } else {
                        b.alinea(&schoon(&alinea));
                    }
                    alinea.clear();
                    in_cel = false;
                    if !sluit {
                        rij = Some(Vec::new());
                    }
                }
                "td" | "th" => {
                    if in_cel {
                        let t = schoon(&alinea).replace("\r\n", " ");
                        if !t.is_empty() {
                            rij.get_or_insert_with(Vec::new).push(t);
                        }
                    } else if rij.is_none() {
                        b.alinea(&schoon(&alinea));
                    }
                    alinea.clear();
                    in_cel = !sluit;
                }
                "table" if sluit => {
                    if let Some(mut cellen) = rij.take() {
                        let t = schoon(&alinea).replace("\r\n", " ");
                        if !t.is_empty() {
                            cellen.push(t);
                        }
                        if !cellen.is_empty() {
                            b.alinea(&cellen.join(" | "));
                        }
                        alinea.clear();
                    }
                    in_cel = false;
                }
                "p" | "div" | "li" | "section" | "article" | "table" | "ul" | "ol" | "blockquote" | "address" | "dt" | "dd" | "pre"
                    if !kop && !in_cel =>
                {
                    b.alinea(&schoon(&alinea));
                    alinea.clear();
                }
                "br" => alinea.push('\n'),
                "p" | "div" | "li" if in_cel => alinea.push(' '),
                _ => {}
            }
            continue;
        }
        if overslaan.is_some() {
            i += 1;
            continue;
        }
        if c == '&' {
            if let Some(p) = tekens[i..].iter().take(10).position(|&x| x == ';') {
                let e: String = tekens[i + 1..i + p].iter().collect();
                if let Some(ch) = entiteit(&e) {
                    alinea.push(ch);
                    i += p + 1;
                    continue;
                }
            }
        }
        alinea.push(if c.is_whitespace() { ' ' } else { c });
        i += 1;
    }
    b.alinea(&schoon(&alinea));
}

// ------------------------------------------------------------------ e-boek (.epub)

/// EPUB: een zip met XHTML-hoofdstukken. De volgorde staat in het OPF-bestand (spine).
pub fn epub(pad: &Path, naam: &str) -> Result<Document, String> {
    use quick_xml::events::Event;
    let namen = zip_namen(pad);
    if namen.iter().any(|n| n == "META-INF/encryption.xml") && zip_deel(pad, "META-INF/encryption.xml").map(|x| x.contains("EncryptedData")).unwrap_or(false) {
        // Met DRM beveiligd: dat kan Paper Pilot niet lezen.
        return Err(crate::teksten::t("LezenEpubBeveiligd"));
    }
    let attr = |e: &quick_xml::events::BytesStart, sleutel: &str| -> Option<String> {
        e.attributes().flatten().find(|a| lokaal(a.key.as_ref()) == sleutel)
            .map(|a| a.unescape_value().map(|v| v.to_string()).unwrap_or_default())
    };
    // 1. Waar staat het OPF-bestand?
    let container = zip_deel(pad, "META-INF/container.xml")?;
    let mut opf_pad = String::new();
    let mut r = quick_xml::Reader::from_str(&container);
    loop {
        match r.read_event() {
            Ok(Event::Empty(e)) | Ok(Event::Start(e)) if lokaal(e.name().as_ref()) == "rootfile" => {
                if let Some(p) = attr(&e, "full-path") {
                    opf_pad = p;
                    break;
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    if opf_pad.is_empty() {
        return Err(kapot());
    }
    let map = opf_pad.rsplit_once('/').map(|x| format!("{}/", x.0)).unwrap_or_default();
    // 2. Titel, inhoud (manifest) en volgorde (spine).
    let opf = zip_deel(pad, &opf_pad)?;
    let mut titel = String::new();
    let mut in_titel = false;
    let mut manifest: Vec<(String, String)> = Vec::new();
    let mut volgorde: Vec<String> = Vec::new();
    let mut r = quick_xml::Reader::from_str(&opf);
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if lokaal(e.name().as_ref()) == "title" && titel.is_empty() => in_titel = true,
            Ok(Event::Text(t)) if in_titel => titel.push_str(&t.unescape().map(|s| s.to_string()).unwrap_or_default()),
            Ok(Event::End(e)) if lokaal(e.name().as_ref()) == "title" => in_titel = false,
            Ok(Event::Empty(e)) | Ok(Event::Start(e)) => match lokaal(e.name().as_ref()).as_str() {
                "item" => {
                    if let (Some(id), Some(href)) = (attr(&e, "id"), attr(&e, "href")) {
                        manifest.push((id, href));
                    }
                }
                "itemref" => {
                    if let Some(id) = attr(&e, "idref") {
                        volgorde.push(id);
                    }
                }
                _ => {}
            },
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    let mut b = Bouwer::nieuw(naam);
    if !titel.trim().is_empty() && !["unknown title", "untitled", "onbekende titel"].contains(&titel.trim().to_lowercase().as_str()) {
        b.kop(titel.trim());
    }
    let mut gelezen = 0;
    for id in &volgorde {
        let Some((_, href)) = manifest.iter().find(|(i, _)| i == id) else { continue };
        let href = href.split('#').next().unwrap_or("");
        let href = href.replace("%20", " ");
        let deel = normaliseer(&format!("{map}{href}"));
        if let Ok(xhtml) = zip_deel(pad, &deel) {
            html_naar(&mut b, &xhtml);
            gelezen += 1;
        }
    }
    if gelezen == 0 {
        return Err(kapot());
    }
    Ok(b.klaar())
}

/// "OEBPS/tekst/../h1.xhtml" wordt "OEBPS/h1.xhtml".
fn normaliseer(pad: &str) -> String {
    let mut delen: Vec<&str> = Vec::new();
    for d in pad.split('/') {
        match d {
            "" | "." => {}
            ".." => {
                delen.pop();
            }
            d => delen.push(d),
        }
    }
    delen.join("/")
}
