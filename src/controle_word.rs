// Paper Pilot - Controleren: is dit Word-bestand (.docx) goed te lezen met een schermlezer?
//
// Uit de XML in het bestand: titel en taal, echte koppen (en tekst die er alleen uitziet als een kop), plaatjes met
// een beschrijving, tabellen met een kopregel, lege alinea's als opvulling, duidelijke linkteksten, kleurcontrast,
// kleine letters, lijstjes die met de hand zijn getypt en tekstvakken.

use crate::controleren::{contrast, getal, hex_kleur, Ernst, Verslag};
use crate::teksten::{t, tf};
use quick_xml::events::{BytesStart, Event};
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

fn deel(pad: &Path, naam: &str) -> Option<String> {
    let f = std::fs::File::open(pad).ok()?;
    let mut z = zip::ZipArchive::new(f).ok()?;
    let mut s = String::new();
    z.by_name(naam).ok()?.read_to_string(&mut s).ok()?;
    Some(s)
}

fn lokaal(n: &[u8]) -> String {
    String::from_utf8_lossy(n).rsplit(':').next().unwrap_or("").to_string()
}

fn attr(e: &BytesStart, sleutel: &str) -> Option<String> {
    e.attributes().flatten().find(|a| lokaal(a.key.as_ref()) == sleutel)
        .map(|a| a.unescape_value().map(|v| v.to_string()).unwrap_or_default())
}

fn kort(s: &str) -> String {
    let s = s.trim();
    if s.chars().count() > 50 { format!("\"{}…\"", s.chars().take(50).collect::<String>()) } else { format!("\"{s}\"") }
}

/// Een stijl uit styles.xml: is het een kop (en welk niveau), en de opmaak.
#[derive(Default, Clone)]
struct Stijl {
    kop: Option<u8>,
    gebaseerd_op: String,
    kleur: Option<String>,
    grootte: Option<u32>,
    vet: bool,
}

fn stijlen(pad: &Path) -> (HashMap<String, Stijl>, Option<String>, Option<u32>) {
    let mut uit = HashMap::new();
    let mut taal = None;
    let mut standaard_grootte = None;
    let Some(xml) = deel(pad, "word/styles.xml") else { return (uit, taal, standaard_grootte) };
    let mut r = quick_xml::Reader::from_str(&xml);
    let mut huidig: Option<(String, Stijl)> = None;
    let mut in_standaard = false;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let n = lokaal(e.name().as_ref());
                match n.as_str() {
                    "docDefaults" => in_standaard = true,
                    "style" => {
                        if let Some(id) = attr(&e, "styleId") {
                            huidig = Some((id, Stijl::default()));
                        }
                    }
                    "name" => {
                        if let (Some((_, s)), Some(naam)) = (huidig.as_mut(), attr(&e, "val")) {
                            let l = naam.to_lowercase();
                            for voor in ["heading ", "kop "] {
                                if let Some(rest) = l.strip_prefix(voor) {
                                    s.kop = rest.trim().parse::<u8>().ok().filter(|n| (1..=9).contains(n));
                                }
                            }
                            if l == "title" || l == "titel" {
                                s.kop = Some(1);
                            }
                        }
                    }
                    "basedOn" => {
                        if let (Some((_, s)), Some(v)) = (huidig.as_mut(), attr(&e, "val")) {
                            s.gebaseerd_op = v;
                        }
                    }
                    "outlineLvl" => {
                        if let (Some((_, s)), Some(v)) = (huidig.as_mut(), attr(&e, "val")) {
                            if s.kop.is_none() {
                                s.kop = v.parse::<u8>().ok().filter(|n| *n < 9).map(|n| n + 1);
                            }
                        }
                    }
                    "color" => {
                        if let (Some((_, s)), Some(v)) = (huidig.as_mut(), attr(&e, "val")) {
                            s.kleur = Some(v);
                        }
                    }
                    "sz" => {
                        let g = attr(&e, "val").and_then(|v| v.parse::<u32>().ok());
                        if let Some((_, s)) = huidig.as_mut() {
                            s.grootte = g;
                        } else if in_standaard {
                            standaard_grootte = g;
                        }
                    }
                    "b" => {
                        if let Some((_, s)) = huidig.as_mut() {
                            s.vet = attr(&e, "val").map(|v| v != "0" && v != "false").unwrap_or(true);
                        }
                    }
                    "lang" if in_standaard => taal = attr(&e, "val"),
                    _ => {}
                }
            }
            Ok(Event::End(e)) => match lokaal(e.name().as_ref()).as_str() {
                "style" => {
                    if let Some((id, s)) = huidig.take() {
                        uit.insert(id, s);
                    }
                }
                "docDefaults" => in_standaard = false,
                _ => {}
            },
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    (uit, taal, standaard_grootte)
}

