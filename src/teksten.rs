// Paper Pilot - alle teksten in het Nederlands en Engels.
// De teksten staan in teksten/nl.txt en teksten/en.txt ("Sleutel = tekst", \n is een nieuwe regel).
// De handleiding, privacyverklaring en wijzigingen staan in losse bestanden per taal.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

static NL: &str = include_str!("../teksten/nl.txt");
static EN: &str = include_str!("../teksten/en.txt");

static ENGELS: AtomicBool = AtomicBool::new(false);

fn lees(bron: &'static str) -> HashMap<&'static str, String> {
    let mut m = HashMap::new();
    for regel in bron.trim_start_matches('\u{feff}').lines() {
        let regel = regel.trim_end();
        if regel.is_empty() || regel.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = regel.split_once(" = ") {
            m.insert(k.trim(), v.replace("\\n", "\n").replace("\\t", "\t"));
        }
    }
    m
}

pub fn nl() -> &'static HashMap<&'static str, String> {
    static M: OnceLock<HashMap<&'static str, String>> = OnceLock::new();
    M.get_or_init(|| lees(NL))
}

pub fn en() -> &'static HashMap<&'static str, String> {
    static M: OnceLock<HashMap<&'static str, String>> = OnceLock::new();
    M.get_or_init(|| lees(EN))
}

pub fn zet_taal(taal: &str) {
    ENGELS.store(taal == "en", Ordering::Relaxed);
}

pub fn taal() -> &'static str {
    if ENGELS.load(Ordering::Relaxed) { "en" } else { "nl" }
}

/// De tekst bij een sleutel in de gekozen taal. Ontbreekt hij, dan Nederlands, en anders de sleutel zelf.
pub fn t(sleutel: &str) -> String {
    let gekozen = if ENGELS.load(Ordering::Relaxed) { en() } else { nl() };
    gekozen.get(sleutel).or_else(|| nl().get(sleutel)).cloned().unwrap_or_else(|| sleutel.to_string())
}

/// Tekst met {0}, {1} ... ingevuld.
pub fn tf(sleutel: &str, waarden: &[&str]) -> String {
    let mut s = t(sleutel);
    for (i, v) in waarden.iter().enumerate() {
        s = s.replace(&format!("{{{i}}}"), v);
    }
    s
}

/// Grote teksten (handleiding, privacy, wijzigingen) in de gekozen taal.
pub fn document(naam: &str) -> &'static str {
    let engels = ENGELS.load(Ordering::Relaxed);
    let s = match (naam, engels) {
        ("handleiding", false) => include_str!("../teksten/handleiding-nl.txt"),
        ("handleiding", true) => include_str!("../teksten/handleiding-en.txt"),
        ("privacy", false) => include_str!("../teksten/privacy-nl.txt"),
        ("privacy", true) => include_str!("../teksten/privacy-en.txt"),
        ("wijzigingen", false) => include_str!("../teksten/wijzigingen-nl.txt"),
        ("wijzigingen", true) => include_str!("../teksten/wijzigingen-en.txt"),
        _ => "",
    };
    s.trim_start_matches('\u{feff}')
}

/// Versienummer als vier getallen, om te vergelijken.
pub fn versie_getallen(s: &str) -> Option<[u32; 4]> {
    let delen: Vec<u32> = s.trim().split('.').map(|d| d.trim().parse().ok()).collect::<Option<_>>()?;
    if delen.is_empty() || delen.len() > 4 {
        return None;
    }
    let mut v = [0u32; 4];
    v[..delen.len()].copy_from_slice(&delen);
    Some(v)
}

/// De wijzigingen die nieuwer zijn dan `sinds` (bij een update). Een blok begint met "Versie x.x.x.x".
pub fn wijzigingen_sinds(sinds: &str) -> String {
    let oud = versie_getallen(sinds).unwrap_or([0; 4]);
    let mut uit = String::new();
    let mut meenemen = false;
    for regel in document("wijzigingen").lines() {
        let woorden: Vec<&str> = regel.split_whitespace().collect();
        if woorden.len() >= 2 && (woorden[0] == "Versie" || woorden[0] == "Version") {
            meenemen = versie_getallen(woorden[1]).map(|v| v > oud).unwrap_or(false);
        }
        if meenemen {
            uit.push_str(regel);
            uit.push('\n');
        }
    }
    uit.trim().to_string()
}
