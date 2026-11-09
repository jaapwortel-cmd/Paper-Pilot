// Paper Pilot - afdrukken op elke printer die Windows kent, met het gewone afdrukvenster van Windows.
// Tekst: lettertype 12 punten, marges van 2 cm, onderaan de naam en het paginanummer.
// Scans en foto's: elke afbeelding op een eigen pagina, zo groot mogelijk zonder vervormen.

use crate::beeld::Beeld;
use crate::win::*;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::Storage::Xps::*;
use windows_sys::Win32::UI::Controls::Dialogs::*;

/// Het afdrukvenster van Windows. Geeft de printer (als tekenvlak) of None bij Annuleren.
fn kies_printer(eigenaar: HWND) -> Option<HDC> {
    let mut pd: PRINTDLGW = unsafe { std::mem::zeroed() };
    pd.lStructSize = std::mem::size_of::<PRINTDLGW>() as u32;
    pd.hwndOwner = eigenaar;
    pd.Flags = PD_RETURNDC | PD_NOSELECTION | PD_NOPAGENUMS | PD_USEDEVMODECOPIESANDCOLLATE;
    pd.nCopies = 1;
    // De standaardprinter van Paper Pilot staat al klaar in het venster.
    if let Some(naam) = crate::instellingen::get("standaard_printer").filter(|n| !n.is_empty()) {
        pd.hDevNames = maak_devnames(&naam);
    }
    if unsafe { PrintDlgW(&mut pd) } == 0 || pd.hDC.is_null() {
        unsafe {
            if !pd.hDevNames.is_null() {
                GlobalFree(pd.hDevNames as _);
            }
            if !pd.hDevMode.is_null() {
                GlobalFree(pd.hDevMode as _);
            }
        }
        return None;
    }
    unsafe {
        // De gekozen printer onthouden voor de volgende keer.
        if let Some(naam) = lees_devnames(pd.hDevNames) {
            if crate::instellingen::get("standaard_printer").as_deref() != Some(naam.as_str()) {
                crate::instellingen::zet("standaard_printer", &naam);
                crate::logboek::schrijf(&format!("Standaardprinter: {naam}"));
            }
        }
        if !pd.hDevMode.is_null() {
            GlobalFree(pd.hDevMode as _);
        }
        if !pd.hDevNames.is_null() {
            GlobalFree(pd.hDevNames as _);
        }
    }
    Some(pd.hDC)
}

/// Een DEVNAMES-blok met alleen de naam van de printer (voor PrintDlgW).
fn maak_devnames(printer: &str) -> *mut core::ffi::c_void {
    use windows_sys::Win32::System::Memory::*;
    let naam: Vec<u16> = printer.encode_utf16().chain(std::iter::once(0)).collect();
    // DEVNAMES: vier u16-verwijzingen (in tekens), daarna: lege driver, de naam, lege uitvoer.
    let mut data: Vec<u16> = vec![4, 5, 5 + naam.len() as u16, 0, 0];
    data.extend_from_slice(&naam);
    data.push(0);
    unsafe {
        let h = GlobalAlloc(GMEM_MOVEABLE, data.len() * 2);
        if h.is_null() {
            return std::ptr::null_mut();
        }
        let p = GlobalLock(h) as *mut u16;
        if !p.is_null() {
            std::ptr::copy_nonoverlapping(data.as_ptr(), p, data.len());
            GlobalUnlock(h);
        }
        h as _
    }
}

/// De naam van de printer uit een DEVNAMES-blok.
unsafe fn lees_devnames(h: *mut core::ffi::c_void) -> Option<String> {
    use windows_sys::Win32::System::Memory::*;
    if h.is_null() {
        return None;
    }
    let p = GlobalLock(h as _) as *const u16;
    if p.is_null() {
        return None;
    }
    let start = p.add(*p.add(1) as usize);
    let mut n = 0;
    while *start.add(n) != 0 && n < 1024 {
        n += 1;
    }
    let naam = String::from_utf16_lossy(std::slice::from_raw_parts(start, n));
    GlobalUnlock(h as _);
    if naam.is_empty() { None } else { Some(naam) }
}

fn begin(dc: HDC, naam: &str) -> bool {
    let n = w(&format!("{naam} - Paper Pilot"));
    let info = DOCINFOW { cbSize: std::mem::size_of::<DOCINFOW>() as i32, lpszDocName: n.as_ptr(), lpszOutput: std::ptr::null(),
        lpszDatatype: std::ptr::null(), fwType: 0 };
    unsafe { StartDocW(dc, &info) > 0 }
}

/// Drukt tekst af. Geeft Some(aantal pagina's), of None als de gebruiker annuleert.
pub fn tekst(eigenaar: HWND, naam: &str, tekst: &str) -> Option<Result<usize, String>> {
    let dc = kies_printer(eigenaar)?;
    Some(unsafe { tekst_op(dc, naam, tekst) })
}

