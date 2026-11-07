// Paper Pilot - voorlezen met een stem van Windows (SAPI), voor wie geen schermlezer gebruikt.
//
// Eerst een stem in de taal van het programma: de nieuwere stemmen van Windows (OneCore, bijvoorbeeld "Microsoft Frank"),
// dan de gewone SAPI-stemmen, en anders de standaardstem. Voorlezen gaat op de achtergrond (SPF_ASYNC); met
// stop() houdt het meteen op. positie() zegt bij welke zin de stem is, zodat het leesvenster kan meelopen.

use windows::core::{PCWSTR, HSTRING};
use windows::Win32::Media::Speech::*;
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};
use std::cell::RefCell;

const ONECORE: &str = "HKEY_LOCAL_MACHINE\\SOFTWARE\\Microsoft\\Speech_OneCore\\Voices";
const SAPI: &str = "HKEY_LOCAL_MACHINE\\SOFTWARE\\Microsoft\\Speech\\Voices";

thread_local! {
    static STEM: RefCell<Option<ISpVoice>> = const { RefCell::new(None) };
}

/// Windows-taalcode (hexadecimaal, zoals SAPI hem wil) van de programmataal.
fn taalcode() -> &'static str {
    if crate::teksten::taal() == "en" { "409;809" } else { "413;813" }
}

/// Zoekt een stem in een categorie (OneCore of SAPI) met een van de taalcodes.
fn zoek_stem(categorie: &str) -> Option<ISpObjectToken> {
    unsafe {
        let cat: ISpObjectTokenCategory = CoCreateInstance(&SpObjectTokenCategory, None, CLSCTX_ALL).ok()?;
        cat.SetId(&HSTRING::from(categorie), false).ok()?;
        for code in taalcode().split(';') {
            let eis = HSTRING::from(format!("Language={code}"));
            let Ok(lijst) = cat.EnumTokens(&eis, PCWSTR::null()) else { continue };
            let mut token: [Option<ISpObjectToken>; 1] = [None];
            let mut n = 0u32;
            if lijst.Next(1, token.as_mut_ptr(), Some(&mut n)).is_ok() && n == 1 {
                if let Some(t) = token[0].take() {
                    return Some(t);
                }
            }
        }
        None
    }
}

/// De naam van een stem (voor het logboek en de proef).
fn naam(token: &ISpObjectToken) -> String {
    unsafe { token.GetStringValue(PCWSTR::null()).ok().map(|p| p.to_string().unwrap_or_default()).unwrap_or_default() }
}

/// Maakt de stem (één keer per draad).
fn stem() -> Result<ISpVoice, String> {
    if let Some(s) = STEM.with(|s| s.borrow().clone()) {
        return Ok(s);
    }
    crate::beeld::com_start();
    let s: ISpVoice = unsafe { CoCreateInstance(&SpVoice, None, CLSCTX_ALL) }.map_err(|e| format!("{e}"))?;
    if let Some(token) = zoek_stem(ONECORE).or_else(|| zoek_stem(SAPI)) {
        let n = naam(&token);
        if unsafe { s.SetVoice(&token) }.is_ok() {
            crate::logboek::schrijf(&format!("Stem voor voorlezen: {n}"));
        }
    }
    STEM.with(|x| *x.borrow_mut() = Some(s.clone()));
    Ok(s)
}

/// De snelheid uit de instellingen: -10 (heel langzaam) tot 10 (heel snel).
pub fn snelheid() -> i32 {
    crate::instellingen::get("voorlees_snelheid").and_then(|v| v.parse().ok()).unwrap_or(0i32).clamp(-10, 10)
}

/// Begint met voorlezen (wat er al werd voorgelezen, stopt eerst).
pub fn lees_voor(tekst: &str) -> Result<(), String> {
    let s = stem()?;
    let w: Vec<u16> = tekst.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let _ = s.SetRate(snelheid());
        s.Speak(PCWSTR(w.as_ptr()), (SPF_ASYNC.0 | SPF_PURGEBEFORESPEAK.0 | SPF_IS_NOT_XML.0) as u32, None).map_err(|e| format!("{e}"))
    }
}

pub fn stop() {
    if let Some(s) = STEM.with(|s| s.borrow().clone()) {
        unsafe {
            let _ = s.Speak(PCWSTR::null(), SPF_PURGEBEFORESPEAK.0 as u32, None);
        }
    }
}

/// Is de stem nog aan het voorlezen?
pub fn bezig() -> bool {
    status().map(|st| st.dwRunningState == SPRS_IS_SPEAKING.0 as u32).unwrap_or(false)
}

/// Waar de stem nu is: het begin van de zin (in UTF-16-tekens van de voorgelezen tekst).
pub fn positie() -> Option<usize> {
    status().filter(|st| st.dwRunningState == SPRS_IS_SPEAKING.0 as u32).map(|st| st.ulInputSentPos as usize)
}

fn status() -> Option<SPVOICESTATUS> {
    let s = STEM.with(|s| s.borrow().clone())?;
    let mut st = SPVOICESTATUS::default();
    let mut bladwijzer = windows::core::PWSTR::null();
    unsafe { s.GetStatus(&mut st, &mut bladwijzer) }.ok()?;
    Some(st)
}

/// Voor de proef: welke stemmen zijn er, en lukt het voorlezen?
pub fn proef() -> String {
    let mut uit = String::new();
    crate::beeld::com_start();
    for (soort, cat) in [("OneCore", ONECORE.to_string()), ("SAPI", SAPI.to_string())] {
        match zoek_stem(&cat) {
            Some(t) => uit.push_str(&format!("{soort}: {}\r\n", naam(&t))),
            None => uit.push_str(&format!("{soort}: geen stem in deze taal\r\n")),
        }
    }
    match lees_voor("Paper Pilot.") {
        Ok(_) => {
            for _ in 0..50 {
                if !bezig() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            uit.push_str("Voorlezen: gelukt\r\n");
        }
        Err(e) => uit.push_str(&format!("Voorlezen: FOUT {e}\r\n")),
    }
    uit
}
