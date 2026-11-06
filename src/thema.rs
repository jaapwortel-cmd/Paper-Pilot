// Paper Pilot - kleuren: licht, donker, of zoals Windows (automatisch).
// Staat hoog contrast van Windows aan, dan gebruikt Paper Pilot altijd de kleuren van Windows zelf.

use crate::win::rgb;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::Registry::*;
use windows_sys::Win32::UI::Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

#[derive(Clone, Copy)]
pub struct Kleuren {
    pub achter: u32,
    pub tekst: u32,
    pub zacht: u32,
    pub kop: u32,
    pub kop_tekst: u32,
    pub kop_zacht: u32,
    pub tegel: u32,
    pub tegel_rand: u32,
    pub zweven: u32,
    pub ingedrukt: u32,
    pub accent: u32,
    pub focus: u32,
    pub vak: u32,
    pub donker: bool,
    pub hoog_contrast: bool,
}

pub fn hoog_contrast() -> bool {
    let mut hc: HIGHCONTRASTW = unsafe { std::mem::zeroed() };
    hc.cbSize = std::mem::size_of::<HIGHCONTRASTW>() as u32;
    let ok = unsafe {
        SystemParametersInfoW(SPI_GETHIGHCONTRAST, hc.cbSize, &mut hc as *mut _ as *mut _, 0)
    };
    ok != 0 && (hc.dwFlags & HCF_HIGHCONTRASTON) != 0
}

/// Waar als Windows voor programma's de donkere stand gebruikt.
pub fn windows_donker() -> bool {
    let sleutel = crate::win::w("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");
    let naam = crate::win::w("AppsUseLightTheme");
    let mut waarde: u32 = 1;
    let mut grootte = 4u32;
    let r = unsafe {
        RegGetValueW(HKEY_CURRENT_USER, sleutel.as_ptr(), naam.as_ptr(), RRF_RT_REG_DWORD, std::ptr::null_mut(),
            &mut waarde as *mut u32 as *mut _, &mut grootte)
    };
    r == 0 && waarde == 0
}

/// keuze: "automatisch", "licht" of "donker".
pub fn kies(keuze: &str) -> Kleuren {
    if hoog_contrast() {
        let s = |i| unsafe { GetSysColor(i) };
        return Kleuren {
            achter: s(COLOR_WINDOW), tekst: s(COLOR_WINDOWTEXT), zacht: s(COLOR_WINDOWTEXT),
            kop: s(COLOR_WINDOW), kop_tekst: s(COLOR_WINDOWTEXT), kop_zacht: s(COLOR_WINDOWTEXT),
            tegel: s(COLOR_BTNFACE), tegel_rand: s(COLOR_BTNTEXT), zweven: s(COLOR_HIGHLIGHT),
            ingedrukt: s(COLOR_HIGHLIGHT), accent: s(COLOR_HOTLIGHT), focus: s(COLOR_HIGHLIGHT),
            vak: s(COLOR_WINDOW), donker: false, hoog_contrast: true,
        };
    }
    let donker = match keuze {
        "licht" => false,
        "donker" => true,
        _ => windows_donker(),
    };
    if donker {
        Kleuren {
            achter: rgb(0x17, 0x1A, 0x21), tekst: rgb(0xF2, 0xF4, 0xF8), zacht: rgb(0xB4, 0xBD, 0xCB),
            kop: rgb(0x1F, 0x3F, 0x73), kop_tekst: rgb(0xFF, 0xFF, 0xFF), kop_zacht: rgb(0xC9, 0xD8, 0xF2),
            tegel: rgb(0x23, 0x28, 0x31), tegel_rand: rgb(0x3A, 0x41, 0x4E), zweven: rgb(0x2B, 0x35, 0x48),
            ingedrukt: rgb(0x1F, 0x2B, 0x3F), accent: rgb(0x7F, 0xB2, 0xFF), focus: rgb(0xFF, 0xD3, 0x4D),
            vak: rgb(0x1E, 0x22, 0x2A), donker: true, hoog_contrast: false,
        }
    } else {
        Kleuren {
            achter: rgb(0xF4, 0xF6, 0xFA), tekst: rgb(0x14, 0x1B, 0x2A), zacht: rgb(0x4A, 0x55, 0x68),
            kop: rgb(0x1F, 0x5F, 0xBF), kop_tekst: rgb(0xFF, 0xFF, 0xFF), kop_zacht: rgb(0xDC, 0xE8, 0xFA),
            tegel: rgb(0xFF, 0xFF, 0xFF), tegel_rand: rgb(0xD3, 0xDA, 0xE6), zweven: rgb(0xEA, 0xF1, 0xFC),
            ingedrukt: rgb(0xD6, 0xE4, 0xF8), accent: rgb(0x1F, 0x5F, 0xBF), focus: rgb(0x0B, 0x2A, 0x5C),
            vak: rgb(0xFF, 0xFF, 0xFF), donker: false, hoog_contrast: false,
        }
    }
}

/// Donkere titelbalk bij de donkere stand (Windows 10 2004 en nieuwer).
pub fn titelbalk(hwnd: HWND, donker: bool) {
    let aan: i32 = donker as i32;
    unsafe {
        DwmSetWindowAttribute(hwnd, 20, &aan as *const i32 as *const _, 4);
    }
}

/// Een gevuld vlak met ronde hoeken en een rand.
pub fn rond_vlak(dc: HDC, l: i32, b: i32, r: i32, o: i32, straal: i32, vul: u32, rand: u32, dikte: i32) {
    unsafe {
        let kwast = CreateSolidBrush(vul);
        let pen = CreatePen(PS_SOLID, dikte.max(1), rand);
        let oude_k = SelectObject(dc, kwast as _);
        let oude_p = SelectObject(dc, pen as _);
        let half = dikte / 2;
        RoundRect(dc, l + half, b + half, r - half, o - half, straal, straal);
        SelectObject(dc, oude_k);
        SelectObject(dc, oude_p);
        DeleteObject(kwast as _);
        DeleteObject(pen as _);
    }
}

pub fn vul(dc: HDC, rc: &windows_sys::Win32::Foundation::RECT, kleur: u32) {
    unsafe {
        let k = CreateSolidBrush(kleur);
        FillRect(dc, rc, k);
        DeleteObject(k as _);
    }
}
