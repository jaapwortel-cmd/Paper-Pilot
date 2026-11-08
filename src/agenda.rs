// Paper Pilot - afspraken uit een brief in je agenda zetten.
//
// Paper Pilot zoekt in de brief naar een afspraak: een datum met een tijd, en woorden als "afspraak", "verwacht",
// "uitnodiging" of een regel "Tijd:". Ook de plaats ("Locatie:", "Adres:"). Daarna maakt het een agendabestand
// (.ics) en opent dat: Outlook, de Agenda van Windows of een ander agendaprogramma neemt de afspraak over.
// Met een herinnering een dag van tevoren.

use crate::herinneringen::{iso, lees_datum, mooi};
use crate::teksten::{t, tf};

#[derive(Clone, Debug, PartialEq)]
pub struct Afspraak {
    pub datum: (i32, u32, u32),
    pub tijd: Option<(u32, u32)>,
    pub eind: Option<(u32, u32)>,
    pub plaats: String,
    pub wat: String,
    /// De zin of regels uit de brief.
    pub zin: String,
}

impl Afspraak {
    /// "12 december 2026, 10.30 uur" (voor de lijst en de melding).
    pub fn wanneer(&self) -> String {
        match self.tijd {
            Some((u, m)) => tf("AgendaWanneer", &[&mooi(self.datum), &format!("{u}.{m:02}")]),
            None => mooi(self.datum),
        }
    }
}

const WOORDEN: [&str; 24] = ["afspraak", "verwacht", "uitnodig", "spreekuur", "consult", "gesprek", "bezoek", "onderzoek", "melden",
    "keuring", "behandeling", "plaatsvind", "plaats vind", "tijd:", "aanvang", "begint om", "appointment", "expected", "invite",
    "meeting", "visit", "check-in", "inloop", "vaccinatie"];

/// Tijden in een regel: "10.30 uur", "10:30", "14 uur", "van 9.00 tot 10.00".
pub fn tijden_in(regel: &str) -> Vec<(u32, u32)> {
    let mut uit = Vec::new();
    let tekens: Vec<char> = regel.chars().collect();
    let mut i = 0;
    while i < tekens.len() {
        if !tekens[i].is_ascii_digit() || (i > 0 && (tekens[i - 1].is_ascii_digit() || tekens[i - 1] == '-' || tekens[i - 1] == '/')) {
            i += 1;
            continue;
        }
        let mut j = i;
        while j < tekens.len() && tekens[j].is_ascii_digit() {
            j += 1;
        }
        let uur: u32 = tekens[i..j].iter().collect::<String>().parse().unwrap_or(99);
        // Een bedrag ("€ 12.50" of "12.50 euro") is geen tijd.
        let ervoor: String = tekens[i.saturating_sub(5)..i].iter().collect::<String>().to_lowercase();
        let erna: String = tekens[j..(j + 10).min(tekens.len())].iter().collect::<String>().to_lowercase();
        if ervoor.trim_end().ends_with('€') || ervoor.trim_end().ends_with("eur") || erna.contains("euro") {
            i = j;
            while i < tekens.len() && (tekens[i].is_ascii_digit() || tekens[i] == '.' || tekens[i] == ',') {
                i += 1;
            }
            continue;
        }
        if j - i <= 2 && uur < 24 {
            // 10.30 of 10:30
            if j + 2 < tekens.len() && (tekens[j] == '.' || tekens[j] == ':') && tekens[j + 1].is_ascii_digit() && tekens[j + 2].is_ascii_digit()
                && !tekens.get(j + 3).map(|c| c.is_ascii_digit() || *c == '.' || *c == '-').unwrap_or(false)
            {
                let min: u32 = tekens[j + 1..j + 3].iter().collect::<String>().parse().unwrap_or(99);
                if min < 60 {
                    uit.push((uur, min));
                    i = j + 3;
                    continue;
                }
            }
            // 14 uur
            let rest: String = tekens[j..(j + 5).min(tekens.len())].iter().collect::<String>().to_lowercase();
            if rest.trim_start().starts_with("uur") {
                uit.push((uur, 0));
            }
        }
        i = j;
    }
    uit
}

/// De plaats bij een afspraak: een regel als "Locatie: ...", "Adres: ...", "Waar: ...".
fn plaats_in(regels: &[&str]) -> String {
    for r in regels {
        let l = r.trim().to_lowercase();
        for label in ["locatie", "adres", "plaats", "waar", "afdeling", "location", "address", "where"] {
            if let Some(rest) = l.strip_prefix(label) {
                let rest = rest.trim_start();
                if rest.starts_with(':') || rest.starts_with('|') {
                    let begin = r.trim().len() - rest.len() + 1;
                    let waarde = r.trim().get(begin..).unwrap_or("").trim_start_matches([':', '|', ' ']).trim().to_string();
                    if !waarde.is_empty() && !waarde.chars().all(|c| c.is_ascii_digit()) {
                        return waarde;
                    }
                }
            }
        }
    }
    String::new()
}

