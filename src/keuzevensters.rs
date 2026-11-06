// Paper Pilot - kleine vensters om iets te vragen: een tekst (bijvoorbeeld een zoekwoord) of een keuze uit een lijst.
// Echte Windows-onderdelen: een tekstvak (ook om in te dicteren) of een lijst, met OK en Annuleren.
// Enter = OK, Escape = Annuleren. De eigenaar is uitgeschakeld zolang het venster open is.

use crate::win::*;
use std::cell::RefCell;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::UI::Controls::EM_SETSEL;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, SetFocus};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const KLASSE: &str = "PaperPilotKeuze";
const ID_INVOER: i32 = 10;
const LBN_DBLCLK: u32 = 2;

struct Staat {
    hwnd: HWND,
    invoer: HWND,
    lijst: bool,
    klaar: bool,
    uitkomst: Option<(String, usize)>,
    kwast: HBRUSH,
    vak: HBRUSH,
}

thread_local! {
    static OPEN: RefCell<Vec<Staat>> = const { RefCell::new(Vec::new()) };
}

fn registreer() {
    let klasse = w(KLASSE);
    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(venster_proc),
        hInstance: instantie(),
        hCursor: unsafe { LoadCursorW(std::ptr::null_mut(), IDC_ARROW) },
        lpszClassName: klasse.as_ptr(),
        ..unsafe { std::mem::zeroed() }
    };
    unsafe { RegisterClassExW(&wc) };
}

/// Vraagt een tekst. Geeft None bij Annuleren.
pub fn vraag_tekst(eigenaar: HWND, titel: &str, label: &str, standaard: &str) -> Option<String> {
    toon(eigenaar, titel, label, None, standaard, 0).map(|(t, _)| t)
}

/// Laat kiezen uit een lijst. Geeft het nummer van de keuze, of None bij Annuleren.
pub fn kies_uit_lijst(eigenaar: HWND, titel: &str, label: &str, items: &[String], begin: usize) -> Option<usize> {
    if items.is_empty() {
        return None;
    }
    toon(eigenaar, titel, label, Some(items), "", begin).map(|(_, i)| i)
}