unsafe fn tekst_op(dc: HDC, naam: &str, tekst: &str) -> Result<usize, String> {
    let dpi_x = GetDeviceCaps(dc, LOGPIXELSX as _);
    let dpi_y = GetDeviceCaps(dc, LOGPIXELSY as _);
    let breed = GetDeviceCaps(dc, HORZRES as _);
    let hoog = GetDeviceCaps(dc, VERTRES as _);
    let marge_x = dpi_x * 2 / 3; // ongeveer 1,7 cm (de printer heeft zelf ook al een rand)
    let marge_y = dpi_y * 2 / 3;
    let letter = CreateFontW(-(12 * dpi_y / 72), 0, 0, 0, 400, 0, 0, 0, DEFAULT_CHARSET as u32, 0, 0, 0, 0, w("Segoe UI").as_ptr());
    let klein = CreateFontW(-(9 * dpi_y / 72), 0, 0, 0, 400, 0, 0, 0, DEFAULT_CHARSET as u32, 0, 0, 0, 0, w("Segoe UI").as_ptr());
    let oud = SelectObject(dc, letter as _);
    let mut tm: TEXTMETRICW = std::mem::zeroed();
    GetTextMetricsW(dc, &mut tm);
    let regelhoogte = tm.tmHeight + tm.tmExternalLeading;
    let bruikbaar = breed - 2 * marge_x;
    let breedte = |s: &str| -> i32 {
        let t: Vec<u16> = s.encode_utf16().collect();
        let mut maat = SIZE { cx: 0, cy: 0 };
        GetTextExtentPoint32W(dc, t.as_ptr(), t.len() as i32, &mut maat);
        maat.cx
    };
    // Regels afbreken op woorden.
    let mut regels: Vec<String> = Vec::new();
    for alinea in tekst.replace("\r\n", "\n").split('\n') {
        let mut huidig = String::new();
        for woord in alinea.split(' ') {
            let proef = if huidig.is_empty() { woord.to_string() } else { format!("{huidig} {woord}") };
            if breedte(&proef) > bruikbaar && !huidig.is_empty() {
                regels.push(std::mem::take(&mut huidig));
                huidig = woord.to_string();
            } else {
                huidig = proef;
            }
        }
        regels.push(huidig);
    }
    let per_pagina = ((hoog - 2 * marge_y - regelhoogte * 2) / regelhoogte).max(1) as usize;
    let paginas: Vec<&[String]> = regels.chunks(per_pagina).collect();
    if !begin(dc, naam) {
        SelectObject(dc, oud);
        DeleteObject(letter as _);
        DeleteObject(klein as _);
        DeleteDC(dc);
        return Err(crate::teksten::t("AfdrukkenMislukt"));
    }
    for (nr, pagina) in paginas.iter().enumerate() {
        StartPage(dc);
        SelectObject(dc, letter as _);
        let mut y = marge_y;
        for r in pagina.iter() {
            let t: Vec<u16> = r.encode_utf16().collect();
            TextOutW(dc, marge_x, y, t.as_ptr(), t.len() as i32);
            y += regelhoogte;
        }
        SelectObject(dc, klein as _);
        let voet: Vec<u16> = crate::teksten::tf("AfdrukVoet", &[naam, &(nr + 1).to_string(), &paginas.len().to_string()]).encode_utf16().collect();
        TextOutW(dc, marge_x, hoog - marge_y, voet.as_ptr(), voet.len() as i32);
        EndPage(dc);
    }
    EndDoc(dc);
    SelectObject(dc, oud);
    DeleteObject(letter as _);
    DeleteObject(klein as _);
    DeleteDC(dc);
    Ok(paginas.len())
}

/// Drukt scans of foto's af, elk op een eigen pagina.
pub fn beelden(eigenaar: HWND, naam: &str, beelden: &[Beeld]) -> Option<Result<usize, String>> {
    let dc = kies_printer(eigenaar)?;
    unsafe {
        let breed = GetDeviceCaps(dc, HORZRES as _);
        let hoog = GetDeviceCaps(dc, VERTRES as _);
        if !begin(dc, naam) {
            DeleteDC(dc);
            return Some(Err(crate::teksten::t("AfdrukkenMislukt")));
        }
        SetStretchBltMode(dc, HALFTONE);
        for b in beelden {
            StartPage(dc);
            let schaal = (breed as f64 / b.breed as f64).min(hoog as f64 / b.hoog as f64);
            let (dw, dh) = ((b.breed as f64 * schaal) as i32, (b.hoog as f64 * schaal) as i32);
            let mut info: BITMAPINFO = std::mem::zeroed();
            info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            info.bmiHeader.biWidth = b.breed as i32;
            info.bmiHeader.biHeight = -(b.hoog as i32); // van boven naar beneden
            info.bmiHeader.biPlanes = 1;
            info.bmiHeader.biBitCount = 32;
            info.bmiHeader.biCompression = BI_RGB;
            StretchDIBits(dc, (breed - dw) / 2, (hoog - dh) / 2, dw, dh, 0, 0, b.breed as i32, b.hoog as i32,
                b.punten.as_ptr() as *const _, &info, DIB_RGB_COLORS, SRCCOPY);
            EndPage(dc);
        }
        EndDoc(dc);
        DeleteDC(dc);
    }
    Some(Ok(beelden.len()))
}
