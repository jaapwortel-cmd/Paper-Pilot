// Paper Pilot - "Wat moet ik doen?": de belangrijke gegevens uit een brief halen.
//
// Zonder taalmodel, met vaste regels: rekeningnummers (IBAN, met de controle van het getal), bedragen,
// het betalingskenmerk en uiterste datums ("vóór 1 december 2026"). Ook betaal-QR-codes tellen mee.
// De samenvatting in gewone woorden maakt het taalmodel (als dat er is), op de computer zelf.

use crate::teksten::{t, tf};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Gegevens {
    pub ibans: Vec<String>,
    pub bedragen: Vec<String>,
    pub kenmerken: Vec<String>,
    /// (datum zoals in de brief, de zin eromheen)
    pub datums: Vec<(String, String)>,
    pub telefoon: Vec<String>,
}

fn voeg_toe(lijst: &mut Vec<String>, s: String) {
    if !s.is_empty() && !lijst.contains(&s) {
        lijst.push(s);
    }
}

/// Zoekt de gegevens in de tekst van een document (en in de QR-codes).
pub fn zoek(tekst: &str, codes: &[crate::qr::Code]) -> Gegevens {
    let mut g = Gegevens::default();
    for c in codes {
        if let Some(b) = crate::qr::betaling(&c.tekst) {
            if iban_klopt(&b.iban) {
                voeg_toe(&mut g.ibans, iban_netjes(&b.iban));
            }
            voeg_toe(&mut g.bedragen, b.bedrag.clone());
            voeg_toe(&mut g.kenmerken, if b.kenmerk.is_empty() { b.omschrijving.clone() } else { b.kenmerk.clone() });
        }
    }
    for i in ibans(tekst) {
        voeg_toe(&mut g.ibans, i);
    }
    for b in bedragen(tekst) {
        voeg_toe(&mut g.bedragen, b);
    }
    for k in kenmerken(tekst) {
        voeg_toe(&mut g.kenmerken, k);
    }
    g.datums = uiterste_datums(tekst);
    g.telefoon = telefoonnummers(tekst);
    g
}

// ------------------------------------------------------------------ IBAN

/// Controleert een IBAN met het getal 97 (zoals de bank dat doet).
pub fn iban_klopt(iban: &str) -> bool {
    let s: String = iban.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_uppercase();
    if s.len() < 15 || s.len() > 34 || !s.chars().all(|c| c.is_ascii_alphanumeric()) {
        return false;
    }
    let b = s.as_bytes();
    if !b[0].is_ascii_alphabetic() || !b[1].is_ascii_alphabetic() || !b[2].is_ascii_digit() || !b[3].is_ascii_digit() {
        return false;
    }
    let lengte = match &s[..2] {
        "NL" => 18, "BE" => 16, "DE" => 22, "FR" => 27, "LU" => 20, "GB" => 22, "ES" => 24, "IT" => 27, "AT" => 20, "CH" => 21,
        _ => s.len(),
    };
    if s.len() != lengte {
        return false;
    }
    let omgedraaid = format!("{}{}", &s[4..], &s[..4]);
    let mut rest: u32 = 0;
    for c in omgedraaid.chars() {
        let waarde = if c.is_ascii_digit() { c as u32 - '0' as u32 } else { c as u32 - 'A' as u32 + 10 };
        rest = if waarde >= 10 { (rest * 100 + waarde) % 97 } else { (rest * 10 + waarde) % 97 };
    }
    rest == 1
}

/// In groepjes van vier, zoals op een bankpas: NL91 ABNA 0417 1643 00.
pub fn iban_netjes(iban: &str) -> String {
    let s: String = iban.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_uppercase();
    s.chars().collect::<Vec<_>>().chunks(4).map(|c| c.iter().collect::<String>()).collect::<Vec<_>>().join(" ")
}

fn ibans(tekst: &str) -> Vec<String> {
    let tekens: Vec<char> = tekst.chars().collect();
    let mut uit = Vec::new();
    let mut i = 0;
    while i + 4 <= tekens.len() {
        let begin_woord = i == 0 || !tekens[i - 1].is_alphanumeric();
        if begin_woord && tekens[i].is_ascii_uppercase() && tekens[i + 1].is_ascii_uppercase() && tekens[i + 2].is_ascii_digit() && tekens[i + 3].is_ascii_digit() {
            // Letters en cijfers verzamelen, losse spaties mogen (NL91 ABNA 0417 1643 00).
            let mut s = String::new();
            let mut j = i;
            while j < tekens.len() && s.len() < 34 {
                let c = tekens[j];
                if c.is_ascii_alphanumeric() {
                    s.push(c.to_ascii_uppercase());
                } else if c == ' ' && j + 1 < tekens.len() && tekens[j + 1].is_ascii_alphanumeric() && !s.is_empty() {
                    // spatie tussen groepjes
                } else {
                    break;
                }
                j += 1;
            }
            // De langste geldige IBAN vanaf hier.
            let mut gevonden = false;
            for lengte in (15..=s.len()).rev() {
                if iban_klopt(&s[..lengte]) {
                    let n = iban_netjes(&s[..lengte]);
                    if !uit.contains(&n) {
                        uit.push(n);
                    }
                    gevonden = true;
                    break;
                }
            }
            if gevonden {
                i = j;
                continue;
            }
        }
        i += 1;
    }
    uit
}

