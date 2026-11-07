// Paper Pilot - QR-codes en streepjescodes op scans, foto's en PDF-pagina's (met rxing, helemaal op de computer).
//
// Een QR-code kan van alles zijn: een link, een betaalverzoek (EPC-QR, "scan en betaal" op een factuur),
// wifi-gegevens, een telefoonnummer, een visitekaartje of gewone tekst. Paper Pilot zegt in gewone woorden
// wat erin staat. Een link wordt nooit vanzelf geopend: de gebruiker kiest dat zelf (Ctrl+Q).

use crate::beeld::Beeld;
use crate::teksten::{t, tf};

#[derive(Clone, Debug, Default)]
pub struct Code {
    /// "QR-code", "streepjescode" enz. (al vertaald).
    pub soort: String,
    /// De ruwe inhoud.
    pub tekst: String,
    /// Paginanummer (vanaf 1).
    pub pagina: usize,
}

/// Een betaalverzoek uit een EPC-QR-code.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Betaling {
    pub naam: String,
    pub iban: String,
    pub bedrag: String,
    pub omschrijving: String,
    pub kenmerk: String,
}

/// Zoekt alle QR-codes en streepjescodes op een beeld.
pub fn zoek(beeld: &Beeld, pagina: usize) -> Vec<Code> {
    if beeld.breed < 20 || beeld.hoog < 20 {
        return Vec::new();
    }
    let mut uit: Vec<Code> = Vec::new();
    // Eerst op ware grootte; grote foto's ook verkleind (kleine stipjes van de camera storen dan minder).
    let mut pogingen = vec![grijs(beeld)];
    if beeld.breed.max(beeld.hoog) > 2600 {
        let klein = beeld.verklein(1600);
        pogingen.push(grijs(&klein));
    }
    for (punten, b, h) in pogingen {
        let mut hints = rxing::DecodeHints::default().with(rxing::DecodeHintValue::TryHarder(true));
        if let Ok(gevonden) = rxing::helpers::detect_multiple_in_luma_with_hints(punten, b, h, &mut hints) {
            for r in gevonden {
                let tekst = r.getText().trim().to_string();
                if tekst.is_empty() || uit.iter().any(|c| c.tekst == tekst) {
                    continue;
                }
                uit.push(Code { soort: soort_naam(r.getBarcodeFormat()), tekst, pagina });
            }
        }
        if !uit.is_empty() {
            break;
        }
    }
    uit
}

fn grijs(beeld: &Beeld) -> (Vec<u8>, u32, u32) {
    let punten = beeld.punten.chunks_exact(4)
        .map(|p| ((p[2] as u32 * 299 + p[1] as u32 * 587 + p[0] as u32 * 114) / 1000) as u8)
        .collect();
    (punten, beeld.breed, beeld.hoog)
}

fn soort_naam(f: &rxing::BarcodeFormat) -> String {
    use rxing::BarcodeFormat::*;
    match f {
        QR_CODE | MICRO_QR_CODE | RECTANGULAR_MICRO_QR_CODE => t("QrSoortQr"),
        DATA_MATRIX => "Data Matrix".into(),
        AZTEC => "Aztec".into(),
        PDF_417 => "PDF417".into(),
        _ => t("QrSoortStreep"),
    }
}

/// Leest een EPC-QR-code (European Payments Council, "BCD"): naam, IBAN, bedrag en omschrijving.
pub fn betaling(tekst: &str) -> Option<Betaling> {
    let regels: Vec<&str> = tekst.split('\n').map(|r| r.trim_end_matches('\r').trim()).collect();
    if regels.first() != Some(&"BCD") || regels.len() < 7 || regels.get(3) != Some(&"SCT") {
        return None;
    }
    let bedrag = regels.get(7).copied().unwrap_or("");
    let bedrag = bedrag.strip_prefix("EUR").map(|b| b.replace('.', ",")).unwrap_or_default();
    Some(Betaling {
        naam: regels.get(5).copied().unwrap_or("").to_string(),
        iban: regels.get(6).copied().unwrap_or("").replace(' ', ""),
        bedrag,
        kenmerk: regels.get(9).copied().unwrap_or("").to_string(),
        omschrijving: regels.get(10).copied().unwrap_or("").to_string(),
    })
}

/// Is de code een webadres?
pub fn link(tekst: &str) -> Option<String> {
    let l = tekst.trim();
    let klein = l.to_lowercase();
    if (klein.starts_with("https://") || klein.starts_with("http://")) && !l.contains(char::is_whitespace) {
        return Some(l.to_string());
    }
    if klein.starts_with("www.") && !l.contains(char::is_whitespace) {
        return Some(format!("https://{l}"));
    }
    None
}

