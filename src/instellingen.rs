// Paper Pilot - instellingen in %APPDATA%\Paper Pilot\instellingen.txt ("sleutel = waarde").
// Veilig schrijven: eerst naar .nieuw, de oude wordt .vorige, dan pas de nieuwe op zijn plek.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

static WAARDEN: Mutex<BTreeMap<String, String>> = Mutex::new(BTreeMap::new());
static EERSTE: Mutex<bool> = Mutex::new(false);

pub fn map() -> PathBuf {
    let basis = crate::win::omgeving("APPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    basis.join("Paper Pilot")
}

fn bestand() -> PathBuf {
    map().join("instellingen.txt")
}

pub fn laad() {
    let pad = bestand();
    let tekst = fs::read_to_string(&pad).or_else(|_| fs::read_to_string(pad.with_extension("txt.vorige")));
    let mut m = WAARDEN.lock().unwrap();
    match tekst {
        Ok(t) => {
            for regel in t.trim_start_matches('\u{feff}').lines() {
                if let Some((k, v)) = regel.split_once(" = ") {
                    m.insert(k.trim().to_string(), v.trim().to_string());
                }
            }
        }
        Err(_) => *EERSTE.lock().unwrap() = true,
    }
}

/// Waar als Paper Pilot voor het eerst op deze computer (voor deze gebruiker) start.
pub fn eerste_start() -> bool {
    *EERSTE.lock().unwrap()
}

pub fn get(k: &str) -> Option<String> {
    WAARDEN.lock().unwrap().get(k).cloned()
}

pub fn aan(k: &str, standaard: bool) -> bool {
    get(k).map(|v| v == "ja").unwrap_or(standaard)
}

pub fn zet(k: &str, v: &str) {
    WAARDEN.lock().unwrap().insert(k.to_string(), v.to_string());
    bewaar();
}

pub fn zet_aan(k: &str, v: bool) {
    zet(k, if v { "ja" } else { "nee" });
}

fn bewaar() {
    let m = WAARDEN.lock().unwrap().clone();
    let mut tekst = String::from("\u{feff}# Paper Pilot - instellingen\r\n");
    for (k, v) in &m {
        tekst.push_str(&format!("{k} = {v}\r\n"));
    }
    let pad = bestand();
    let _ = fs::create_dir_all(map());
    let nieuw = pad.with_extension("txt.nieuw");
    let vorige = pad.with_extension("txt.vorige");
    if fs::write(&nieuw, tekst).is_ok() {
        if pad.exists() {
            let _ = fs::remove_file(&vorige);
            let _ = fs::rename(&pad, &vorige);
        }
        if fs::rename(&nieuw, &pad).is_err() {
            crate::logboek::schrijf("Instellingen bewaren lukte niet.");
        }
    }
}
