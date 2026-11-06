// Paper Pilot - enveloppen herkennen (vooral enveloppen met een venster).
//
// Op een vensterenvelop staat meestal:
// - linksboven het logo of de naam van de afzender, soms een klein retouradres ("Retouradres Postbus 1234 ...");
// - in het venster het adres: een naam, een straat met huisnummer (of Postbus) en een postcode met plaats.
// Paper Pilot zoekt het adresblok aan de postcode (1234 AB) en zoekt de afzender in het retouradres, bij bekende
// afzenders, of in de tekst linksboven. Dan zegt Paper Pilot kort: van wie, aan wie.

use crate::herkennen::Regel;

pub struct Envelop {
    pub van: Option<String>,
    pub aan: Vec<String>,
}

/// Bekende afzenders van officiële post (een stukje van de naam is genoeg).
const BEKEND: [&str; 34] = [
    "Belastingdienst", "Toeslagen", "Gemeente", "Rijksoverheid", "UWV", "SVB", "DUO", "CJIB", "CAK", "RDW", "Kadaster",
    "Zilveren Kruis", "CZ", "Menzis", "VGZ", "Zorg en Zekerheid", "DSW", "ONVZ", "Anderzorg", "zorgkantoor",
    "ING", "Rabobank", "ABN AMRO", "SNS", "ASN", "Triodos", "KPN", "Ziggo", "Eneco", "Vattenfall", "Essent",
    "Waterschap", "Visio", "Bartiméus",
];

/// Een Nederlandse of Belgische postcode in de regel? (1234 AB of 1234AB, of B-1000 / 1000 met plaats)
fn heeft_postcode(s: &str) -> bool {
    let w: Vec<&str> = s.split_whitespace().collect();
    for i in 0..w.len() {
        let d = w[i];
        if d.len() == 4 && d.chars().all(|c| c.is_ascii_digit()) {
            if let Some(n) = w.get(i + 1) {
                if n.len() == 2 && n.chars().all(|c| c.is_ascii_uppercase()) {
                    return true;
                }
                // Belgisch: 4 cijfers en dan een plaatsnaam met hoofdletter.
                if n.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) && n.len() > 2 && i == 0 {
                    return true;
                }
            }
        }
        if d.len() == 6 && d[..4].chars().all(|c| c.is_ascii_digit()) && d[4..].chars().all(|c| c.is_ascii_uppercase()) {
            return true;
        }
    }
    false
}

fn heeft_huisnummer(s: &str) -> bool {
    let l = s.to_lowercase();
    l.contains("postbus") || l.contains("antwoordnummer")
        || s.split_whitespace().skip(1).any(|w| w.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false))
}

/// Bekijkt de herkende regels van een pagina. `zeker`: de gebruiker zei zelf dat het een envelop is.
pub fn bekijk(regels: &[Regel], breed: u32, hoog: u32, zeker: bool) -> Option<Envelop> {
    if regels.is_empty() {
        return None;
    }
    let woorden: usize = regels.iter().map(|r| r.tekst.split_whitespace().count()).sum();
    let liggend = breed as f32 / hoog.max(1) as f32;
    // Een envelop: liggend (ongeveer 1,3 tot 2,5 keer zo breed als hoog), weinig tekst, en een adres met postcode.
    let vorm = (1.25..=2.6).contains(&liggend);
    let postcode = regels.iter().rposition(|r| heeft_postcode(&r.tekst));
    if !zeker && !(vorm && woorden <= 90 && postcode.is_some()) {
        return None;
    }
    // Het adresblok: de regel met de postcode en hooguit vier regels erboven die dicht bij elkaar staan.
    let mut aan = Vec::new();
    let mut retour = None;
    if let Some(p) = postcode {
        let mut begin = p;
        while begin > 0 && p - begin < 4 {
            let boven = &regels[begin - 1];
            let onder = &regels[begin];
            let dicht = onder.boven - (boven.boven + boven.hoogte) < onder.hoogte * 1.6 && (boven.links - onder.links).abs() < breed as f32 * 0.15;
            let is_retour = boven.tekst.to_lowercase().starts_with("retour");
            if !dicht || is_retour {
                break;
            }
            begin -= 1;
        }
        // Een retouradresregel net boven het adres ("Retouradres Postbus 2500 1000 AB Amsterdam").
        if begin > 0 && regels[begin - 1].tekst.to_lowercase().starts_with("retour") {
            retour = Some(regels[begin - 1].tekst.clone());
        }
        aan = regels[begin..=p].iter().map(|r| r.tekst.trim().to_string()).collect();
        if !aan.iter().any(|r| heeft_huisnummer(r)) && aan.len() < 2 && !zeker {
            return None;
        }
    }
    // De afzender: een bekende naam, het retouradres, of de tekst linksboven.
    let alle: String = regels.iter().map(|r| r.tekst.as_str()).collect::<Vec<_>>().join(" ");
    let komt_voor = |tekst: &str, b: &str| -> bool {
        if b.len() <= 4 {
            // Korte namen (CZ, ING, SVB) alleen als los woord in hoofdletters.
            tekst.split(|c: char| !c.is_alphanumeric()).any(|w| w == b)
        } else {
            tekst.to_lowercase().contains(&b.to_lowercase())
        }
    };
    let mut van = BEKEND.iter().find(|b| komt_voor(&alle, b) && !aan.iter().any(|a| komt_voor(a, b))).map(|b| {
        // De hele regel noemen (bijvoorbeeld "Gemeente Utrecht").
        regels.iter().find(|r| komt_voor(&r.tekst, b)).map(|r| r.tekst.trim().to_string()).unwrap_or_else(|| b.to_string())
    });
    if van.is_none() {
        if let Some(r) = &retour {
            van = Some(r.trim_start_matches(|c: char| c.is_alphabetic()).trim_start_matches([':', ' ']).to_string());
        }
    }
    if van.is_none() {
        van = regels.iter().find(|r| r.boven < hoog as f32 * 0.35 && r.links < breed as f32 * 0.5 && !aan.contains(&r.tekst.trim().to_string()))
            .map(|r| r.tekst.trim().to_string());
    }
    if aan.is_empty() && van.is_none() {
        return None;
    }
    Some(Envelop { van, aan })
}
