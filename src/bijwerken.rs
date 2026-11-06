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

/// Haalt een bestand op met WinHTTP (volgt doorverwijzingen, zoals die van Dropbox; gebruikt de proxy van Windows).
fn download(adres: &str, doel: &Path) -> Result<(), String> {
    use std::io::Write;
    use windows_sys::Win32::Networking::WinHttp::*;
    let rest = adres.strip_prefix("https://").ok_or("alleen https")?;
    let (host, pad) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    struct Greep(*mut core::ffi::c_void);
    impl Drop for Greep {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe { WinHttpCloseHandle(self.0) };
            }
        }
    }
    let fout = |wat: &str| format!("{wat} mislukt ({})", unsafe { GetLastError() });
    unsafe {
        let agent = w(&format!("PaperPilot/{}", crate::VERSIE));
        let sessie = Greep(WinHttpOpen(agent.as_ptr(), WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, std::ptr::null(), std::ptr::null(), 0));
        if sessie.0.is_null() {
            return Err(fout("WinHttpOpen"));
        }
        let h = w(host);
        let verbinding = Greep(WinHttpConnect(sessie.0, h.as_ptr(), 443, 0));
        if verbinding.0.is_null() {
            return Err(fout("verbinden"));
        }
        let get = w("GET");
        let p = w(pad);
        let verzoek = Greep(WinHttpOpenRequest(verbinding.0, get.as_ptr(), p.as_ptr(), std::ptr::null(), std::ptr::null(),
            std::ptr::null(), WINHTTP_FLAG_SECURE));
        if verzoek.0.is_null() {
            return Err(fout("verzoek"));
        }
        if WinHttpSendRequest(verzoek.0, std::ptr::null(), 0, std::ptr::null(), 0, 0, 0) == 0 {
            return Err(fout("versturen"));
        }
        if WinHttpReceiveResponse(verzoek.0, std::ptr::null_mut()) == 0 {
            return Err(fout("antwoord"));
        }
        let mut status = 0u32;
        let mut lengte = 4u32;
        WinHttpQueryHeaders(verzoek.0, WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER, std::ptr::null(),
            &mut status as *mut u32 as *mut _, &mut lengte, std::ptr::null_mut());
        if status != 200 {
            return Err(format!("de server antwoordde {status}"));
        }
        let mut bestand = fs::File::create(doel).map_err(|e| e.to_string())?;
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let mut gelezen = 0u32;
            if WinHttpReadData(verzoek.0, buf.as_mut_ptr() as *mut _, buf.len() as u32, &mut gelezen) == 0 {
                return Err(fout("lezen"));
            }
            if gelezen == 0 {
                break;
            }
            bestand.write_all(&buf[..gelezen as usize]).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
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

/// Start het installatieprogramma met beheerdersrechten. Daarna moet Paper Pilot zichzelf sluiten.
pub fn installeer(pad: &Path) -> bool {
    let taal = if crate::teksten::taal() == "en" { "english" } else { "dutch" };
    let parameters = w(&format!("/SILENT /SUPPRESSMSGBOXES /NORESTART /CLOSEAPPLICATIONS /LANG={taal}"));
    let bestand = w(&pad.to_string_lossy());
    let werkwoord = w("runas");
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
