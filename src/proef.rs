// Paper Pilot - proeven voor GitHub (bij elke bouwronde) en schermafbeeldingen.
//
//   PaperPilot.exe --proefstart verslag.txt      controleert teksten, documenten, het venster en de namen
//   PaperPilot.exe --schermafbeelding foto.bmp   maakt een schermafbeelding van het hoofdvenster
//   PaperPilot.exe --proef-updates verslag.txt   kijkt in de Dropbox-map (alleen ter informatie)

use crate::win::*;
use std::fmt::Write as _;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::Storage::Xps::{PrintWindow, PRINT_WINDOW_FLAGS};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

pub fn proefstart(verslag: &str) -> i32 {
    let mut uit = String::new();
    let mut fouten = 0;
    let mut fout = |uit: &mut String, s: String| {
        fouten += 1;
        let _ = writeln!(uit, "FOUT: {s}");
    };
    let _ = writeln!(uit, "Paper Pilot {} - proefstart", crate::VERSIE);

    // 1. Elke tekst bestaat in het Nederlands en het Engels, en is niet leeg.
    let nl = crate::teksten::nl();
    let en = crate::teksten::en();
    for k in nl.keys() {
        if !en.contains_key(k) {
            fout(&mut uit, format!("tekst '{k}' ontbreekt in en.txt"));
        }
    }
    for k in en.keys() {
        if !nl.contains_key(k) {
            fout(&mut uit, format!("tekst '{k}' ontbreekt in nl.txt"));
        }
    }
    for (k, v) in nl.iter().chain(en.iter()) {
        if v.trim().is_empty() {
            fout(&mut uit, format!("tekst '{k}' is leeg"));
        }
    }
    let _ = writeln!(uit, "Teksten: {} in het Nederlands, {} in het Engels.", nl.len(), en.len());

    // 2. Handleiding, privacyverklaring en wijzigingen in beide talen; deze versie staat in de wijzigingen.
    for taal in ["nl", "en"] {
        crate::teksten::zet_taal(taal);
        for doc in ["handleiding", "privacy", "wijzigingen"] {
            if crate::teksten::document(doc).trim().len() < 200 {
                fout(&mut uit, format!("{doc} ({taal}) is (bijna) leeg"));
            }
        }
        if crate::teksten::wijzigingen_sinds("0.0.0.0").is_empty()
            || !crate::teksten::document("wijzigingen").contains(crate::VERSIE)
        {
            fout(&mut uit, format!("wijzigingen ({taal}) noemen versie {} niet", crate::VERSIE));
        }
        if !crate::teksten::document("privacy").contains(crate::MAIL) {
            fout(&mut uit, format!("privacyverklaring ({taal}) noemt het mailadres niet"));
        }
    }
    crate::teksten::zet_taal("nl");
    if crate::teksten::versie_getallen("0.0.0.2") <= crate::teksten::versie_getallen("0.0.0.1") {
        fout(&mut uit, "versies vergelijken werkt niet".into());
    }

    // 3. Het hoofdvenster: menubalk met drie menu's, en elk onderdeel met focus heeft een naam.
    for taal in ["nl", "en"] {
        crate::teksten::zet_taal(taal);
        let opties = crate::Opties { demo: true, taal: Some(taal.into()), ..Default::default() };
        let hwnd = crate::hoofdvenster::maak(opties);
        if hwnd.is_null() {
            fout(&mut uit, format!("hoofdvenster ({taal}) kon niet worden gemaakt"));
            continue;
        }
        let menu = unsafe { GetMenu(hwnd) };
        let n = unsafe { GetMenuItemCount(menu) };
        if n != 3 {
            fout(&mut uit, format!("menubalk ({taal}) heeft {n} menu's in plaats van 3"));
        }
        let mut kinderen: Vec<HWND> = Vec::new();
        unsafe extern "system" fn verzamel(h: HWND, lp: LPARAM) -> windows_sys::core::BOOL {
            (*(lp as *mut Vec<HWND>)).push(h);
            1
        }
        unsafe { EnumChildWindows(hwnd, Some(verzamel), &mut kinderen as *mut _ as LPARAM) };
        let mut met_focus = 0;
        for k in &kinderen {
            let stijl = unsafe { GetWindowLongW(*k, GWL_STYLE) } as u32;
            if stijl & WS_TABSTOP != 0 {
                met_focus += 1;
                let naam = lees_tekst(*k);
                if naam.trim().is_empty() {
                    fout(&mut uit, format!("onderdeel zonder naam in het hoofdvenster ({taal})"));
                } else {
                    let _ = writeln!(uit, "Onderdeel ({taal}): {naam}");
                }
            }
        }
        if met_focus < 2 {
            fout(&mut uit, format!("hoofdvenster ({taal}) heeft {met_focus} onderdelen met focus"));
        }
        unsafe { DestroyWindow(hwnd) };
        // Berichten afhandelen tot het venster weg is.
        let mut msg: MSG = unsafe { std::mem::zeroed() };
        while unsafe { PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) } != 0 {
            if msg.message == WM_QUIT {
                break;
            }
            unsafe {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }

    // Tekstherkenning van Windows (ter informatie: de bouwcomputer heeft alleen Engels).
    crate::beeld::com_start();
    match crate::herkennen::beschikbaar() {
        Ok(taal) => { let _ = writeln!(uit, "Tekstherkenning: beschikbaar ({taal})"); }
        Err(e) => { let _ = writeln!(uit, "Tekstherkenning: niet beschikbaar ({e})"); }
    }
    let _ = writeln!(uit, "{}", if fouten == 0 { "GESLAAGD".to_string() } else { format!("MISLUKT: {fouten} fout(en)") });
    let _ = std::fs::write(verslag, uit);
    if fouten == 0 { 0 } else { 1 }
}

/// Leest een document zoals Paper Pilot het toont, en schrijft de tekst, de koppen en de pagina's op.
pub fn proef_lezen(bestand: &str, verslag: &str) -> i32 {
    crate::beeld::com_start();
    let uit = match crate::lezen::open(std::path::Path::new(bestand), &|_| {}) {
        Ok(d) => {
            let mut s = format!("Bestand: {}\r\nPagina's: {}\r\nZonder tekst: {:?}\r\nInvulvelden: {}\r\nAfbeeldingen: {}\r\n", d.naam, d.paginas.len(), d.zonder_tekst, d.invulvelden, d.beelden.len());
            for m in &d.meldingen {
                s.push_str(&format!("Melding: {m}\r\n"));
            }
            for (k, b, e) in &d.koppen {
                s.push_str(&format!("Kop: {k} ({b}-{e})\r\n"));
            }
            // De plaatsen moeten precies kloppen met de tekst (in UTF-16, zoals het tekstvak telt).
            let utf16: Vec<u16> = d.tekst.encode_utf16().collect();
            for (k, b, e) in &d.koppen {
                if String::from_utf16_lossy(&utf16[*b..*e]) != *k {
                    s.push_str(&format!("FOUT: kop '{k}' staat niet op zijn plaats\r\n"));
                }
            }
            if !d.beelden.is_empty() {
                // Doorzoekbare PDF maken en weer lezen: de tekst moet erin zitten.
                let pdf = format!("{verslag}.pdf");
                match crate::pdf_maken::schrijf(std::path::Path::new(&pdf), &d.beelden, &d.beeld_regels) {
                    Ok(_) => match crate::lezen::open(std::path::Path::new(&pdf), &|_| {}) {
                        Ok(terug) => {
                            let woorden: Vec<&str> = terug.tekst.split_whitespace().collect();
                            s.push_str(&format!("PDF met tekst: {} woorden, {} pagina's zonder tekst\r\n", woorden.len(), terug.zonder_tekst.len()));
                            s.push_str(&format!("PDF-tekst: {}\r\n", woorden.iter().take(25).cloned().collect::<Vec<_>>().join(" ")));
                        }
                        Err(e) => s.push_str(&format!("FOUT: PDF teruglezen: {e}\r\n")),
                    },
                    Err(e) => s.push_str(&format!("FOUT: PDF maken: {e}\r\n")),
                }
                let doel = format!("{verslag}.png");
                match crate::beeld::schrijf(std::path::Path::new(&doel), &d.beelden) {
                    Ok(_) => s.push_str(&format!("Afbeelding opgeslagen: {doel}\r\n")),
                    Err(e) => s.push_str(&format!("FOUT: afbeelding opslaan: {e}\r\n")),
                }
            }
            s.push_str("----\r\n");
            s.push_str(&d.tekst);
            s
        }
        Err(e) => format!("FOUT: {e}"),
    };
    let fout = uit.contains("FOUT:");
    let _ = std::fs::write(verslag, format!("\u{feff}{uit}"));
    if fout { 1 } else { 0 }
}

/// Downloadt de motor en een (klein) beeldmodel naar een tijdelijke map en laat het een afbeelding lezen en beschrijven.
pub fn proef_beeldmodel(hfmap: &str, afbeelding: &str, verslag: &str) -> i32 {
    crate::beeld::com_start();
    std::env::set_var("PAPERPILOT_HFMAP", hfmap);
    if std::env::var_os("PAPERPILOT_TAALMODEL").is_none() {
        std::env::set_var("PAPERPILOT_TAALMODEL", std::env::temp_dir().join("pp-taalmodel"));
    }
    let mut uit = String::new();
    let v = crate::dialogen::Voortgang::nieuw();
    let begin = std::time::Instant::now();
    match crate::beeldmodel::installeer(&v) {
        Ok(_) => uit.push_str(&format!("Downloaden gelukt in {} s\r\n", begin.elapsed().as_secs())),
        Err(e) => {
            let _ = std::fs::write(verslag, format!("FOUT: downloaden: {e}"));
            return 1;
        }
    }
    let beelden = match crate::beeld::lees(std::path::Path::new(afbeelding)) {
        Ok(b) => b,
        Err(e) => {
            let _ = std::fs::write(verslag, format!("FOUT: afbeelding: {e}"));
            return 1;
        }
    };
    let mut fout = false;
    for (naam, soort) in [("Handschrift", crate::beeldmodel::Soort::Handschrift), ("Beschrijven", crate::beeldmodel::Soort::Beschrijven)] {
        let begin = std::time::Instant::now();
        match crate::beeldmodel::vraag(&beelden[0], soort) {
            Ok(t) => uit.push_str(&format!("== {naam} ({} s)\r\n{t}\r\n", begin.elapsed().as_secs())),
            Err(e) => {
                fout = true;
                uit.push_str(&format!("FOUT: {naam}: {e}\r\n"));
            }
        }
    }
    crate::beeldmodel::stop();
    let log = std::fs::read_to_string(crate::beeldmodel::map().join("motor.log")).unwrap_or_default();
    uit.push_str("== motor.log (einde)\r\n");
    uit.push_str(&log.chars().rev().take(3000).collect::<Vec<_>>().into_iter().rev().collect::<String>());
    let _ = std::fs::write(verslag, format!("\u{feff}{uit}"));
    if fout { 1 } else { 0 }
}

/// Zoekt scanners (ter informatie; op de bouwcomputer zijn er geen).
pub fn proef_scanners(verslag: &str) -> i32 {
    let lijst = crate::scanner::zoek();
    let mut uit = format!("Scanners gevonden: {}\r\n", lijst.len());
    for s in &lijst {
        uit.push_str(&format!("- {} (glasplaat {}, invoer {})\r\n", s.naam, s.glasplaat, s.invoer));
    }
    let _ = std::fs::write(verslag, uit);
    0
}

/// Alleen ter informatie: wat staat er nu in de Dropbox-map?
pub fn proef_updates(verslag: &str) -> i32 {
    let mut tekst = match crate::bijwerken::zoek_in(crate::bijwerken::ADRES) {
        Ok(crate::bijwerken::Uitkomst::Nieuw { versie, .. }) => format!("Nieuwere versie in de map: {versie}\r\n"),
        Ok(crate::bijwerken::Uitkomst::Actueel) => format!("Geen nieuwere versie dan {} in de map.\r\n", crate::VERSIE),
        Ok(crate::bijwerken::Uitkomst::Fout(e)) | Err(e) => format!("Fout: {e}\r\n"),
    };
    // Welke installatieprogramma's er in de map stonden, met hun versie.
    if let Ok(lijst) = std::fs::read_dir(crate::bijwerken::tijdelijke_map()) {
        for b in lijst.flatten() {
            let v = crate::bijwerken::versie_van(&b.path()).map(crate::bijwerken::tekst).unwrap_or_else(|| "?".into());
            tekst.push_str(&format!("In de map: {} (versie {v})\r\n", b.file_name().to_string_lossy()));
        }
    }
    let _ = std::fs::write(verslag, tekst);
    crate::bijwerken::ruim_op();
    0
}

/// Schermafbeelding van een venster als BMP (de workflow maakt er PNG van).
pub fn schermafbeelding(hwnd: HWND, pad: &str) -> Result<(), String> {
    unsafe {
        let mut rc: RECT = std::mem::zeroed();
        GetWindowRect(hwnd, &mut rc);
        let (b, h) = (rc.right - rc.left, rc.bottom - rc.top);
        let scherm = GetDC(std::ptr::null_mut());
        let dc = CreateCompatibleDC(scherm);
        let bmp = CreateCompatibleBitmap(scherm, b, h);
        let oud = SelectObject(dc, bmp as _);
        PrintWindow(hwnd, dc, 2 as PRINT_WINDOW_FLAGS);
        // Gaf PrintWindow een leeg vlak (bijvoorbeeld onder Wine), dan het scherm zelf fotograferen.
        let eerste = GetPixel(dc, b / 2, h / 2);
        let leeg = (0..20).all(|i| GetPixel(dc, (b * i) / 20, (h * (20 - i)) / 21) == eerste);
        if leeg {
            BitBlt(dc, 0, 0, b, h, scherm, rc.left, rc.top, SRCCOPY);
        }
        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = b;
        info.bmiHeader.biHeight = h;
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        info.bmiHeader.biCompression = BI_RGB;
        let mut punten = vec![0u8; (b * h * 4) as usize];
        SelectObject(dc, oud);
        let r = GetDIBits(dc, bmp, 0, h as u32, punten.as_mut_ptr() as *mut _, &mut info, DIB_RGB_COLORS);
        DeleteObject(bmp as _);
        DeleteDC(dc);
        ReleaseDC(std::ptr::null_mut(), scherm);
        if r == 0 {
            return Err("GetDIBits lukte niet".into());
        }
        let mut bestand = Vec::with_capacity(54 + punten.len());
        bestand.extend_from_slice(b"BM");
        bestand.extend_from_slice(&((54 + punten.len()) as u32).to_le_bytes());
        bestand.extend_from_slice(&0u32.to_le_bytes());
        bestand.extend_from_slice(&54u32.to_le_bytes());
        let kop = std::slice::from_raw_parts(&info.bmiHeader as *const _ as *const u8, 40);
        bestand.extend_from_slice(kop);
        bestand.extend_from_slice(&punten);
        std::fs::write(pad, bestand).map_err(|e| e.to_string())
    }
}