/// Zoekt afspraken in de tekst van een brief (alleen in de toekomst).
pub fn zoek(tekst: &str, afzender: &str) -> Vec<Afspraak> {
    let regels: Vec<&str> = tekst.lines().collect();
    let vandaag = crate::herinneringen::dagnummer(crate::herinneringen::vandaag());
    let mut uit: Vec<Afspraak> = Vec::new();
    for (i, regel) in regels.iter().enumerate() {
        for d in crate::taken::datums_in(regel) {
            let Some(datum) = lees_datum(&d) else { continue };
            if crate::herinneringen::dagnummer(datum) < vandaag {
                continue;
            }
            // De regel zelf en de twee regels erna (Datum / Tijd / Locatie onder elkaar).
            let van = i.saturating_sub(3);
            let tot = (i + 4).min(regels.len());
            let buurt = &regels[van..tot];
            let tijden: Vec<(u32, u32)> = std::iter::once(*regel).chain(regels[i + 1..(i + 3).min(regels.len())].iter().copied())
                .flat_map(tijden_in).collect();
            let samen = buurt.join(" ").to_lowercase();
            let is_afspraak = WOORDEN.iter().any(|w| samen.contains(w));
            if !is_afspraak {
                continue;
            }
            // Geen afspraak: "betalen vóór 1 december", "uiterlijk", "bezwaar".
            if tijden.is_empty() && ["betaal", "uiterlijk", "voor ", "vóór", "bezwaar"].iter().any(|w| regel.to_lowercase().contains(w)) {
                continue;
            }
            let wat = if afzender.is_empty() { t("AgendaAfspraak") } else { tf("AgendaAfspraakBij", &[afzender]) };
            let a = Afspraak {
                datum,
                tijd: tijden.first().copied(),
                eind: tijden.get(1).copied().filter(|e| Some(*e) > tijden.first().copied()),
                plaats: plaats_in(&regels[i.saturating_sub(2)..(i + 6).min(regels.len())]),
                wat,
                zin: buurt.iter().map(|r| r.trim()).filter(|r| !r.is_empty()).collect::<Vec<_>>().join(" "),
            };
            if !uit.iter().any(|x| x.datum == a.datum && x.tijd == a.tijd) {
                uit.push(a);
            }
        }
    }
    uit
}

fn ics_tekst(s: &str) -> String {
    s.replace('\\', "\\\\").replace(';', "\\;").replace(',', "\\,").replace("\r\n", "\\n").replace('\n', "\\n")
}

/// Het agendabestand (iCalendar) voor een afspraak.
pub fn ics(a: &Afspraak) -> String {
    let nu = crate::logboek::nu().replace(['-', ':'], "").replace(' ', "T");
    let d = iso(a.datum).replace('-', "");
    let (begin, eind) = match a.tijd {
        Some((u, m)) => {
            let (eu, em) = a.eind.unwrap_or(((u + 1).min(23), m));
            (format!("DTSTART:{d}T{u:02}{m:02}00"), format!("DTEND:{d}T{eu:02}{em:02}00"))
        }
        None => {
            let volgende = crate::herinneringen::van_dagnummer(crate::herinneringen::dagnummer(a.datum) + 1);
            (format!("DTSTART;VALUE=DATE:{d}"), format!("DTEND;VALUE=DATE:{}", iso(volgende).replace('-', "")))
        }
    };
    let mut regels = vec![
        "BEGIN:VCALENDAR".to_string(), "VERSION:2.0".into(), "PRODID:-//Paper Pilot//NL".into(), "METHOD:PUBLISH".into(),
        "BEGIN:VEVENT".into(), format!("UID:{nu}-{d}@paperpilot"), format!("DTSTAMP:{nu}"), begin, eind,
        format!("SUMMARY:{}", ics_tekst(&a.wat)),
    ];
    if !a.plaats.is_empty() {
        regels.push(format!("LOCATION:{}", ics_tekst(&a.plaats)));
    }
    regels.push(format!("DESCRIPTION:{}", ics_tekst(&a.zin)));
    regels.extend(["BEGIN:VALARM".to_string(), "ACTION:DISPLAY".into(), format!("DESCRIPTION:{}", ics_tekst(&a.wat)),
        "TRIGGER:-P1D".into(), "END:VALARM".into(), "END:VEVENT".into(), "END:VCALENDAR".into()]);
    regels.join("\r\n") + "\r\n"
}

