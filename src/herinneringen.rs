// Paper Pilot - herinneringen: "betalen vóór 1 december" niet vergeten.
//
// Een herinnering heeft een datum en een korte tekst. Vanaf drie dagen ervoor waarschuwt Paper Pilot, één keer per
// dag, tot je zegt dat het gedaan is. Dat gebeurt bij de start van Paper Pilot, en ook als Paper Pilot niet open is:
// een geplande taak van Windows ("Paper Pilot herinneringen") start elke dag om 10 uur `PaperPilot.exe --herinneringen`.
// Is er niets te melden, dan stopt dat meteen, zonder venster. Mis je 10 uur (computer uit), dan komt hij later.
// De herinneringen staan versleuteld (DPAPI) in %APPDATA%\Paper Pilot\herinneringen.dat.

use crate::teksten::{t, tf};
use serde_json::{json, Value};
use windows_sys::Win32::Foundation::HWND;

const BESTAND: &str = "herinneringen.dat";
const TAAK: &str = "Paper Pilot herinneringen";
/// Zoveel dagen van tevoren begint Paper Pilot te waarschuwen.
pub const DAGEN_VOORAF: i64 = 3;

#[derive(Clone, Debug, PartialEq)]
pub struct Herinnering {
    pub id: u64,
    /// jjjj-mm-dd
    pub datum: String,
    pub wat: String,
    pub document: String,
    /// De dag (jjjj-mm-dd) waarop Paper Pilot het laatst waarschuwde.
    pub gemeld: String,
}

// ------------------------------------------------------------------ datums

/// Vandaag als (jaar, maand, dag).
pub fn vandaag() -> (i32, u32, u32) {
    let nu = crate::logboek::nu();
    lees_iso(&nu[..10]).unwrap_or((2026, 1, 1))
}

pub fn iso(d: (i32, u32, u32)) -> String {
    format!("{:04}-{:02}-{:02}", d.0, d.1, d.2)
}

pub fn lees_iso(s: &str) -> Option<(i32, u32, u32)> {
    let mut delen = s.trim().split('-');
    let j = delen.next()?.parse().ok()?;
    let m = delen.next()?.parse().ok()?;
    let d = delen.next()?.parse().ok()?;
    geldig((j, m, d))
}

