// Paper Pilot - internet en het eigen taalmodel aanspreken met WinHTTP (van Windows zelf).
// Volgt doorverwijzingen (Dropbox, Hugging Face), gebruikt de proxy van Windows, en kan grote downloads hervatten.

use crate::win::w;
use std::fs;
use std::io::Write;
use std::path::Path;
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::Networking::WinHttp::*;

struct Greep(*mut core::ffi::c_void);
impl Drop for Greep {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { WinHttpCloseHandle(self.0) };
        }
    }
}

fn fout(wat: &str) -> String {
    format!("{wat} mislukt ({})", unsafe { GetLastError() })
}

/// Een verzoek. `lichaam` wordt verstuurd (POST) als het er is; `ontvang` krijgt de gegevens in stukken.
/// Geeft de statuscode en de totale lengte (als de server die noemt).
pub fn verzoek(
    methode: &str,
    adres: &str,
    kopregels: &str,
    lichaam: Option<&[u8]>,
    wachttijd_ms: i32,
    ontvang: &mut dyn FnMut(&[u8], u32, Option<u64>) -> Result<(), String>,
) -> Result<(u32, Option<u64>), String> {
    let (veilig, rest) = if let Some(r) = adres.strip_prefix("https://") {
        (true, r)
    } else if let Some(r) = adres.strip_prefix("http://") {
        (false, r)
    } else {
        return Err("onbekend adres".into());
    };
    let (gastheer, pad) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let (host, poort) = match gastheer.rsplit_once(':') {
        Some((h, p)) => (h, p.parse::<u16>().map_err(|_| "poort")?),
        None => (gastheer, if veilig { 443 } else { 80 }),
    };
    unsafe {
        let agent = w(&format!("PaperPilot/{}", crate::VERSIE));
        let toegang = if host == "127.0.0.1" || host == "localhost" { WINHTTP_ACCESS_TYPE_NO_PROXY } else { WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY };
        let sessie = Greep(WinHttpOpen(agent.as_ptr(), toegang, std::ptr::null(), std::ptr::null(), 0));
        if sessie.0.is_null() {
            return Err(fout("WinHttpOpen"));
        }
        WinHttpSetTimeouts(sessie.0, 15000, 15000, wachttijd_ms, wachttijd_ms);
        let h = w(host);
        let verbinding = Greep(WinHttpConnect(sessie.0, h.as_ptr(), poort, 0));
        if verbinding.0.is_null() {
            return Err(fout("verbinden"));
        }
        let m = w(methode);
        let p = w(pad);
        let verzoek = Greep(WinHttpOpenRequest(verbinding.0, m.as_ptr(), p.as_ptr(), std::ptr::null(), std::ptr::null(),
            std::ptr::null(), if veilig { WINHTTP_FLAG_SECURE } else { 0 }));
        if verzoek.0.is_null() {
            return Err(fout("verzoek"));
        }
        let k: Vec<u16> = kopregels.encode_utf16().collect();
        let (lp, ll) = match lichaam {
            Some(b) => (b.as_ptr() as *const core::ffi::c_void, b.len() as u32),
            None => (std::ptr::null(), 0),
        };
        if WinHttpSendRequest(verzoek.0, if k.is_empty() { std::ptr::null() } else { k.as_ptr() }, k.len() as u32, lp, ll, ll, 0) == 0 {
            return Err(fout("versturen"));
        }
        if WinHttpReceiveResponse(verzoek.0, std::ptr::null_mut()) == 0 {
            return Err(fout("antwoord"));
        }
        let mut status = 0u32;
        let mut lengte = 4u32;
        WinHttpQueryHeaders(verzoek.0, WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER, std::ptr::null(),
            &mut status as *mut u32 as *mut _, &mut lengte, std::ptr::null_mut());
        let mut totaal = 0u64;
        let mut l8 = 8u32;
        let heeft_lengte = WinHttpQueryHeaders(verzoek.0, WINHTTP_QUERY_CONTENT_LENGTH | WINHTTP_QUERY_FLAG_NUMBER64, std::ptr::null(),
            &mut totaal as *mut u64 as *mut _, &mut l8, std::ptr::null_mut()) != 0;
        let mut buf = vec![0u8; 256 * 1024];
        loop {
            let mut gelezen = 0u32;
            if WinHttpReadData(verzoek.0, buf.as_mut_ptr() as *mut _, buf.len() as u32, &mut gelezen) == 0 {
                return Err(fout("lezen"));
            }
            if gelezen == 0 {
                break;
            }
            ontvang(&buf[..gelezen as usize], status, if heeft_lengte { Some(totaal) } else { None })?;
        }
        Ok((status, if heeft_lengte { Some(totaal) } else { None }))
    }
}

/// Een klein antwoord helemaal ophalen (bijvoorbeeld JSON).
pub fn haal_tekst(adres: &str) -> Result<String, String> {
    let mut data = Vec::new();
    let (status, _) = verzoek("GET", adres, "", None, 60000, &mut |b, _, _| {
        data.extend_from_slice(b);
        Ok(())
    })?;
    if status != 200 {
        return Err(format!("de server antwoordde {status}"));
    }
    String::from_utf8(data).map_err(|e| e.to_string())
}

/// Een bestand downloaden. Met `hervatten` gaat een afgebroken download (doel + ".part") verder waar hij was.
/// `voortgang` krijgt (binnen, totaal) in bytes.
pub fn download(adres: &str, doel: &Path, hervatten: bool, voortgang: &mut dyn FnMut(u64, Option<u64>), gestopt: &std::sync::atomic::AtomicBool) -> Result<(), String> {
    let deel = doel.with_extension(format!("{}.part", doel.extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_default()));
    let al = if hervatten { fs::metadata(&deel).map(|m| m.len()).unwrap_or(0) } else { 0 };
    let kop = if al > 0 { format!("Range: bytes={al}-\r\n") } else { String::new() };
    let mut bestand = fs::OpenOptions::new().create(true).append(al > 0).write(true).truncate(al == 0).open(&deel).map_err(|e| e.to_string())?;
    let mut binnen = al;
    let (status, _) = verzoek("GET", adres, &kop, None, 120000, &mut |b, status, lengte| {
        if gestopt.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(crate::teksten::t("Gestopt"));
        }
        if status != 200 && status != 206 {
            return Ok(());
        }
        bestand.write_all(b).map_err(|e| e.to_string())?;
        binnen += b.len() as u64;
        voortgang(binnen, lengte.map(|l| l + if status == 206 { al } else { 0 }));
        Ok(())
    })?;
    drop(bestand);
    match status {
        200 | 206 => {}
        416 => {} // al helemaal binnen
        s => {
            if al == 0 {
                let _ = fs::remove_file(&deel);
            }
            return Err(format!("de server antwoordde {s}"));
        }
    }
    if status == 200 && al > 0 {
        // De server begon opnieuw: het bestand bevat dan alleen de nieuwe gegevens niet; opnieuw zonder hervatten.
        return download(adres, doel, false, voortgang, gestopt);
    }
    let _ = fs::remove_file(doel);
    fs::rename(&deel, doel).map_err(|e| e.to_string())
}
