// Paper Pilot - Mijn gegevens: naam, adres, geboortedatum, BSN en dergelijke, om formulieren sneller in te vullen.
//
// Versleuteld bewaard met DPAPI van Windows (CryptProtectData) in %APPDATA%\Paper Pilot\mijn-gegevens.dat:
// alleen de eigen Windows-gebruiker kan ze lezen. Ze gaan nooit naar internet.

use std::collections::BTreeMap;
use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Security::Cryptography::{CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB};

/// De soorten gegevens, in de volgorde van het venster Mijn gegevens.
pub const SOORTEN: [&str; 11] = [
    "voorletters", "voornaam", "achternaam", "straat", "postcode", "woonplaats", "geboortedatum", "telefoon", "email", "bsn", "iban",
];

fn blob(data: &[u8]) -> CRYPT_INTEGER_BLOB {
    CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 }
}

/// Versleutelt met DPAPI: alleen de eigen Windows-gebruiker kan het weer lezen.
pub fn versleutel(data: &[u8]) -> Result<Vec<u8>, String> {
    unsafe {
        let mut uit: CRYPT_INTEGER_BLOB = std::mem::zeroed();
        let naam = crate::win::w("Paper Pilot");
        if CryptProtectData(&blob(data), naam.as_ptr(), std::ptr::null(), std::ptr::null(), std::ptr::null(), 0, &mut uit) == 0 {
            return Err(crate::teksten::t("GegevensNietBewaard"));
        }
        let v = std::slice::from_raw_parts(uit.pbData, uit.cbData as usize).to_vec();
        LocalFree(uit.pbData as _);
        Ok(v)
    }
}

pub fn ontsleutel(data: &[u8]) -> Option<Vec<u8>> {
    unsafe {
        let mut leeg: CRYPT_INTEGER_BLOB = std::mem::zeroed();
        if CryptUnprotectData(&blob(data), std::ptr::null_mut(), std::ptr::null(), std::ptr::null(), std::ptr::null(), 0, &mut leeg) == 0 {
            return None;
        }
        let v = std::slice::from_raw_parts(leeg.pbData, leeg.cbData as usize).to_vec();
        LocalFree(leeg.pbData as _);
        Some(v)
    }
}

/// Leest een versleuteld bestand in %APPDATA%\Paper Pilot (leeg als het er niet is of niet te lezen is).
pub fn lees_versleuteld(naam: &str) -> Option<Vec<u8>> {
    let data = std::fs::read(crate::instellingen::map().join(naam)).ok()?;
    let uit = ontsleutel(&data);
    if uit.is_none() {
        crate::logboek::schrijf(&format!("{naam} kon niet worden gelezen."));
    }
    uit
}

/// Schrijft een versleuteld bestand in %APPDATA%\Paper Pilot (eerst naar .nieuw, dan vervangen).
pub fn schrijf_versleuteld(naam: &str, data: &[u8]) -> Result<(), String> {
    let v = versleutel(data)?;
    let map = crate::instellingen::map();
    let _ = std::fs::create_dir_all(&map);
    let nieuw = map.join(format!("{naam}.nieuw"));
    std::fs::write(&nieuw, v).map_err(|e| e.to_string())?;
    std::fs::rename(&nieuw, map.join(naam)).map_err(|e| e.to_string())
}

pub fn laad() -> BTreeMap<String, String> {
    let mut uit = BTreeMap::new();
    let Some(data) = lees_versleuteld("mijn-gegevens.dat") else { return uit };
    let tekst = String::from_utf8_lossy(&data).to_string();
    for regel in tekst.lines() {
        if let Some((k, v)) = regel.split_once('=') {
            uit.insert(k.to_string(), v.to_string());
        }
    }
    uit
}

pub fn bewaar(g: &BTreeMap<String, String>) -> Result<(), String> {
    let tekst: String = g.iter().filter(|(_, v)| !v.trim().is_empty()).map(|(k, v)| format!("{k}={}\n", v.replace('\n', " "))).collect();
    schrijf_versleuteld("mijn-gegevens.dat", tekst.as_bytes())
}

/// Bij welke soort gegevens hoort deze vraag? (Op woorden in de vraag.)
pub fn soort_van(vraag: &str) -> Option<&'static str> {
    let v = vraag.to_lowercase();
    let heeft = |w: &str| v.contains(w);
    if heeft("handtekening") || heeft("partner") || heeft("kind") || heeft("contactpersoon") || heeft("zorgverlener") {
        return None;
    }
    if heeft("geboortedatum") || heeft("geboren op") {
        return Some("geboortedatum");
    }
    if heeft("burgerservicenummer") || heeft("bsn") || heeft("sofinummer") {
        return Some("bsn");
    }
    if heeft("iban") || heeft("rekeningnummer") || heeft("bankrekening") {
        return Some("iban");
    }
    if heeft("e-mail") || heeft("email") || heeft("e-mailadres") {
        return Some("email");
    }
    if heeft("telefoon") || heeft("mobiel") || heeft("phone") {
        return Some("telefoon");
    }
    if heeft("postcode") && (heeft("plaats") || heeft("woonplaats")) {
        return Some("postcode_plaats");
    }
    if heeft("postcode") || heeft("zip") {
        return Some("postcode");
    }
    if heeft("woonplaats") || v.trim() == "plaats" || heeft("city") {
        return Some("woonplaats");
    }
    if heeft("straat") || heeft("adres") || heeft("address") {
        return Some("straat");
    }
    if heeft("voorletters") || heeft("initialen") {
        return Some("voorletters");
    }
    if heeft("voornaam") || heeft("voornamen") || heeft("first name") {
        return Some("voornaam");
    }
    if heeft("achternaam") || heeft("geslachtsnaam") || heeft("familienaam") || heeft("last name") || heeft("surname") {
        return Some("achternaam");
    }
    if heeft("naam") || heeft("name") {
        return Some("naam");
    }
    if heeft("datum") || heeft("date") {
        return Some("datum");
    }
    None
}

/// De waarde uit Mijn gegevens voor deze soort (met samengestelde soorten zoals "naam").
pub fn waarde(g: &BTreeMap<String, String>, soort: &str) -> Option<String> {
    let k = |s: &str| g.get(s).cloned().filter(|v| !v.trim().is_empty());
    match soort {
        "naam" => {
            let delen: Vec<String> = [k("voornaam").or_else(|| k("voorletters")), k("achternaam")].into_iter().flatten().collect();
            if delen.is_empty() { None } else { Some(delen.join(" ")) }
        }
        "postcode_plaats" => match (k("postcode"), k("woonplaats")) {
            (Some(p), Some(w)) => Some(format!("{p} {w}")),
            (a, b) => a.or(b),
        },
        "datum" => {
            let n = crate::logboek::nu();
            Some(format!("{}-{}-{}", &n[8..10], &n[5..7], &n[0..4]))
        }
        s => k(s),
    }
}
