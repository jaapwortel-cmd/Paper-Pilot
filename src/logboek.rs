// Paper Pilot - logboek in %APPDATA%\Paper Pilot\logboek.txt.
// Alleen op de eigen computer. Het gaat alleen naar de maker als de gebruiker dat zelf kiest.

use std::fs::{self, OpenOptions};
use std::io::Write;
use windows_sys::Win32::System::SystemInformation::GetLocalTime;

pub fn bestand() -> std::path::PathBuf {
    crate::instellingen::map().join("logboek.txt")
}

pub fn nu() -> String {
    let mut t = unsafe { std::mem::zeroed() };
    unsafe { GetLocalTime(&mut t) };
    let t: windows_sys::Win32::Foundation::SYSTEMTIME = t;
    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond)
}

pub fn schrijf(tekst: &str) {
    let pad = bestand();
    let _ = fs::create_dir_all(crate::instellingen::map());
    if fs::metadata(&pad).map(|m| m.len() > 1_000_000).unwrap_or(false) {
        let _ = fs::rename(&pad, pad.with_extension("txt.vorige"));
    }
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&pad) {
        let _ = write!(f, "{} {}\r\n", nu(), tekst);
    }
}

/// Fouten die het programma laten stoppen komen ook in het logboek.
pub fn vang_fouten() {
    std::panic::set_hook(Box::new(|info| {
        schrijf(&format!("FOUT: {info}"));
    }));
}

/// Een tekst uit het register van Windows (HKLM), bijvoorbeeld de Windows-versie.
fn register(sleutel: &str, naam: &str) -> String {
    use windows_sys::Win32::System::Registry::*;
    let s = crate::win::w(sleutel);
    let n = crate::win::w(naam);
    let mut buf = vec![0u16; 256];
    let mut grootte = (buf.len() * 2) as u32;
    let r = unsafe {
        RegGetValueW(HKEY_LOCAL_MACHINE, s.as_ptr(), n.as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), buf.as_mut_ptr() as *mut _, &mut grootte)
    };
    if r != 0 {
        return String::new();
    }
    let len = buf.iter().position(|&c| c == 0).unwrap_or(0);
    String::from_utf16_lossy(&buf[..len])
}

/// De versie van Windows, bijvoorbeeld "Windows 11 Home 24H2 (build 26100)".
pub fn windows_versie() -> String {
    let k = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";
    let mut naam = register(k, "ProductName");
    let build = register(k, "CurrentBuild");
    // Windows 11 noemt zich in het register nog "Windows 10".
    if build.parse::<u32>().unwrap_or(0) >= 22000 {
        naam = naam.replace("Windows 10", "Windows 11");
    }
    let versie = register(k, "DisplayVersion");
    let delen: Vec<&str> = [naam.trim(), versie.trim()].into_iter().filter(|d| !d.is_empty()).collect();
    format!("{} (build {build})", delen.join(" "))
}

/// Het hele logboek om te lezen of te mailen: eerst wat gegevens over de computer, dan het vorige en het huidige logboek.
pub fn inhoud() -> String {
    let mut uit = format!("Paper Pilot {}\r\n{}\r\nTaal: {}\r\nGemaakt: {}\r\n\r\n", crate::VERSIE, windows_versie(), crate::teksten::taal(), nu());
    let vorige = fs::read(bestand().with_extension("txt.vorige")).unwrap_or_default();
    // Van het vorige logboek alleen het laatste stuk (hooguit 300 kB).
    let begin = vorige.len().saturating_sub(300_000);
    uit.push_str(&String::from_utf8_lossy(&vorige[begin..]));
    uit.push_str(&String::from_utf8_lossy(&fs::read(bestand()).unwrap_or_default()));
    uit
}

/// Zet het logboek in een los bestand om mee te sturen ("Paper Pilot logboek.txt" in de tijdelijke map).
pub fn als_bijlage() -> Result<std::path::PathBuf, String> {
    let map = std::env::temp_dir().join("Paper Pilot");
    fs::create_dir_all(&map).map_err(|e| e.to_string())?;
    let pad = map.join("Paper Pilot logboek.txt");
    let mut data = vec![0xEF, 0xBB, 0xBF];
    data.extend_from_slice(inhoud().as_bytes());
    fs::write(&pad, data).map_err(|e| e.to_string())?;
    Ok(pad)
}