fn geldig(d: (i32, u32, u32)) -> Option<(i32, u32, u32)> {
    let dagen = [31, if schrikkel(d.0) { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    if (1..=12).contains(&d.1) && d.2 >= 1 && d.2 <= dagen[d.1 as usize - 1] && (1900..=2200).contains(&d.0) {
        Some(d)
    } else {
        None
    }
}

fn schrikkel(j: i32) -> bool {
    (j % 4 == 0 && j % 100 != 0) || j % 400 == 0
}

/// Het aantal dagen sinds 1 januari 1970 (om verschillen uit te rekenen).
pub fn dagnummer(d: (i32, u32, u32)) -> i64 {
    let (j, m, dag) = (d.0 as i64, d.1 as i64, d.2 as i64);
    let j = if m <= 2 { j - 1 } else { j };
    let era = if j >= 0 { j } else { j - 399 } / 400;
    let yoe = j - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + dag - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// Terug van een dagnummer naar (jaar, maand, dag).
pub fn van_dagnummer(n: i64) -> (i32, u32, u32) {
    let z = n + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let j = (yoe + era * 400 + if m <= 2 { 1 } else { 0 }) as i32;
    (j, m, d)
}

const MAANDEN_NL: [&str; 12] = ["januari", "februari", "maart", "april", "mei", "juni", "juli", "augustus", "september", "oktober", "november", "december"];
const MAANDEN_EN: [&str; 12] = ["january", "february", "march", "april", "may", "june", "july", "august", "september", "october", "november", "december"];
const MAANDEN_KORT: [&str; 12] = ["jan", "feb", "mrt", "apr", "mei", "jun", "jul", "aug", "sep", "okt", "nov", "dec"];

/// Een datum zoals mensen hem schrijven: "1 december 2026", "1 december", "01-12-2026", "1/12/26", "2026-12-01".
/// Zonder jaar: dit jaar, of volgend jaar als de datum al voorbij is.
pub fn lees_datum(s: &str) -> Option<(i32, u32, u32)> {
    let s = s.trim().to_lowercase().replace(['.', ','], " ");
    if let Some(d) = lees_iso(&s) {
        return Some(d);
    }
    let nu = vandaag();
    let met_jaar = |d: u32, m: u32, j: Option<i32>| -> Option<(i32, u32, u32)> {
        match j {
            Some(j) => geldig((if j < 100 { 2000 + j } else { j }, m, d)),
            None => {
                let dit = geldig((nu.0, m, d))?;
                if dagnummer(dit) < dagnummer(nu) { geldig((nu.0 + 1, m, d)) } else { Some(dit) }
            }
        }
    };
    // 01-12-2026 of 1/12/26
    let delen: Vec<&str> = s.split(['-', '/']).map(|d| d.trim()).collect();
    if delen.len() >= 2 && delen.iter().all(|d| !d.is_empty() && d.chars().all(|c| c.is_ascii_digit())) {
        let d = delen[0].parse().ok()?;
        let m = delen[1].parse().ok()?;
        let j = delen.get(2).and_then(|j| j.parse().ok());
        return met_jaar(d, m, j);
    }
    // 1 december 2026
    let woorden: Vec<&str> = s.split_whitespace().collect();
    for (i, w) in woorden.iter().enumerate() {
        let Ok(d) = w.parse::<u32>() else { continue };
        let Some(mw) = woorden.get(i + 1) else { continue };
        let Some(m) = MAANDEN_NL.iter().position(|x| x == mw)
            .or_else(|| MAANDEN_EN.iter().position(|x| x == mw))
            .or_else(|| MAANDEN_KORT.iter().position(|x| mw.starts_with(x))) else { continue };
        let m = m as u32 + 1;
        let j = woorden.get(i + 2).and_then(|j| j.parse::<i32>().ok()).filter(|j| *j >= 1900);
        return met_jaar(d, m, j);
    }
    None
}

/// "1 december 2026" (of "1 December 2026" in het Engels).
pub fn mooi(d: (i32, u32, u32)) -> String {
    if crate::teksten::taal() == "en" {
        let m = MAANDEN_EN[d.1 as usize - 1];
        let mut c = m.chars();
        let m = c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default();
        format!("{} {m} {}", d.2, d.0)
    } else {
        format!("{} {} {}", d.2, MAANDEN_NL[d.1 as usize - 1], d.0)
    }
}

/// "vandaag", "morgen", "over 5 dagen", "gisteren", "3 dagen geleden".
pub fn wanneer(datum: (i32, u32, u32)) -> String {
    let n = dagnummer(datum) - dagnummer(vandaag());
    match n {
        0 => t("WanneerVandaag"),
        1 => t("WanneerMorgen"),
        -1 => t("WanneerGisteren"),
        n if n > 1 => tf("WanneerOver", &[&n.to_string()]),
        n => tf("WanneerGeleden", &[&(-n).to_string()]),
    }
}

// ------------------------------------------------------------------ opslaan

pub fn laad() -> Vec<Herinnering> {
    let Some(data) = crate::gegevens::lees_versleuteld(BESTAND) else { return Vec::new() };
    let v: Value = serde_json::from_slice(&data).unwrap_or(Value::Null);
    let mut uit: Vec<Herinnering> = v.as_array().map(|a| a.iter().map(|h| Herinnering {
        id: h["id"].as_u64().unwrap_or(0),
        datum: h["datum"].as_str().unwrap_or("").to_string(),
        wat: h["wat"].as_str().unwrap_or("").to_string(),
        document: h["document"].as_str().unwrap_or("").to_string(),
        gemeld: h["gemeld"].as_str().unwrap_or("").to_string(),
    }).filter(|h| lees_iso(&h.datum).is_some()).collect()).unwrap_or_default();
    uit.sort_by(|a, b| a.datum.cmp(&b.datum));
    uit
}

pub fn bewaar(lijst: &[Herinnering]) -> Result<(), String> {
    let v: Vec<Value> = lijst.iter().map(|h| json!({ "id": h.id, "datum": h.datum, "wat": h.wat, "document": h.document, "gemeld": h.gemeld })).collect();
    crate::gegevens::schrijf_versleuteld(BESTAND, Value::Array(v).to_string().as_bytes())?;
    if lijst.is_empty() { verwijder_taak() } else { zet_taak() }
    Ok(())
}

pub fn voeg_toe(datum: (i32, u32, u32), wat: &str, document: &str) -> Result<(), String> {
    let mut lijst = laad();
    let id = lijst.iter().map(|h| h.id).max().unwrap_or(0) + 1;
    lijst.push(Herinnering { id, datum: iso(datum), wat: wat.trim().to_string(), document: document.to_string(), gemeld: String::new() });
    lijst.sort_by(|a, b| a.datum.cmp(&b.datum));
    bewaar(&lijst)
}

pub fn verwijder(id: u64) -> Result<(), String> {
    let mut lijst = laad();
    lijst.retain(|h| h.id != id);
    bewaar(&lijst)
}

/// De herinneringen waarvoor Paper Pilot vandaag moet waarschuwen.
pub fn te_melden() -> Vec<Herinnering> {
    let nu = vandaag();
    let vandaag_iso = iso(nu);
    laad().into_iter().filter(|h| {
        let d = lees_iso(&h.datum).unwrap_or(nu);
        dagnummer(d) - dagnummer(nu) <= DAGEN_VOORAF && h.gemeld != vandaag_iso
    }).collect()
}

/// De zin voor een herinnering, bijvoorbeeld "1 december 2026 (over 3 dagen): Betalen ...".
pub fn zin(h: &Herinnering) -> String {
    let d = lees_iso(&h.datum).unwrap_or_else(vandaag);
    let mut z = format!("{} ({}): {}", mooi(d), wanneer(d), h.wat);
    if !h.document.is_empty() {
        z.push_str(&format!(" - {}", h.document));
    }
    z
}

/// Het venster "Herinnering maken": datum en tekst (alvast ingevuld waar dat kan).
pub fn maak_venster(hwnd: HWND, datum: &str, wat: &str, document: &str) {
    use crate::dialogen;
    let mut velden = vec![(t("HerinneringDatumVraag"), datum.to_string()), (t("HerinneringWatVraag"), wat.to_string())];
    loop {
        let Some(antwoord) = crate::keuzevensters::formulier(hwnd, &t("HerinneringMakenTitel"), &t("HerinneringMakenUitleg"), &velden) else { return };
        let Some(d) = lees_datum(&antwoord[0]) else {
            dialogen::melding(hwnd, &t("HerinneringMakenTitel"), &tf("HerinneringDatumFout", &[&antwoord[0]]), dialogen::ICOON_WAARSCHUWING);
            velden[0].1 = antwoord[0].clone();
            velden[1].1 = antwoord[1].clone();
            continue;
        };
        let wat = if antwoord[1].trim().is_empty() { document.to_string() } else { antwoord[1].clone() };
        match voeg_toe(d, &wat, document) {
            Ok(_) => dialogen::melding(hwnd, &t("HerinneringGemaakt"), &tf("HerinneringGemaaktTekst", &[&mooi(d), &DAGEN_VOORAF.to_string()]), dialogen::ICOON_INFO),
            Err(e) => dialogen::melding(hwnd, &t("OpslaanMislukt"), &e, dialogen::ICOON_WAARSCHUWING),
        }
        return;
    }
}

/// Menu Bestand, Herinneringen: de lijst, een nieuwe maken, of er een weghalen.
pub fn lijst_venster(hwnd: HWND) {
    use crate::dialogen;
    loop {
        let lijst = laad();
        let mut namen = vec![t("HerinneringNieuw")];
        namen.extend(lijst.iter().map(zin));
        let label = if lijst.is_empty() { t("HerinneringenGeen") } else { tf("HerinneringenAantal", &[&lijst.len().to_string()]) };
        let Some(i) = crate::keuzevensters::kies_uit_lijst(hwnd, &t("HerinneringenTitel"), &label, &namen, if lijst.is_empty() { 0 } else { 1 }) else { return };
        if i == 0 {
            maak_venster(hwnd, "", "", "");
            continue;
        }
        let h = &lijst[i - 1];
        if dialogen::ja_nee(hwnd, &t("HerinneringenTitel"), &tf("HerinneringWegVraag", &[&zin(h)]), false) {
            if let Err(e) = verwijder(h.id) {
                dialogen::melding(hwnd, &t("OpslaanMislukt"), &e, dialogen::ICOON_WAARSCHUWING);
            }
        }
    }
}

// ------------------------------------------------------------------ waarschuwen

/// Laat de herinneringen van vandaag zien. Geeft het aantal getoonde herinneringen.
pub fn meld(eigenaar: HWND) -> usize {
    let lijst = te_melden();
    let vandaag_iso = iso(vandaag());
    for h in &lijst {
        let d = lees_iso(&h.datum).unwrap_or_else(vandaag);
        let voorbij = dagnummer(d) < dagnummer(vandaag());
        let titel = t("HerinneringTitel");
        let kop = if voorbij { tf("HerinneringKopVoorbij", &[&mooi(d)]) } else { tf("HerinneringKop", &[&wanneer(d), &mooi(d)]) };
        let mut tekst = h.wat.clone();
        if !h.document.is_empty() {
            tekst.push_str(&format!("\n\n{}", tf("HerinneringUit", &[&h.document])));
        }
        let mut v = crate::dialogen::Vraag::nieuw(&titel, &kop, &tekst);
        v.icoon = crate::dialogen::ICOON_INFO;
        v.opdrachtlinks = true;
        v.knoppen.push(crate::dialogen::Knop { id: 4001, tekst: t("HerinneringGedaan") });
        v.knoppen.push(crate::dialogen::Knop { id: 4002, tekst: t("HerinneringLater") });
        v.standaard = 4002;
        let keuze = v.toon(eigenaar);
        let mut alle = laad();
        if keuze == 4001 {
            alle.retain(|x| x.id != h.id);
        } else if let Some(x) = alle.iter_mut().find(|x| x.id == h.id) {
            x.gemeld = vandaag_iso.clone();
        }
        if let Err(e) = bewaar(&alle) {
            crate::logboek::schrijf(&format!("Herinneringen bewaren: {e}"));
        }
    }
    lijst.len()
}

/// `PaperPilot.exe --herinneringen` (vanuit de geplande taak): alleen een venster als er iets te melden is.
pub fn vanuit_taak() -> i32 {
    if te_melden().is_empty() {
        return 0;
    }
    crate::logboek::schrijf("Herinneringen gemeld vanuit de geplande taak.");
    meld(std::ptr::null_mut());
    0
}

// ------------------------------------------------------------------ geplande taak van Windows

fn schtasks(args: &[&str]) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    let uit = std::process::Command::new("schtasks.exe").args(args).creation_flags(0x0800_0000)
        .stdin(std::process::Stdio::null()).output().map_err(|e| e.to_string())?;
    if uit.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&uit.stderr).trim().to_string())
    }
}

