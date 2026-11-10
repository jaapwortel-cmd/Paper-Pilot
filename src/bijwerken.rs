// Paper Pilot - updates uit de Dropbox-map van Jaap.
//
// In de map staat alleen het installatieprogramma ("Paper Pilot 0.0.0.2 installatie.exe").
// Dropbox geeft een hele map als zip (dl=1). Paper Pilot pakt de zip uit, leest het versienummer
// uit elk installatieprogramma zelf en kiest het nieuwste. Is dat nieuwer dan deze versie, dan wordt het
// met beheerdersrechten gestart (een UAC-vraag). Het installatieprogramma ruimt de oude
// programmabestanden op en zet de nieuwe in de plaats; eigen gegevens in %APPDATA% blijven.

use crate::win::w;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Storage::FileSystem::*;
use windows_sys::Win32::UI::Shell::*;
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

/// Alleen bekijken (nooit een link met bewerkrechten: updates draaien met beheerdersrechten).
pub const ADRES: &str = "https://www.dropbox.com/scl/fo/dgj0om0qkz4al43owg91c/AENyj6dSYBH7sdMj1VOE96I?rlkey=vhtmwomkhbzgvhb0kaffpyeex&dl=1";

pub enum Uitkomst {
    Nieuw { versie: String, pad: PathBuf },
    Actueel,
    Fout(String),
}

pub fn tijdelijke_map() -> PathBuf {
    std::env::temp_dir().join("Paper Pilot update")
}

/// Bij de start: de map van een vorige update opruimen.
pub fn ruim_op() {
    let _ = fs::remove_dir_all(tijdelijke_map());
}

fn download(adres: &str, doel: &Path) -> Result<(), String> {
    static NOOIT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    crate::http::download(adres, doel, false, &mut |_, _| {}, &NOOIT)
}

/// Leest het versienummer uit een .exe (de versie-informatie van Windows).
pub fn versie_van(pad: &Path) -> Option<[u32; 4]> {
    let p = w(&pad.to_string_lossy());
    unsafe {
        let mut h = 0u32;
        let grootte = GetFileVersionInfoSizeW(p.as_ptr(), &mut h);
        if grootte == 0 {
            return None;
        }
        let mut buf = vec![0u8; grootte as usize];
        if GetFileVersionInfoW(p.as_ptr(), 0, grootte, buf.as_mut_ptr() as *mut _) == 0 {
            return None;
        }
        let mut info: *mut VS_FIXEDFILEINFO = std::ptr::null_mut();
        let mut lengte = 0u32;
        let wortel = w("\\");
        if VerQueryValueW(buf.as_ptr() as *const _, wortel.as_ptr(), &mut info as *mut _ as *mut _, &mut lengte) == 0 || info.is_null() {
            return None;
        }
        let i = &*info;
        Some([i.dwFileVersionMS >> 16, i.dwFileVersionMS & 0xFFFF, i.dwFileVersionLS >> 16, i.dwFileVersionLS & 0xFFFF])
    }
}

pub fn tekst(v: [u32; 4]) -> String {
    format!("{}.{}.{}.{}", v[0], v[1], v[2], v[3])
}

/// Zoekt een nieuwere versie. Draait op een eigen draad (niet op de draad van het venster).
pub fn zoek() -> Uitkomst {
    match zoek_in(ADRES) {
        Ok(u) => u,
        Err(e) => {
            crate::logboek::schrijf(&format!("Updates zoeken: {e}"));
            Uitkomst::Fout(e)
        }
    }
}

pub fn zoek_in(adres: &str) -> Result<Uitkomst, String> {
    let map = tijdelijke_map();
    let _ = fs::remove_dir_all(&map);
    fs::create_dir_all(&map).map_err(|e| e.to_string())?;
    let zip = map.join("map.zip");
    download(adres, &zip)?;
    let deze = crate::teksten::versie_getallen(crate::VERSIE).unwrap_or([0; 4]);
    let bestand = fs::File::open(&zip).map_err(|e| e.to_string())?;
    let mut archief = match zip::ZipArchive::new(bestand) {
        Ok(a) => a,
        // Een lege map geeft geen geldige zip: dan is er geen update.
        Err(_) if fs::metadata(&zip).map(|m| m.len() < 200).unwrap_or(true) => return Ok(Uitkomst::Actueel),
        Err(e) => return Err(format!("zip: {e}")),
    };
    let mut beste: Option<([u32; 4], PathBuf)> = None;
    for i in 0..archief.len() {
        let mut deel = archief.by_index(i).map_err(|e| e.to_string())?;
        let naam = deel.name().rsplit('/').next().unwrap_or("").to_string();
        let klein = naam.to_lowercase();
        if !(klein.starts_with("paper pilot") && klein.ends_with(".exe")) {
            continue;
        }
        let doel = map.join(&naam);
        let mut inhoud = Vec::new();
        deel.read_to_end(&mut inhoud).map_err(|e| e.to_string())?;
        fs::write(&doel, inhoud).map_err(|e| e.to_string())?;
        if let Some(v) = versie_van(&doel) {
            if beste.as_ref().map(|(b, _)| v > *b).unwrap_or(true) {
                beste = Some((v, doel));
            }
        }
    }
    let _ = fs::remove_file(&zip);
    match beste {
        Some((v, pad)) if v > deze => {
            crate::logboek::schrijf(&format!("Nieuwe versie gevonden: {}", tekst(v)));
            Ok(Uitkomst::Nieuw { versie: tekst(v), pad })
        }
        _ => Ok(Uitkomst::Actueel),
    }
}

/// Start het installatieprogramma; dat vraagt zelf om beheerdersrechten. Daarna moet Paper Pilot zichzelf sluiten.
pub fn installeer(pad: &Path) -> bool {
    let taal = if crate::teksten::taal() == "en" { "english" } else { "dutch" };
    let parameters = w(&format!("/SILENT /SUPPRESSMSGBOXES /NORESTART /CLOSEAPPLICATIONS /LANG={taal}"));
    let bestand = w(&pad.to_string_lossy());
    // Niet "runas": dan start het installatieprogramma Paper Pilot daarna ook als beheerder (runasoriginaluser werkt
    // alleen als het installatieprogramma zelf om rechten vraagt). Als beheerder kon JAWS bij de Verhalenschrijver
    // de tekst van vragen niet lezen (10 nov 2026). Met "open" vraagt Windows om toestemming en start Paper Pilot gewoon.
    let werkwoord = w("open");
    let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
    info.lpVerb = werkwoord.as_ptr();
    info.lpFile = bestand.as_ptr();
    info.lpParameters = parameters.as_ptr();
    info.nShow = SW_SHOWNORMAL;
    let ok = unsafe { ShellExecuteExW(&mut info) } != 0;
    if !ok {
        let fout = unsafe { GetLastError() };
        crate::logboek::schrijf(&format!("Installatieprogramma starten lukte niet ({fout})."));
    } else {
        crate::logboek::schrijf("Installatieprogramma gestart voor de update.");
    }
    ok
}