// ------------------------------------------------------------------ bedragen

/// Bedragen als "€ 12,50", "€12,-", "12,50 euro" of "EUR 1.234,56". Geeft "12,50" (zonder euroteken).
fn bedragen(tekst: &str) -> Vec<String> {
    let tekens: Vec<char> = tekst.chars().collect();
    let mut uit: Vec<(u8, String)> = Vec::new();
    let getal_vanaf = |mut i: usize| -> Option<(String, usize)> {
        let begin = i;
        while i < tekens.len() && (tekens[i].is_ascii_digit() || tekens[i] == '.' || tekens[i] == ',' || (tekens[i] == '-' && i > begin && tekens[i - 1] == ',')) {
            i += 1;
        }
        let s: String = tekens[begin..i].iter().collect();
        let s = s.trim_end_matches('.').to_string();
        if s.is_empty() || !s.chars().next().unwrap().is_ascii_digit() {
            return None;
        }
        Some((s, i))
    };
    let mut i = 0;
    while i < tekens.len() {
        let mut gevonden: Option<(String, usize, usize)> = None; // (getal, begin, eind)
        if tekens[i] == '€' || tekst_op(&tekens, i, "EUR ") || tekst_op(&tekens, i, "EUR\u{a0}") {
            let mut j = i + if tekens[i] == '€' { 1 } else { 4 };
            while j < tekens.len() && (tekens[j] == ' ' || tekens[j] == '\u{a0}') {
                j += 1;
            }
            if let Some((g, eind)) = getal_vanaf(j) {
                gevonden = Some((g, i, eind));
            }
        } else if tekens[i].is_ascii_digit() && (i == 0 || !tekens[i - 1].is_alphanumeric() && tekens[i - 1] != '.' && tekens[i - 1] != ',') {
            if let Some((g, eind)) = getal_vanaf(i) {
                let mut k = eind;
                while k < tekens.len() && tekens[k] == ' ' {
                    k += 1;
                }
                let na: String = tekens[k..(k + 5).min(tekens.len())].iter().collect::<String>().to_lowercase();
                if na.starts_with("euro") && g.contains(',') {
                    gevonden = Some((g, i, k + 4));
                }
            }
        }
        if let Some((g, begin, eind)) = gevonden {
            let netjes = g.trim_end_matches(",-").trim_end_matches(",--").to_string();
            let netjes = if g.ends_with(",-") { format!("{netjes},00") } else { netjes };
            // Wat staat er vlak voor en na? "Te betalen", "nog ... betalen" = het bedrag dat je moet betalen (vooraan).
            let voor: String = tekens[begin.saturating_sub(40)..begin].iter().collect::<String>().to_lowercase();
            let na: String = tekens[eind.min(tekens.len())..(eind + 25).min(tekens.len())].iter().collect::<String>().to_lowercase();
            let mut punten = 0u8;
            if ["te betalen", "openstaand", "nog ", "to pay", "amount due", "please pay"].iter().any(|w| voor.contains(w))
                || ["betalen", "te betalen", "overmaken", "to pay"].iter().any(|w| na.trim_start().starts_with(w) || na.contains(w))
            {
                punten = 2;
            } else if ["totaal", "bedrag", "betaal", "amount", "total"].iter().any(|w| voor.contains(w)) {
                punten = 1;
            }
            if let Some(x) = uit.iter_mut().find(|(_, b)| *b == netjes) {
                x.0 = x.0.max(punten);
            } else {
                uit.push((punten, netjes));
            }
            i = eind.max(i + 1);
            continue;
        }
        i += 1;
    }
    uit.sort_by_key(|(p, _)| std::cmp::Reverse(*p));
    uit.into_iter().map(|(_, b)| b).collect()
}