fn xml_tekst(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Maakt (of vernieuwt) de taak die elke dag om 10 uur kijkt of er iets te melden is.
pub fn zet_taak() {
    let Ok(exe) = std::env::current_exe() else { return };
    let xml = format!(r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo><Description>{}</Description></RegistrationInfo>
  <Triggers>
    <CalendarTrigger><StartBoundary>{}T10:00:00</StartBoundary><Enabled>true</Enabled><ScheduleByDay><DaysInterval>1</DaysInterval></ScheduleByDay></CalendarTrigger>
  </Triggers>
  <Principals><Principal id="Author"><LogonType>InteractiveToken</LogonType><RunLevel>LeastPrivilege</RunLevel></Principal></Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <StartWhenAvailable>true</StartWhenAvailable>
    <RunOnlyIfNetworkAvailable>false</RunOnlyIfNetworkAvailable>
    <ExecutionTimeLimit>PT2H</ExecutionTimeLimit>
    <Enabled>true</Enabled>
  </Settings>
  <Actions Context="Author"><Exec><Command>{}</Command><Arguments>--herinneringen</Arguments></Exec></Actions>
</Task>
"#, xml_tekst(&t("HerinneringTaakUitleg")), iso(vandaag()), xml_tekst(&exe.to_string_lossy()));
    let pad = std::env::temp_dir().join(format!("paperpilot-taak-{}.xml", std::process::id()));
    let mut data: Vec<u8> = vec![0xFF, 0xFE];
    for c in xml.replace('\n', "\r\n").encode_utf16() {
        data.extend_from_slice(&c.to_le_bytes());
    }
    if std::fs::write(&pad, data).is_ok() {
        let p = pad.to_string_lossy().to_string();
        match schtasks(&["/Create", "/TN", TAAK, "/XML", &p, "/F"]) {
            Ok(_) => crate::logboek::schrijf("Geplande taak voor herinneringen gemaakt."),
            Err(e) => crate::logboek::schrijf(&format!("Geplande taak maken lukte niet: {e}")),
        }
        let _ = std::fs::remove_file(&pad);
    }
}

pub fn verwijder_taak() {
    let _ = schtasks(&["/Delete", "/TN", TAAK, "/F"]);
}

#[cfg(test)]
mod proeven {
    use super::*;

    #[test]
    fn datums() {
        assert_eq!(lees_datum("1 december 2026"), Some((2026, 12, 1)));
        assert_eq!(lees_datum("15-12-2026"), Some((2026, 12, 15)));
        assert_eq!(lees_datum("2026-02-29"), None);
        assert_eq!(dagnummer((2026, 12, 1)) - dagnummer((2026, 11, 28)), 3);
        assert_eq!(dagnummer((1970, 1, 1)), 0);
    }
}
