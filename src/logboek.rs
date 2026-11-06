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