fn tekst_op(tekens: &[char], i: usize, woord: &str) -> bool {
    let w: Vec<char> = woord.chars().collect();
    i + w.len() <= tekens.len() && tekens[i..i + w.len()].iter().zip(&w).all(|(a, b)| a.to_lowercase().eq(b.to_lowercase()))
}

// ------------------------------------------------------------------ betalingskenmerk

fn kenmerken(tekst: &str) -> Vec<String> {
    let mut uit = Vec::new();
    let klein = tekst.to_lowercase();
    for woord in ["betalingskenmerk", "betaalkenmerk", "betalingsreferentie", "payment reference", "omschrijving", "factuurnummer", "invoice number"] {
        let mut vanaf = 0;
        while let Some(p) = klein[vanaf..].find(woord) {
            let begin = vanaf + p + woord.len();
            vanaf = begin;
            // Na het woord: eventueel ":" en dan de waarde tot het einde van de regel.
            let Some(rest) = tekst.get(begin..) else { continue };
            let rest = rest.trim_start_matches(|c: char| c == ':' || c == ' ' || c == '\u{a0}' || c == '|' || c == '\t');
            let regel = rest.split(['\r', '\n']).next().unwrap_or("").trim();
            // Het kenmerk zelf: cijfers, letters, spaties en streepjes; stopt bij een woord zonder cijfers na het kenmerk.
            let mut delen: Vec<&str> = Vec::new();
            for d in regel.split_whitespace() {
                let d = d.trim_end_matches(['.', ',', ';', ')']);
                if d.chars().any(|c| c.is_ascii_digit()) && d.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '/') {
                    delen.push(d);
                } else {
                    break;
                }
            }
            let k = delen.join(" ");
            if k.chars().filter(|c| c.is_ascii_digit()).count() >= 4 && !uit.contains(&k) {
                uit.push(k);
            }
        }
    }
    uit
}

// ------------------------------------------------------------------ uiterste datum

const MAANDEN: [&str; 24] = ["januari", "februari", "maart", "april", "mei", "juni", "juli", "augustus", "september", "oktober", "november", "december",
    "january", "february", "march", "april", "may", "june", "july", "august", "september", "october", "november", "december"];

/// Datums in de tekst, met de plaats: "1 december 2026", "01-12-2026", "1/12/2026".
fn datums_in(zin: &str) -> Vec<String> {
    let woorden: Vec<&str> = zin.split_whitespace().collect();
    let mut uit = Vec::new();
    for (i, w) in woorden.iter().enumerate() {
        let w = w.trim_matches(|c: char| c == ',' || c == '.' || c == '(' || c == ')' || c == ':');
        // 1 december 2026
        if w.chars().all(|c| c.is_ascii_digit()) && !w.is_empty() && w.len() <= 2 {
            if let Some(m) = woorden.get(i + 1).map(|m| m.trim_matches(|c: char| !c.is_alphabetic()).to_lowercase()) {
                if MAANDEN.contains(&m.as_str()) {
                    let mut d = format!("{w} {m}");
                    if let Some(j) = woorden.get(i + 2).map(|j| j.trim_matches(|c: char| !c.is_ascii_digit())) {
                        if j.len() == 4 {
                            d.push(' ');
                            d.push_str(j);
                        }
                    }
                    uit.push(d);
                }
            }
        }
        // 01-12-2026 of 1/12/2026
        let delen: Vec<&str> = w.split(['-', '/']).collect();
        if delen.len() == 3 && delen.iter().all(|d| !d.is_empty() && d.chars().all(|c| c.is_ascii_digit()))
            && delen[0].len() <= 2 && delen[1].len() <= 2 && (delen[2].len() == 4 || delen[2].len() == 2)
        {
            let (d, m) = (delen[0].parse::<u32>().unwrap_or(0), delen[1].parse::<u32>().unwrap_or(0));
            if (1..=31).contains(&d) && (1..=12).contains(&m) {
                uit.push(w.to_string());
            }
        }
    }
    uit
}

fn uiterste_datums(tekst: &str) -> Vec<(String, String)> {
    const WOORDEN: [&str; 14] = ["vóór", "voor ", "uiterlijk", "uiterste", "binnen", "deadline", "betaal", "vervalt", "verloopt",
        "before", "no later", "due", "by ", "terugsturen"];
    let mut uit: Vec<(String, String)> = Vec::new();
    // Zinnen: tot een punt met spatie of een regeleinde.
    for regel in tekst.split(['\r', '\n']) {
        for zin in regel.split(". ") {
            let klein = zin.to_lowercase();
            if !WOORDEN.iter().any(|w| klein.contains(w)) {
                continue;
            }
            for d in datums_in(zin) {
                if !uit.iter().any(|(x, _)| *x == d) {
                    let z = zin.trim();
                    let z: String = if z.chars().count() > 200 { z.chars().take(200).collect::<String>() + "…" } else { z.to_string() };
                    uit.push((d, z));
                }
            }
        }
    }
    uit
}