/// Zet de afspraak in de agenda: maakt het .ics-bestand en opent het met het agendaprogramma.
pub fn in_agenda(eigenaar: windows_sys::Win32::Foundation::HWND, a: &Afspraak) {
    use crate::dialogen;
    let map = std::env::temp_dir().join("Paper Pilot");
    let _ = std::fs::create_dir_all(&map);
    let pad = map.join(format!("afspraak {}.ics", iso(a.datum)));
    if let Err(e) = std::fs::write(&pad, ics(a)) {
        dialogen::melding(eigenaar, &t("AgendaTitel"), &e.to_string(), dialogen::ICOON_WAARSCHUWING);
        return;
    }
    crate::logboek::schrijf("Afspraak naar de agenda gestuurd.");
    if crate::win::open_adres(&pad.to_string_lossy()) {
        dialogen::melding(eigenaar, &t("AgendaTitel"), &tf("AgendaGeopend", &[&a.wanneer()]), dialogen::ICOON_INFO);
    } else {
        dialogen::melding(eigenaar, &t("AgendaTitel"), &tf("AgendaGeenProgramma", &[&pad.to_string_lossy()]), dialogen::ICOON_WAARSCHUWING);
    }
}

/// Ctrl+E: de afspraken in deze brief; kies er een, of maak er zelf een.
pub fn venster(eigenaar: windows_sys::Win32::Foundation::HWND, doc: &crate::lezen::Document) {
    use crate::dialogen;
    let afzender = crate::post::afzender(doc);
    let afspraken = zoek(&doc.tekst, &afzender);
    let mut namen: Vec<String> = afspraken.iter().map(|a| tf("AgendaLijstRegel", &[&a.wanneer(), &a.plaats])).collect();
    namen.push(t("AgendaZelf"));
    let label = if afspraken.is_empty() { t("AgendaGeenGevonden") } else { t("AgendaKies") };
    let Some(i) = crate::keuzevensters::kies_uit_lijst(eigenaar, &t("AgendaTitel"), &label, &namen, 0) else { return };
    let mut a = match afspraken.get(i) {
        Some(a) => a.clone(),
        None => Afspraak { datum: crate::herinneringen::vandaag(), tijd: None, eind: None, plaats: String::new(),
            wat: if afzender.is_empty() { t("AgendaAfspraak") } else { tf("AgendaAfspraakBij", &[&afzender]) }, zin: doc.naam.clone() },
    };
    // Altijd nog even laten nakijken: datum, tijd, wat en waar.
    let mut velden = vec![
        (t("AgendaDatumVraag"), if afspraken.get(i).is_some() { mooi(a.datum) } else { String::new() }),
        (t("AgendaTijdVraag"), a.tijd.map(|(u, m)| format!("{u}.{m:02}")).unwrap_or_default()),
        (t("AgendaWatVraag"), a.wat.clone()),
        (t("AgendaWaarVraag"), a.plaats.clone()),
    ];
    loop {
        let Some(antwoord) = crate::keuzevensters::formulier(eigenaar, &t("AgendaTitel"), &t("AgendaNakijken"), &velden) else { return };
        let Some(d) = lees_datum(&antwoord[0]) else {
            dialogen::melding(eigenaar, &t("AgendaTitel"), &tf("HerinneringDatumFout", &[&antwoord[0]]), dialogen::ICOON_WAARSCHUWING);
            for (v, w) in velden.iter_mut().zip(&antwoord) {
                v.1 = w.clone();
            }
            continue;
        };
        a.datum = d;
        let tijd = tijden_in(&format!("{} uur", antwoord[1].trim().trim_end_matches("uur").trim()));
        if a.tijd != tijd.first().copied() {
            a.eind = None;
        }
        a.tijd = tijd.first().copied();
        a.wat = if antwoord[2].trim().is_empty() { t("AgendaAfspraak") } else { antwoord[2].trim().to_string() };
        a.plaats = antwoord[3].trim().to_string();
        in_agenda(eigenaar, &a);
        return;
    }
}

#[cfg(test)]
mod proeven {
    use super::*;

    #[test]
    fn tijden() {
        assert_eq!(tijden_in("om 10.30 uur"), vec![(10, 30)]);
        assert_eq!(tijden_in("van 9:00 tot 10:15"), vec![(9, 0), (10, 15)]);
        assert_eq!(tijden_in("om 14 uur"), vec![(14, 0)]);
        assert!(tijden_in("12-12-2026").is_empty());
        assert!(tijden_in("bedrag 245,80").is_empty());
    }
}
