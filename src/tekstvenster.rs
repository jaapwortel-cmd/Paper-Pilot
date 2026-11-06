// Paper Pilot - venster met een lange tekst: handleiding, privacyverklaring, wijzigingen.
// Een gewoon Windows-tekstvak (alleen lezen), zodat elke schermlezer het met de pijltjes kan lezen.
// Tab gaat naar Sluiten, Escape sluit het venster.

use crate::win::*;
use std::cell::RefCell;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::UI::Controls::*;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, SetFocus};
use windows_sys::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const KLASSE: &str = "PaperPilotTekst";
const ID_TEKST: i32 = 10;
const ID_SLUITEN: i32 = IDCANCEL;

struct Staat {
    hwnd: HWND,
    eigenaar: HWND,
    kop: HWND,
    vak: HWND,
    knop: HWND,
    letter: HFONT,
    kopletter: HFONT,
    kwast: HBRUSH,
    kleuren: crate::thema::Kleuren,
}

thread_local! {
    static OPEN: RefCell<Vec<Staat>> = const { RefCell::new(Vec::new()) };
}

pub fn registreer() {
    let klasse = w(KLASSE);
    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(venster_proc),
        hInstance: instantie(),
        hIcon: unsafe { LoadIconW(instantie(), 1 as _) },
        hCursor: unsafe { LoadCursorW(std::ptr::null_mut(), IDC_ARROW) },
        lpszClassName: klasse.as_ptr(),
        ..unsafe { std::mem::zeroed() }
    };
    unsafe { RegisterClassExW(&wc) };
}

/// Opent het venster. De eigenaar is zolang uitgeschakeld (zoals bij een dialoogvenster).
pub fn toon(eigenaar: HWND, titel: &str, tekst: &str) {
    let klasse = w(KLASSE);
    let naam = w(&format!("{titel} - Paper Pilot"));
    let mut rc: RECT = unsafe { std::mem::zeroed() };
    unsafe { GetWindowRect(eigenaar, &mut rc) };
    let d = dpi(eigenaar);
    let breed = px(820, d).min(rc.right - rc.left);
    let hoog = px(640, d).min(rc.bottom - rc.top);
    let x = rc.left + (rc.right - rc.left - breed) / 2;
    let y = rc.top + (rc.bottom - rc.top - hoog) / 2;
    let hwnd = unsafe {
        CreateWindowExW(WS_EX_DLGMODALFRAME, klasse.as_ptr(), naam.as_ptr(), WS_OVERLAPPEDWINDOW & !WS_MINIMIZEBOX,
            x, y, breed, hoog, eigenaar, std::ptr::null_mut(), instantie(), std::ptr::null())
    };
    if hwnd.is_null() {
        return;
    }
    let kleuren = crate::hoofdvenster::kleuren();
    crate::thema::titelbalk(hwnd, kleuren.donker);
    let letter = lettertype(12, false, d, "Segoe UI");
    // De kop boven het tekstvak is ook de naam van het tekstvak voor de schermlezer.
    let kopletter = lettertype(16, true, d, "Segoe UI");
    let statisch = w("STATIC");
    let koptekst = w(titel);
    let kop = unsafe {
        CreateWindowExW(0, statisch.as_ptr(), koptekst.as_ptr(), WS_CHILD | WS_VISIBLE | 0x80u32 /* SS_NOPREFIX */,
            0, 0, 10, 10, hwnd, std::ptr::null_mut(), instantie(), std::ptr::null())
    };
    let edit = w("EDIT");
    let leeg = w("");
    let vak = unsafe {
        CreateWindowExW(WS_EX_CLIENTEDGE, edit.as_ptr(), leeg.as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WS_VSCROLL | (ES_MULTILINE | ES_READONLY | ES_AUTOVSCROLL) as u32,
            0, 0, 10, 10, hwnd, ID_TEKST as _, instantie(), std::ptr::null())
    };
    let knopklasse = w("BUTTON");
    let knoptekst = w(&crate::teksten::t("KnopSluiten"));
    let knop = unsafe {
        CreateWindowExW(0, knopklasse.as_ptr(), knoptekst.as_ptr(), WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_PUSHBUTTON as u32,
            0, 0, 10, 10, hwnd, ID_SLUITEN as _, instantie(), std::ptr::null())
    };
    zet_lettertype(vak, letter);
    zet_lettertype(knop, letter);
    // Het tekstvak mag Tab en Escape niet opeten en selecteert niet alles bij de focus.
    unsafe {
        SetWindowSubclass(vak, Some(vak_proc), 1, 0);
        SendMessageW(vak, EM_SETLIMITTEXT, 0, 0);
    }
    zet_lettertype(kop, kopletter);
    zet_tekst(vak, &regels(tekst));
    let kwast = unsafe { CreateSolidBrush(kleuren.vak) };
    OPEN.with(|o| o.borrow_mut().push(Staat { hwnd, eigenaar, kop, vak, knop, letter, kopletter, kwast, kleuren }));
    schik(hwnd);
    unsafe {
        EnableWindow(eigenaar, 0);
        ShowWindow(hwnd, SW_SHOW);
        SetFocus(vak);
        SendMessageW(vak, EM_SETSEL, 0, 0);
        SendMessageW(vak, EM_SCROLLCARET, 0, 0);
    }
}