// ------------------------------------------------------------------ telefoon

fn telefoonnummers(tekst: &str) -> Vec<String> {
    let mut uit = Vec::new();
    let klein = tekst.to_lowercase();
    for woord in ["telefoon", "tel.", "tel:", "bel ", "bellen", "phone", "call "] {
        let mut vanaf = 0;
        while let Some(p) = klein[vanaf..].find(woord) {
            let begin = vanaf + p + woord.len();
            vanaf = begin;
            let Some(rest) = tekst.get(begin..) else { continue };
            let rest: String = rest.chars().take(60).collect();
            // Het eerste stuk met minstens 10 cijfers (spaties, streepjes en haakjes mogen).
            let Some(start) = rest.find(|c: char| c.is_ascii_digit() || c == '+') else { continue };
            let mut nummer = String::new();
            for c in rest[start..].chars() {
                if c.is_ascii_digit() || c == '+' || c == ' ' || c == '-' || c == '(' || c == ')' {
                    nummer.push(c);
                } else {
                    break;
                }
            }
            let nummer = nummer.trim().trim_end_matches(['-', '(', ' ']).trim().to_string();
            let cijfers = nummer.chars().filter(|c| c.is_ascii_digit()).count();
            if (10..=13).contains(&cijfers) && !uit.contains(&nummer) {
                uit.push(nummer);
            }
        }
    }
    uit
}

// ------------------------------------------------------------------ voor het venster

/// De gegevens als tekst onder de samenvatting.
pub fn als_tekst(g: &Gegevens) -> String {
    let mut r: Vec<String> = Vec::new();
    if let Some(b) = g.bedragen.first() {
        r.push(tf("TaakBedrag", &[b]));
    }
    for i in &g.ibans {
        r.push(tf("TaakIban", &[i]));
    }
    for k in &g.kenmerken {
        r.push(tf("TaakKenmerk", &[k]));
    }
    for (d, zin) in &g.datums {
        r.push(tf("TaakDatum", &[d, zin]));
    }
    for n in &g.telefoon {
        r.push(tf("TaakTelefoon", &[n]));
    }
    if g.bedragen.len() > 1 {
        r.push(tf("TaakAndereBedragen", &[&g.bedragen[1..].join("; ")]));
    }
    if r.is_empty() {
        return t("TaakGeenGegevens");
    }
    r.join("\r\n")
}

/// Wat er gekopieerd kan worden: (naam in de lijst, tekst voor het klembord).
pub fn kopieerbaar(g: &Gegevens) -> Vec<(String, String)> {
    let mut uit = Vec::new();
    for b in &g.bedragen {
        uit.push((tf("TaakKopieerBedrag", &[b]), b.clone()));
    }
    for i in &g.ibans {
        uit.push((tf("TaakKopieerIban", &[i]), i.replace(' ', "")));
    }
    for k in &g.kenmerken {
        uit.push((tf("TaakKopieerKenmerk", &[k]), k.clone()));
    }
    for (d, _) in &g.datums {
        uit.push((tf("TaakKopieerDatum", &[d]), d.clone()));
    }
    for n in &g.telefoon {
        uit.push((tf("TaakKopieerTelefoon", &[n]), n.clone()));
    }
    uit
}

#[cfg(test)]
mod proeven {
    use super::*;

    #[test]
    fn iban() {
        assert!(iban_klopt("NL91ABNA0417164300"));
        assert!(iban_klopt("NL91 ABNA 0417 1643 00"));
        assert!(!iban_klopt("NL92ABNA0417164300"));
        assert_eq!(ibans("Maak het over naar NL91 ABNA 0417 1643 00 ten name van X."), vec!["NL91 ABNA 0417 1643 00"]);
    }

    #[test]
    fn brief() {
        let tekst = "Te betalen: € 12,50\r\nBetalingskenmerk: 1234 5678 9012 3456\r\nBetaal vóór 1 december 2026. Bel ons op 030 - 123 45 67.";
        let g = zoek(tekst, &[]);
        assert_eq!(g.bedragen, vec!["12,50"]);
        assert_eq!(g.kenmerken, vec!["1234 5678 9012 3456"]);
        assert_eq!(g.datums[0].0, "1 december 2026");
        assert_eq!(g.telefoon, vec!["030 - 123 45 67"]);
    }
}