/// In gewone woorden wat er in de code staat (voor in de tekst en voor de schermlezer).
pub fn uitleg(code: &Code) -> String {
    let tekst = code.tekst.trim();
    if let Some(b) = betaling(tekst) {
        let mut delen = vec![t("QrBetaal")];
        if !b.naam.is_empty() {
            delen.push(tf("QrBetaalAan", &[&b.naam]));
        }
        if !b.bedrag.is_empty() {
            delen.push(tf("QrBetaalBedrag", &[&b.bedrag]));
        }
        if !b.iban.is_empty() {
            delen.push(tf("QrBetaalIban", &[&b.iban]));
        }
        if !b.kenmerk.is_empty() {
            delen.push(tf("QrBetaalKenmerk", &[&b.kenmerk]));
        }
        if !b.omschrijving.is_empty() {
            delen.push(tf("QrBetaalOmschrijving", &[&b.omschrijving]));
        }
        return delen.join("\r\n");
    }
    if let Some(l) = link(tekst) {
        return tf("QrLink", &[&l]);
    }
    let klein = tekst.to_lowercase();
    if klein.starts_with("wifi:") {
        let deel = |sleutel: &str| -> String {
            tekst[5..].split(';').find_map(|d| d.strip_prefix(sleutel)).unwrap_or("").to_string()
        };
        return tf("QrWifi", &[&deel("S:"), &deel("P:")]);
    }
    if klein.starts_with("tel:") {
        return tf("QrTelefoon", &[&tekst[4..]]);
    }
    if klein.starts_with("mailto:") {
        return tf("QrMail", &[tekst[7..].split('?').next().unwrap_or("")]);
    }
    if klein.starts_with("begin:vcard") {
        let naam = tekst.lines().find_map(|r| r.strip_prefix("FN:").or_else(|| r.strip_prefix("FN;CHARSET=UTF-8:"))).unwrap_or("").trim();
        let mut delen = vec![tf("QrVisitekaartje", &[naam])];
        for r in tekst.lines() {
            let r = r.trim();
            let waarde = r.split_once(':').map(|x| x.1.trim()).unwrap_or("");
            if waarde.is_empty() {
                continue;
            }
            let k = r.to_uppercase();
            if k.starts_with("TEL") {
                delen.push(tf("QrTelefoon", &[waarde]));
            } else if k.starts_with("EMAIL") {
                delen.push(tf("QrMail", &[waarde]));
            } else if k.starts_with("ADR") {
                delen.push(tf("QrAdres", &[&waarde.split(';').filter(|d| !d.is_empty()).collect::<Vec<_>>().join(", ")]));
            }
        }
        return delen.join("\r\n");
    }
    if code.soort == t("QrSoortStreep") && tekst.chars().all(|c| c.is_ascii_digit()) {
        return tf("QrNummer", &[tekst]);
    }
    tekst.replace('\n', "\r\n")
}

/// Wat er gekopieerd kan worden (naam van de keuze, de tekst), voor het keuzelijstje bij Ctrl+Q.
pub fn kopieerbaar(code: &Code) -> Vec<(String, String)> {
    let mut uit = Vec::new();
    if let Some(b) = betaling(&code.tekst) {
        if !b.iban.is_empty() {
            uit.push((tf("QrKopieerIban", &[&b.iban]), b.iban.clone()));
        }
        if !b.bedrag.is_empty() {
            uit.push((tf("QrKopieerBedrag", &[&b.bedrag]), b.bedrag.clone()));
        }
        if !b.naam.is_empty() {
            uit.push((tf("QrKopieerNaam", &[&b.naam]), b.naam.clone()));
        }
        let oms = if b.kenmerk.is_empty() { b.omschrijving.clone() } else { b.kenmerk.clone() };
        if !oms.is_empty() {
            uit.push((tf("QrKopieerOmschrijving", &[&oms]), oms));
        }
        return uit;
    }
    uit.push((t("QrKopieerTekst"), code.tekst.clone()));
    uit
}

#[cfg(test)]
mod proeven {
    use super::*;

    #[test]
    fn epc() {
        let b = betaling("BCD\n002\n1\nSCT\n\nGemeente Voorbeeld\nNL91ABNA0417164300\nEUR12.50\n\n\nFactuur 2026-123\n").unwrap();
        assert_eq!(b.naam, "Gemeente Voorbeeld");
        assert_eq!(b.iban, "NL91ABNA0417164300");
        assert_eq!(b.bedrag, "12,50");
        assert_eq!(b.omschrijving, "Factuur 2026-123");
        assert!(betaling("hallo").is_none());
    }

    #[test]
    fn links() {
        assert_eq!(link("https://example.org/a").as_deref(), Some("https://example.org/a"));
        assert_eq!(link("www.example.org").as_deref(), Some("https://www.example.org"));
        assert!(link("javascript:alert(1)").is_none());
    }
}
