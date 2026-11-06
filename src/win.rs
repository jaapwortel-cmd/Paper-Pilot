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
