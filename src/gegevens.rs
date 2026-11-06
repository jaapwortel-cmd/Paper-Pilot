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

fn bestand() -> std::path::PathBuf {
    crate::instellingen::map().join("mijn-gegevens.dat")
}

fn blob(data: &[u8]) -> CRYPT_INTEGER_BLOB {
    CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 }
}

pub fn laad() -> BTreeMap<String, String> {
    let mut uit = BTreeMap::new();
    let Ok(versleuteld) = std::fs::read(bestand()) else { return uit };
    unsafe {
        let mut leeg: CRYPT_INTEGER_BLOB = std::mem::zeroed();
        if CryptUnprotectData(&blob(&versleuteld), std::ptr::null_mut(), std::ptr::null(), std::ptr::null(), std::ptr::null(), 0, &mut leeg) == 0 {
            crate::logboek::schrijf("Mijn gegevens konden niet worden gelezen.");
            return uit;
        }
        let tekst = String::from_utf8_lossy(std::slice::from_raw_parts(leeg.pbData, leeg.cbData as usize)).to_string();
        LocalFree(leeg.pbData as _);
        for regel in tekst.lines() {
            if let Some((k, v)) = regel.split_once('=') {
                uit.insert(k.to_string(), v.to_string());
            }
        }
    }
    uit
}

pub fn bewaar(g: &BTreeMap<String, String>) -> Result<(), String> {
    let tekst: String = g.iter().filter(|(_, v)| !v.trim().is_empty()).map(|(k, v)| format!("{k}={}\n", v.replace('\n', " "))).collect();
    unsafe {
        let mut uit: CRYPT_INTEGER_BLOB = std::mem::zeroed();
        let naam = crate::win::w("Paper Pilot");
        if CryptProtectData(&blob(tekst.as_bytes()), naam.as_ptr(), std::ptr::null(), std::ptr::null(), std::ptr::null(), 0, &mut uit) == 0 {
            return Err(crate::teksten::t("GegevensNietBewaard"));
        }
        let data = std::slice::from_raw_parts(uit.pbData, uit.cbData as usize).to_vec();
        LocalFree(uit.pbData as _);
        let _ = std::fs::create_dir_all(crate::instellingen::map());
        std::fs::write(bestand(), data).map_err(|e| e.to_string())
    }
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