fn toon(eigenaar: HWND, titel: &str, label: &str, items: Option<&[String]>, standaard: &str, begin: usize) -> Option<(String, usize)> {
    registreer();
    let d = dpi(eigenaar);
    let kleuren = crate::hoofdvenster::kleuren();
    let breed = px(520, d);
    let hoog = if items.is_some() { px(480, d) } else { px(220, d) };
    let mut rc: RECT = unsafe { std::mem::zeroed() };
    unsafe { GetWindowRect(eigenaar, &mut rc) };
    let x = rc.left + (rc.right - rc.left - breed) / 2;
    let y = rc.top + (rc.bottom - rc.top - hoog) / 3;
    let klasse = w(KLASSE);
    let naam = w(titel);
    let hwnd = unsafe {
        CreateWindowExW(WS_EX_DLGMODALFRAME, klasse.as_ptr(), naam.as_ptr(), WS_POPUP | WS_CAPTION | WS_SYSMENU,
            x, y, breed, hoog, eigenaar, std::ptr::null_mut(), instantie(), std::ptr::null())
    };
    if hwnd.is_null() {
        return None;
    }
    crate::thema::titelbalk(hwnd, kleuren.donker);
    let mut client: RECT = unsafe { std::mem::zeroed() };
    unsafe { GetClientRect(hwnd, &mut client) };
    let m = px(16, d);
    let letter = lettertype(12, false, d, "Segoe UI");
    let maak = |klasse: &str, tekst: &str, stijl: u32, ex: u32, x: i32, y: i32, b: i32, h: i32, id: i32| unsafe {
        let k = w(klasse);
        let t = w(tekst);
        let c = CreateWindowExW(ex, k.as_ptr(), t.as_ptr(), WS_CHILD | WS_VISIBLE | stijl, x, y, b, h, hwnd, id as isize as _,
            instantie(), std::ptr::null());
        zet_lettertype(c, letter);
        c
    };
    // Het label staat vlak voor het invoervak: dat is de naam die de schermlezer noemt.
    maak("STATIC", label, 0x80, 0, m, m, client.right - 2 * m, px(26, d), -1);
    let kh = px(40, d);
    let kb = px(130, d);
    let onder = client.bottom - m - kh;
    let invoer = match items {
        Some(lijst) => {
            let l = maak("LISTBOX", "", WS_TABSTOP | WS_VSCROLL | WS_BORDER | (LBS_NOTIFY | LBS_NOINTEGRALHEIGHT) as u32, 0,
                m, m + px(32, d), client.right - 2 * m, onder - m - (m + px(32, d)), ID_INVOER);
            for item in lijst {
                let t = w(item);
                unsafe { SendMessageW(l, LB_ADDSTRING, 0, t.as_ptr() as LPARAM) };
            }
            unsafe { SendMessageW(l, LB_SETCURSEL, begin.min(lijst.len() - 1), 0) };
            l
        }
        None => {
            let e = maak("EDIT", standaard, WS_TABSTOP | ES_AUTOHSCROLL as u32, WS_EX_CLIENTEDGE,
                m, m + px(32, d), client.right - 2 * m, px(34, d), ID_INVOER);
            unsafe { SendMessageW(e, EM_SETSEL, 0, -1) };
            e
        }
    };
    maak("BUTTON", &crate::teksten::t("KnopOk"), WS_TABSTOP | BS_DEFPUSHBUTTON as u32, 0,
        client.right - m - 2 * kb - m / 2, onder, kb, kh, IDOK);
    maak("BUTTON", &crate::teksten::t("KnopAnnuleren"), WS_TABSTOP | BS_PUSHBUTTON as u32, 0,
        client.right - m - kb, onder, kb, kh, IDCANCEL);
    let kwast = unsafe { CreateSolidBrush(kleuren.achter) };
    let vak = unsafe { CreateSolidBrush(kleuren.vak) };
    OPEN.with(|o| o.borrow_mut().push(Staat { hwnd, invoer, lijst: items.is_some(), klaar: false, uitkomst: None, kwast, vak }));
    unsafe {
        EnableWindow(eigenaar, 0);
        ShowWindow(hwnd, SW_SHOW);
        SetFocus(invoer);
    }
    // Eigen berichtenlus tot het venster klaar is.
    let mut msg: MSG = unsafe { std::mem::zeroed() };
    loop {
        let klaar = OPEN.with(|o| o.borrow().iter().find(|s| s.hwnd == hwnd).map(|s| s.klaar).unwrap_or(true));
        if klaar {
            break;
        }
        if unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) } <= 0 {
            unsafe { PostQuitMessage(msg.wParam as i32) };
            break;
        }
        if unsafe { IsDialogMessageW(hwnd, &msg) } == 0 {
            unsafe {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }
    let staat = OPEN.with(|o| {
        let mut o = o.borrow_mut();
        o.iter().position(|s| s.hwnd == hwnd).map(|i| o.remove(i))
    });
    unsafe {
        EnableWindow(eigenaar, 1);
        DestroyWindow(hwnd);
        SetForegroundWindow(eigenaar);
        weg(letter as _);
    }
    let s = staat?;
    weg(s.kwast as _);
    weg(s.vak as _);
    s.uitkomst
}

fn sluit(hwnd: HWND, ok: bool) {
    let invoer = OPEN.with(|o| o.borrow().iter().find(|s| s.hwnd == hwnd).map(|s| (s.invoer, s.lijst)));
    let Some((invoer, lijst)) = invoer else { return };
    let uitkomst = if !ok {
        None
    } else if lijst {
        let i = unsafe { SendMessageW(invoer, LB_GETCURSEL, 0, 0) };
        if i < 0 { None } else { Some((String::new(), i as usize)) }
    } else {
        Some((lees_tekst(invoer), 0))
    };
    OPEN.with(|o| {
        if let Some(s) = o.borrow_mut().iter_mut().find(|s| s.hwnd == hwnd) {
            s.klaar = true;
            s.uitkomst = uitkomst;
        }
    });
}

unsafe extern "system" fn venster_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_COMMAND => {
            let id = (wp & 0xFFFF) as i32;
            let melding = ((wp >> 16) & 0xFFFF) as u32;
            if id == IDOK || (id == ID_INVOER && melding == LBN_DBLCLK) {
                sluit(hwnd, true);
            } else if id == IDCANCEL {
                sluit(hwnd, false);
            }
            0
        }
        WM_CLOSE => {
            sluit(hwnd, false);
            0
        }
        WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => {
            let kwasten = OPEN.with(|o| o.borrow().iter().find(|s| s.hwnd == hwnd).map(|s| (s.kwast, s.vak)));
            let k = crate::hoofdvenster::kleuren();
            if let Some((achter, vak)) = kwasten {
                SetTextColor(wp as HDC, k.tekst);
                if msg == WM_CTLCOLORSTATIC {
                    SetBkColor(wp as HDC, k.achter);
                    return achter as LRESULT;
                }
                SetBkColor(wp as HDC, k.vak);
                return vak as LRESULT;
            }
            DefWindowProcW(hwnd, msg, wp, lp)
        }
        WM_ERASEBKGND => {
            let mut rc: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut rc);
            crate::thema::vul(wp as HDC, &rc, crate::hoofdvenster::kleuren().achter);
            1
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}