fn schik(hwnd: HWND) {
    OPEN.with(|o| {
        if let Some(s) = o.borrow().iter().find(|s| s.hwnd == hwnd) {
            let mut rc: RECT = unsafe { std::mem::zeroed() };
            unsafe { GetClientRect(hwnd, &mut rc) };
            let d = dpi(hwnd);
            let m = px(16, d);
            let kh = px(44, d);
            let kb = px(140, d);
            let koph = px(36, d);
            unsafe {
                MoveWindow(s.kop, m, m, rc.right - 2 * m, koph, 1);
                MoveWindow(s.vak, m, m + koph, rc.right - 2 * m, rc.bottom - 3 * m - kh - koph, 1);
                MoveWindow(s.knop, rc.right - m - kb, rc.bottom - m - kh, kb, kh, 1);
            }
        }
    });
}

pub unsafe extern "system" fn vak_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM, _id: usize, _data: usize) -> LRESULT {
    if msg == WM_GETDLGCODE {
        let r = DefSubclassProc(hwnd, msg, wp, lp);
        return r & !((DLGC_WANTALLKEYS | DLGC_WANTTAB | DLGC_HASSETSEL) as isize);
    }
    DefSubclassProc(hwnd, msg, wp, lp)
}

unsafe extern "system" fn venster_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_SIZE => {
            schik(hwnd);
            0
        }
        WM_COMMAND => {
            if (wp & 0xFFFF) as i32 == ID_SLUITEN {
                DestroyWindow(hwnd);
            }
            0
        }
        WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT => {
            let kwast = OPEN.with(|o| o.borrow().iter().find(|s| s.hwnd == hwnd).map(|s| (s.kwast, s.kleuren, s.kop)));
            if let Some((k, kl, kop)) = kwast {
                SetTextColor(wp as HDC, kl.tekst);
                if lp as HWND == kop {
                    SetBkColor(wp as HDC, kl.achter);
                    SetDCBrushColor(wp as HDC, kl.achter);
                    return GetStockObject(DC_BRUSH) as LRESULT;
                }
                SetBkColor(wp as HDC, kl.vak);
                return k as LRESULT;
            }
            DefWindowProcW(hwnd, msg, wp, lp)
        }
        WM_ERASEBKGND => {
            let kleur = OPEN.with(|o| o.borrow().iter().find(|s| s.hwnd == hwnd).map(|s| s.kleuren.achter));
            if let Some(k) = kleur {
                let mut rc: RECT = std::mem::zeroed();
                GetClientRect(hwnd, &mut rc);
                crate::thema::vul(wp as HDC, &rc, k);
                return 1;
            }
            DefWindowProcW(hwnd, msg, wp, lp)
        }
        WM_CLOSE => {
            DestroyWindow(hwnd);
            0
        }
        WM_DESTROY => {
            let staat = OPEN.with(|o| {
                let mut o = o.borrow_mut();
                let i = o.iter().position(|s| s.hwnd == hwnd)?;
                Some(o.remove(i))
            });
            if let Some(s) = staat {
                EnableWindow(s.eigenaar, 1);
                SetForegroundWindow(s.eigenaar);
                weg(s.letter as _);
                weg(s.kopletter as _);
                weg(s.kwast as _);
            }
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

/// Voor de berichtenlus: is dit een tekstvenster? (dan werken Tab en Escape via IsDialogMessage)
pub fn is_tekstvenster(hwnd: HWND) -> bool {
    OPEN.with(|o| o.borrow().iter().any(|s| s.hwnd == hwnd))
}
