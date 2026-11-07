// Paper Pilot - Mijn post: een lijst van de brieven en documenten die je hebt gelezen.
//
// Per document: de datum, de naam, de afzender (als Paper Pilot die weet), waar het bestand staat, en de tekst.
// Zo vind je een oude brief terug, ook een scan die je niet hebt opgeslagen. Alles staat versleuteld (DPAPI) in
// %APPDATA%\Paper Pilot\mijn-post.dat, alleen op deze computer. Uit te zetten in Instellingen; te wissen in de lijst.

use serde_json::{json, Value};

const BESTAND: &str = "mijn-post.dat";
/// Hooguit zoveel documenten; de oudste gaan eruit.
const MAX: usize = 300;

#[derive(Clone, Debug, Default)]
pub struct Stuk {
    /// jjjj-mm-dd uu:mm
    pub datum: String,
    pub naam: String,
    pub afzender: String,
    /// Leeg bij een scan.
    pub pad: String,
    pub tekst: String,
}

pub fn aan() -> bool {
    crate::instellingen::aan("mijn_post", true)
}

pub fn laad() -> Vec<Stuk> {
    let Some(data) = crate::gegevens::lees_versleuteld(BESTAND) else { return Vec::new() };
    let v: Value = serde_json::from_slice(&data).unwrap_or(Value::Null);
    v.as_array().map(|a| a.iter().map(|s| Stuk {
        datum: s["datum"].as_str().unwrap_or("").to_string(),
        naam: s["naam"].as_str().unwrap_or("").to_string(),
        afzender: s["afzender"].as_str().unwrap_or("").to_string(),
        pad: s["pad"].as_str().unwrap_or("").to_string(),
        tekst: s["tekst"].as_str().unwrap_or("").to_string(),
    }).collect()).unwrap_or_default()
}

fn bewaar(lijst: &[Stuk]) -> Result<(), String> {
    let v: Vec<Value> = lijst.iter().map(|s| json!({ "datum": s.datum, "naam": s.naam, "afzender": s.afzender, "pad": s.pad, "tekst": s.tekst })).collect();
    crate::gegevens::schrijf_versleuteld(BESTAND, Value::Array(v).to_string().as_bytes())
}

/// Van wie komt het? Bij een envelop de afzender, anders de eerste korte regel bovenaan (vaak de naam van de afzender).
pub fn afzender(doc: &crate::lezen::Document) -> String {
    let van = crate::teksten::t("EnvelopVan");
    let van = van.split("{0}").next().unwrap_or("").trim().to_string();
    let pagina = crate::teksten::t("PaginaVan");
    let pagina = pagina.split("{0}").next().unwrap_or("").trim().to_string();
    let envelop = crate::teksten::t("EnvelopKop");
    for regel in doc.tekst.lines() {
        let r = regel.trim();
        if r.is_empty() || (!pagina.is_empty() && r.starts_with(&pagina)) || r == envelop {
            continue;
        }
        if !van.is_empty() {
            if let Some(v) = r.strip_prefix(&van) {
                return v.trim().to_string();
            }
        }
        if r.chars().count() <= 60 && r.chars().any(|c| c.is_alphabetic()) {
            return r.to_string();
        }
        break;
    }
    String::new()
}

/// Zet een geopend of gescand document in Mijn post (vooraan). Hetzelfde bestand of dezelfde tekst komt er niet dubbel in.
pub fn onthoud(doc: &crate::lezen::Document, pad: &str) {
    if !aan() || doc.tekst.trim().is_empty() {
        return;
    }
    let mut lijst = laad();
    lijst.retain(|s| !(s.tekst == doc.tekst || (!pad.is_empty() && s.pad == pad)));
    lijst.insert(0, Stuk {
        datum: crate::logboek::nu()[..16].to_string(),
        naam: doc.naam.clone(),
        afzender: afzender(doc),
        pad: pad.to_string(),
        tekst: doc.tekst.clone(),
    });
    lijst.truncate(MAX);
    if let Err(e) = bewaar(&lijst) {
        crate::logboek::schrijf(&format!("Mijn post bewaren: {e}"));
    }
}

pub fn verwijder(index: usize) -> Result<(), String> {
    let mut lijst = laad();
    if index < lijst.len() {
        lijst.remove(index);
    }
    bewaar(&lijst)
}

pub fn wis() {
    let _ = std::fs::remove_file(crate::instellingen::map().join(BESTAND));
}

/// De regel in de lijst: "3 november 2026, 10.15 - Gemeente Voorbeeld - aanslag.pdf".
pub fn regel(s: &Stuk) -> String {
    let datum = crate::herinneringen::lees_iso(&s.datum[..10.min(s.datum.len())])
        .map(crate::herinneringen::mooi).unwrap_or_else(|| s.datum.clone());
    let tijd = s.datum.get(11..16).map(|t| t.replace(':', ".")).unwrap_or_default();
    let mut delen = vec![format!("{datum}, {tijd}")];
    if !s.afzender.is_empty() && s.afzender != s.naam {
        delen.push(s.afzender.clone());
    }
    delen.push(s.naam.clone());
    delen.join(" - ")
}