/// Opmaak van een stijl, met wat hij erft van de stijl waarop hij is gebaseerd.
fn opmaak(stijlen: &HashMap<String, Stijl>, id: &str) -> Stijl {
    let mut s = Stijl::default();
    let mut huidig = id.to_string();
    for _ in 0..10 {
        let Some(st) = stijlen.get(&huidig) else { break };
        if s.kop.is_none() { s.kop = st.kop; }
        if s.kleur.is_none() { s.kleur = st.kleur.clone(); }
        if s.grootte.is_none() { s.grootte = st.grootte; }
        s.vet |= st.vet;
        if st.gebaseerd_op.is_empty() { break; }
        huidig = st.gebaseerd_op.clone();
    }
    s
}

pub fn controleer(pad: &Path) -> Result<Verslag, String> {
    let naam = pad.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let mut v = Verslag::nieuw(&naam);
    let doc = deel(pad, "word/document.xml").ok_or_else(|| t("LezenKapot"))?;
    let (stijlen, taal_stijl, standaard_grootte) = stijlen(pad);
    // Titel en taal.
    let titel = deel(pad, "docProps/core.xml").and_then(|x| {
        let mut r = quick_xml::Reader::from_str(&x);
        let mut in_titel = false;
        let mut t = String::new();
        loop {
            match r.read_event() {
                Ok(Event::Start(e)) if lokaal(e.name().as_ref()) == "title" => in_titel = true,
                Ok(Event::Text(x)) if in_titel => t.push_str(&x.unescape().map(|s| s.to_string()).unwrap_or_default()),
                Ok(Event::End(e)) if lokaal(e.name().as_ref()) == "title" => in_titel = false,
                Ok(Event::Eof) | Err(_) => break,
                _ => {}
            }
        }
        Some(t.trim().to_string()).filter(|t| !t.is_empty())
    });
    let taal = taal_stijl.or_else(|| deel(pad, "word/settings.xml").and_then(|x| {
        let i = x.find("themeFontLang")?;
        let rest = &x[i..];
        let j = rest.find("w:val=\"")? + 7;
        Some(rest[j..].split('"').next()?.to_string())
    }));

    // Door het document lopen.
    #[derive(Default)]
    struct Alinea {
        tekst: String,
        stijl: String,
        vet_tekens: usize,
        tekens: usize,
        grootte: Option<u32>,
        lijst: bool,
        uitgevuld: bool,
    }
    let mut r = quick_xml::Reader::from_str(&doc);
    let mut alinea: Option<Alinea> = None;
    let mut koppen: Vec<(u8, String)> = Vec::new();
    let mut nep_koppen: Vec<String> = Vec::new();
    let mut handlijsten: Vec<String> = Vec::new();
    let mut leeg_op_rij = 0usize;
    let mut lege_stukken = 0usize;
    let mut alineas = 0usize;
    let mut plaatjes = 0usize;
    let mut plaatjes_zonder: Vec<String> = Vec::new();
    let mut decoratief = false;
    let mut laatste_plaatje: Option<(String, bool)> = None;
    let mut tabel_diepte = 0usize;
    let mut tabellen = 0usize;
    let mut tabellen_zonder_kop = 0usize;
    let mut geneste_tabellen = 0usize;
    let mut eerste_rij = false;
    let mut rij_kop = false;
    let mut samengevoegd = 0usize;
    let mut in_link = false;
    let mut linktekst = String::new();
    let mut vage_links: Vec<String> = Vec::new();
    let mut links = 0usize;
    let mut tekstvakken = 0usize;
    let mut laag_contrast: Vec<String> = Vec::new();
    let mut klein: Vec<String> = Vec::new();
    let mut uitgevuld = 0usize;
    let mut hoofdletters: Vec<String> = Vec::new();
    let mut run_caps = false;
    // Opmaak van het stukje tekst (w:r) dat nu loopt.
    let mut run_vet = false;
    let mut run_kleur: Option<String> = None;
    let mut run_grootte: Option<u32> = None;
    let mut run_achter: Option<String> = None;
    let mut in_run_opmaak = false;
    let mut in_tekst = false;
    let mut run_tekst = String::new();
    loop {
        match r.read_event() {
            // Een lege alinea (<w:p/>).
            Ok(Event::Empty(e)) if lokaal(e.name().as_ref()) == "p" => {
                alineas += 1;
                leeg_op_rij += 1;
                if leeg_op_rij == 3 {
                    lege_stukken += 1;
                }
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let n = lokaal(e.name().as_ref());
                match n.as_str() {
                    "p" => {
                        alinea = Some(Alinea::default());
                    }
                    "pStyle" => {
                        if let (Some(a), Some(s)) = (alinea.as_mut(), attr(&e, "val")) {
                            a.stijl = s;
                        }
                    }
                    "jc" if !in_run_opmaak => {
                        if let (Some(a), Some(v)) = (alinea.as_mut(), attr(&e, "val")) {
                            a.uitgevuld = v == "both" || v == "distribute";
                        }
                    }
                    "numPr" => {
                        if let Some(a) = alinea.as_mut() {
                            a.lijst = true;
                        }
                    }
                    "r" => {
                        run_vet = false;
                        run_kleur = None;
                        run_grootte = None;
                        run_achter = None;
                        run_caps = false;
                        run_tekst.clear();
                    }
                    "rPr" => in_run_opmaak = true,
                    "caps" if in_run_opmaak => run_caps = attr(&e, "val").map(|v| v != "0" && v != "false").unwrap_or(true),
                    "b" if in_run_opmaak => run_vet = attr(&e, "val").map(|v| v != "0" && v != "false").unwrap_or(true),
                    "color" if in_run_opmaak => run_kleur = attr(&e, "val"),
                    "sz" if in_run_opmaak => run_grootte = attr(&e, "val").and_then(|v| v.parse().ok()),
                    "shd" if in_run_opmaak => run_achter = attr(&e, "fill").filter(|f| f != "auto"),
                    "highlight" if in_run_opmaak => run_achter = attr(&e, "val").and_then(|v| match v.as_str() {
                        "yellow" => Some("FFFF00".to_string()), "green" => Some("00FF00".to_string()), "cyan" => Some("00FFFF".to_string()),
                        "black" => Some("000000".to_string()), "darkBlue" => Some("000080".to_string()), "lightGray" => Some("C0C0C0".to_string()),
                        _ => None,
                    }),
                    "t" => in_tekst = true,
                    "tab" => run_tekst.push('\t'),
                    "docPr" => {
                        plaatjes += 1;
                        let beschrijving = attr(&e, "descr").unwrap_or_default();
                        let titel = attr(&e, "title").unwrap_or_default();
                        let naam = attr(&e, "name").unwrap_or_default();
                        decoratief = false;
                        laatste_plaatje = Some((naam, !beschrijving.trim().is_empty() || !titel.trim().is_empty()));
                    }
                    "decorative" => {
                        if attr(&e, "val").map(|v| v == "1" || v == "true").unwrap_or(true) {
                            decoratief = true;
                        }
                    }
                    "tbl" => {
                        tabel_diepte += 1;
                        if tabel_diepte > 1 {
                            geneste_tabellen += 1;
                        } else {
                            tabellen += 1;
                            eerste_rij = true;
                            rij_kop = false;
                        }
                    }
                    "tblHeader" => rij_kop = true,
                    "vMerge" | "gridSpan" => samengevoegd += 1,
                    "hyperlink" => {
                        in_link = true;
                        linktekst.clear();
                        links += 1;
                    }
                    "txbxContent" | "textbox" => tekstvakken += 1,
                    _ => {}
                }
            }
            Ok(Event::Text(x)) if in_tekst => {
                let s = x.unescape().map(|s| s.to_string()).unwrap_or_default();
                run_tekst.push_str(&s);
            }
            Ok(Event::End(e)) => {
                let n = lokaal(e.name().as_ref());
                match n.as_str() {
                    "t" => in_tekst = false,
                    "rPr" => in_run_opmaak = false,
                    "r" => {
                        let stijl = alinea.as_ref().map(|a| opmaak(&stijlen, &a.stijl)).unwrap_or_default();
                        let tekens = run_tekst.trim().chars().count();
                        if tekens > 0 {
                            // Contrast tegen de achtergrond (wit als er geen is).
                            let kleur = run_kleur.clone().or(stijl.kleur.clone()).filter(|k| k != "auto");
                            if let Some(k) = kleur.as_deref().and_then(hex_kleur) {
                                let achter = run_achter.as_deref().and_then(hex_kleur).unwrap_or((255, 255, 255));
                                let c = contrast(k, achter);
                                let grootte = run_grootte.or(stijl.grootte).or(standaard_grootte).unwrap_or(22);
                                let groot = grootte >= 36 || (grootte >= 28 && (run_vet || stijl.vet));
                                if c < if groot { 3.0 } else { 4.5 } && laag_contrast.len() < 40 {
                                    laag_contrast.push(format!("{} ({} {})", kort(&run_tekst), t("ControleContrast"), getal(c)));
                                }
                            }
                            if let Some(g) = run_grootte.or(stijl.grootte).or(standaard_grootte) {
                                if g < 18 && klein.len() < 40 {
                                    klein.push(format!("{} ({} pt)", kort(&run_tekst), g / 2));
                                }
                            }
                            // Hele zinnen in hoofdletters lezen moeilijk (en schermlezers spellen ze soms).
                            let letters: Vec<char> = run_tekst.chars().filter(|c| c.is_alphabetic()).collect();
                            let alles_groot = letters.len() >= 15 && letters.iter().all(|c| c.is_uppercase()) && run_tekst.split_whitespace().count() >= 3;
                            if (run_caps && run_tekst.split_whitespace().count() >= 3) || alles_groot {
                                if hoofdletters.len() < 40 {
                                    hoofdletters.push(kort(&run_tekst));
                                }
                            }
                            if let Some(a) = alinea.as_mut() {
                                a.tekens += tekens;
                                if run_vet || stijl.vet {
                                    a.vet_tekens += tekens;
                                }
                                if let Some(g) = run_grootte {
                                    a.grootte = Some(a.grootte.unwrap_or(0).max(g));
                                }
                            }
                        }
                        if let Some(a) = alinea.as_mut() {
                            a.tekst.push_str(&run_tekst);
                        }
                        if in_link {
                            linktekst.push_str(&run_tekst);
                        }
                    }
                    "hyperlink" => {
                        in_link = false;
                        let l = linktekst.trim().to_lowercase();
                        if ["klik hier", "hier", "click here", "here", "lees meer", "meer", "read more", "more", "link", "deze link"].contains(&l.as_str())
                            || l.starts_with("http") || l.starts_with("www.")
                        {
                            vage_links.push(kort(&linktekst));
                        }
                    }
                    "docPr" | "inline" | "anchor" => {
                        if let Some((naam, beschreven)) = laatste_plaatje.take() {
                            if !beschreven && !decoratief {
                                plaatjes_zonder.push(if naam.is_empty() { t("ControleNaamloos") } else { naam });
                            }
                        }
                    }
                    "tr" if tabel_diepte == 1 && eerste_rij => {
                        eerste_rij = false;
                        if !rij_kop {
                            tabellen_zonder_kop += 1;
                        }
                    }
                    "tbl" => tabel_diepte = tabel_diepte.saturating_sub(1),
                    "p" => {
                        if let Some(a) = alinea.take() {
                            alineas += 1;
                            let tekst = a.tekst.trim().to_string();
                            if tekst.is_empty() {
                                leeg_op_rij += 1;
                                if leeg_op_rij == 3 {
                                    lege_stukken += 1;
                                }
                                continue;
                            }
                            leeg_op_rij = 0;
                            if a.uitgevuld && tekst.chars().count() > 80 {
                                uitgevuld += 1;
                            }
                            let stijl = opmaak(&stijlen, &a.stijl);
                            if let Some(niveau) = stijl.kop {
                                koppen.push((niveau, tekst.clone()));
                            } else if tabel_diepte == 0 && tekst.chars().count() <= 80 && !tekst.ends_with('.') && !tekst.ends_with(':')
                                && ((a.tekens > 0 && a.vet_tekens * 10 >= a.tekens * 9) || a.grootte.unwrap_or(0) >= 28)
                            {
                                nep_koppen.push(kort(&tekst));
                            }
                            if !a.lijst && tabel_diepte == 0 {
                                let begin: String = tekst.chars().take(4).collect();
                                let met_hand = begin.starts_with("- ") || begin.starts_with("• ") || begin.starts_with("* ") || begin.starts_with("– ")
                                    || {
                                        let cijfers: String = begin.chars().take_while(|c| c.is_ascii_digit()).collect();
                                        !cijfers.is_empty() && cijfers.len() <= 2 && (begin[cijfers.len()..].starts_with(". ") || begin[cijfers.len()..].starts_with(") "))
                                    };
                                if met_hand {
                                    handlijsten.push(kort(&tekst));
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => return Err(t("LezenKapot")),
            _ => {}
        }
    }

    // Het verslag.
    match &titel {
        Some(t) => v.goed("CWordTitel", &[t]),
        None => v.punt(Ernst::Probleem, "CWordGeenTitel", &[], Vec::new()),
    }
    match &taal {
        Some(t) => v.goed("CWordTaal", &[t]),
        None => v.punt(Ernst::LetOp, "CWordGeenTaal", &[], Vec::new()),
    }
    if koppen.is_empty() && alineas > 8 {
        v.punt(Ernst::Probleem, "CWordGeenKoppen", &[], Vec::new());
    } else if !koppen.is_empty() {
        v.goed("CWordKoppen", &[&koppen.len().to_string()]);
        let mut gaten = Vec::new();
        let mut vorig = 0u8;
        for (n, tekst) in &koppen {
            if vorig > 0 && *n > vorig + 1 {
                gaten.push(format!("{} ({} {n})", kort(tekst), t("ControleNiveau")));
            }
            vorig = *n;
        }
        if !gaten.is_empty() {
            v.punt(Ernst::LetOp, "CWordKopNiveaus", &[], gaten);
        }
    }
    if !nep_koppen.is_empty() {
        v.punt(Ernst::Probleem, "CWordNepKoppen", &[&nep_koppen.len().to_string()], nep_koppen);
    }
    if !plaatjes_zonder.is_empty() {
        v.punt(Ernst::Probleem, "CWordPlaatjesZonder", &[&plaatjes_zonder.len().to_string(), &plaatjes.to_string()], plaatjes_zonder);
    } else if plaatjes > 0 {
        v.goed("CWordPlaatjesGoed", &[&plaatjes.to_string()]);
    }
    if tabellen_zonder_kop > 0 {
        v.punt(Ernst::Probleem, "CWordTabelZonderKop", &[&tabellen_zonder_kop.to_string(), &tabellen.to_string()], Vec::new());
    } else if tabellen > 0 {
        v.goed("CWordTabellenGoed", &[&tabellen.to_string()]);
    }
    if geneste_tabellen > 0 || samengevoegd > 0 {
        v.punt(Ernst::LetOp, "CWordTabelIngewikkeld", &[], Vec::new());
    }
    if !laag_contrast.is_empty() {
        v.punt(Ernst::Probleem, "CWordContrast", &[&laag_contrast.len().to_string()], laag_contrast);
    } else {
        v.goed("CWordContrastGoed", &[]);
    }
    if !klein.is_empty() {
        v.punt(Ernst::LetOp, "CWordKlein", &[&klein.len().to_string()], klein);
    }
    if uitgevuld > 0 {
        v.punt(Ernst::LetOp, "CWordUitgevuld", &[&uitgevuld.to_string()], Vec::new());
    }
    if !hoofdletters.is_empty() {
        v.punt(Ernst::LetOp, "CWordHoofdletters", &[&hoofdletters.len().to_string()], hoofdletters);
    }
    if lege_stukken > 0 {
        v.punt(Ernst::LetOp, "CWordLegeAlineas", &[&lege_stukken.to_string()], Vec::new());
    }
    if !vage_links.is_empty() {
        v.punt(Ernst::LetOp, "CWordVageLinks", &[&vage_links.len().to_string()], vage_links);
    } else if links > 0 {
        v.goed("CWordLinksGoed", &[&links.to_string()]);
    }
    if !handlijsten.is_empty() {
        v.punt(Ernst::LetOp, "CWordHandLijst", &[&handlijsten.len().to_string()], handlijsten);
    }
    if tekstvakken > 0 {
        v.punt(Ernst::LetOp, "CWordTekstvakken", &[&tekstvakken.to_string()], Vec::new());
    }
    v.opmerkingen.push(tf("CWordOokWord", &[]));
    Ok(v)
}
