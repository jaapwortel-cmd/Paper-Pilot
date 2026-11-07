// Paper Pilot - kleine hulpjes voor Windows (tekst omzetten, lettertypen, schaal).

use std::ffi::c_void;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

/// Tekst naar UTF-16 met een nul aan het eind, voor Windows.
pub fn w(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Tekst voor Windows-onderdelen: gewone regeleinden worden \r\n.
pub fn regels(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\n', "\r\n")
}

pub fn dpi(hwnd: HWND) -> i32 {
    let d = unsafe { GetDpiForWindow(hwnd) } as i32;
    if d <= 0 { 96 } else { d }
}

/// Maat in "beeldpunten bij 100%" omrekenen naar echte pixels.
pub fn px(waarde: i32, dpi: i32) -> i32 {
    (waarde * dpi + 48) / 96
}

pub fn lettertype(punten: i32, vet: bool, dpi: i32, naam: &str) -> HFONT {
    let hoogte = -((punten * dpi + 36) / 72);
    let n = w(naam);
    unsafe {
        CreateFontW(hoogte, 0, 0, 0, if vet { 600 } else { 400 }, 0, 0, 0, DEFAULT_CHARSET as u32,
            OUT_DEFAULT_PRECIS as u32, CLIP_DEFAULT_PRECIS as u32, CLEARTYPE_QUALITY as u32,
            (DEFAULT_PITCH | FF_DONTCARE) as u32, n.as_ptr())
    }
}

pub fn zet_tekst(hwnd: HWND, s: &str) {
    let t = w(s);
    unsafe { SetWindowTextW(hwnd, t.as_ptr()) };
}

pub fn lees_tekst(hwnd: HWND) -> String {
    unsafe {
        let n = GetWindowTextLengthW(hwnd);
        let mut buf = vec![0u16; n as usize + 1];
        let m = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
        String::from_utf16_lossy(&buf[..m.max(0) as usize])
    }
}

pub fn zet_lettertype(hwnd: HWND, f: HFONT) {
    unsafe { SendMessageW(hwnd, WM_SETFONT, f as WPARAM, 1) };
}

pub fn rgb(r: u8, g: u8, b: u8) -> u32 {
    r as u32 | (g as u32) << 8 | (b as u32) << 16
}

pub fn weg(obj: *mut c_void) {
    if !obj.is_null() {
        unsafe { DeleteObject(obj) };
    }
}

pub fn instantie() -> HINSTANCE {
    unsafe { windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(std::ptr::null()) }
}

/// Een bekende map van Windows, bijvoorbeeld %APPDATA%.
pub fn omgeving(naam: &str) -> Option<String> {
    std::env::var(naam).ok().filter(|s| !s.is_empty())
}

/// Opent een adres (mailto: of https:) met het standaardprogramma van Windows.
pub fn open_adres(adres: &str) -> bool {
    let a = w(adres);
    let open = w("open");
    let r = unsafe {
        windows_sys::Win32::UI::Shell::ShellExecuteW(std::ptr::null_mut(), open.as_ptr(), a.as_ptr(),
            std::ptr::null(), std::ptr::null(), SW_SHOWNORMAL)
    };
    r as isize > 32
}

/// Codeert tekst voor een mailto- of webadres.
pub fn adrescode(s: &str) -> String {
    let mut uit = String::new();
    for b in s.as_bytes() {
        match *b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => uit.push(*b as char),
            _ => uit.push_str(&format!("%{:02X}", b)),
        }
    }
    uit
}

/// Zet tekst op het klembord (om te plakken met Ctrl+V).
pub fn kopieer(eigenaar: HWND, tekst: &str) -> bool {
    use windows_sys::Win32::System::DataExchange::*;
    use windows_sys::Win32::System::Memory::*;
    const CF_UNICODETEXT: u32 = 13;
    let data = w(tekst);
    unsafe {
        let mut gelukt = false;
        for _ in 0..5 {
            if OpenClipboard(eigenaar) != 0 {
                gelukt = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        if !gelukt {
            return false;
        }
        EmptyClipboard();
        let geheugen = GlobalAlloc(GMEM_MOVEABLE, data.len() * 2);
        let mut ok = false;
        if !geheugen.is_null() {
            let p = GlobalLock(geheugen) as *mut u16;
            if !p.is_null() {
                std::ptr::copy_nonoverlapping(data.as_ptr(), p, data.len());
                GlobalUnlock(geheugen);
                ok = !SetClipboardData(CF_UNICODETEXT, geheugen as HANDLE).is_null();
            }
            if !ok {
                GlobalFree(geheugen);
            }
        }
        CloseClipboard();
        ok
    }
}

/// Zet een bestand op het klembord, zoals Kopiëren in de Verkenner (plakken in een mail = bijlage).
pub fn kopieer_bestand(eigenaar: HWND, pad: &std::path::Path) -> bool {
    use windows_sys::Win32::System::DataExchange::*;
    use windows_sys::Win32::System::Memory::*;
    use windows_sys::Win32::UI::Shell::DROPFILES;
    const CF_HDROP: u32 = 15;
    let mut namen: Vec<u16> = pad.as_os_str().to_string_lossy().encode_utf16().collect();
    namen.push(0);
    namen.push(0);
    let kop = std::mem::size_of::<DROPFILES>();
    unsafe {
        if OpenClipboard(eigenaar) == 0 {
            return false;
        }
        EmptyClipboard();
        let geheugen = GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, kop + namen.len() * 2);
        let mut ok = false;
        if !geheugen.is_null() {
            let p = GlobalLock(geheugen) as *mut u8;
            if !p.is_null() {
                let df = p as *mut DROPFILES;
                (*df).pFiles = kop as u32;
                (*df).fWide = 1;
                std::ptr::copy_nonoverlapping(namen.as_ptr() as *const u8, p.add(kop), namen.len() * 2);
                GlobalUnlock(geheugen);
                ok = !SetClipboardData(CF_HDROP, geheugen as HANDLE).is_null();
            }
            if !ok {
                GlobalFree(geheugen);
            }
        }
        CloseClipboard();
        ok
    }
}

/// De lettergrootte voor tekst om te lezen (punten), te kiezen met Ctrl+plus en Ctrl+min.
pub const GROOTTES: [i32; 10] = [10, 12, 14, 16, 18, 20, 24, 28, 32, 36];

pub fn tekstgrootte() -> i32 {
    crate::instellingen::get("tekstgrootte").and_then(|v| v.parse().ok()).unwrap_or(12).clamp(10, 36)
}
