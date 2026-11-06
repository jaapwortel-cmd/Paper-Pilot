// Paper Pilot - het hoofdvenster.
//
// Bovenaan een gekleurde kop met de naam, daaronder twee grote tegels: "Lezen en invullen" en "Controleren".
// De tegels zijn echte Windows-knoppen (zelf getekend), dus schermlezers zien gewone knoppen met een naam.
// De kop en de regel onderaan zijn alleen tekening: ze krijgen geen focus en zitten de schermlezer niet in de weg.
// Alles werkt met het toetsenbord (Tab, Enter, spatie, Alt voor het menu, F1) en met de muis.

use crate::teksten::{t, tf};
use crate::thema::{self, Kleuren};
use crate::win::*;
use crate::{dialogen, instellingen, logboek, tekstvenster, Opties};
use std::cell::{Cell, RefCell};
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::UI::Controls::*;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const KLASSE: &str = "PaperPilotHoofdvenster";

const ID_LEZEN: i32 = 1;
const ID_CONTROLEREN: i32 = 2;
const ID_AFSLUITEN: i32 = 100;
const ID_TAAL_NL: i32 = 200;
const ID_TAAL_EN: i32 = 201;
const ID_THEMA_AUTO: i32 = 210;
const ID_THEMA_LICHT: i32 = 211;
const ID_THEMA_DONKER: i32 = 212;
const ID_UPDATES_START: i32 = 220;
const ID_HANDLEIDING: i32 = 300;
const ID_RONDLEIDING: i32 = 301;
const ID_WIJZIGINGEN: i32 = 302;
const ID_PRIVACY: i32 = 303;
const ID_MEER: i32 = 304;
const ID_MAIL: i32 = 305;
const ID_UPDATES_NU: i32 = 306;
const ID_OVER: i32 = 307;

const WM_START: u32 = WM_APP + 1;
const WM_UPDATE: u32 = WM_APP + 2;
const TIMER_FOTO: usize = 1;

#[derive(Clone, Copy)]
struct Letters {
    titel: HFONT,
    onder: HFONT,
    tegel_kop: HFONT,
    tegel_tekst: HFONT,
    klein: HFONT,
    symbool: HFONT,
}

impl Letters {
    fn maak(d: i32) -> Self {
        Letters {
            titel: lettertype(26, true, d, "Segoe UI"),
            onder: lettertype(12, false, d, "Segoe UI"),
            tegel_kop: lettertype(16, true, d, "Segoe UI"),
            tegel_tekst: lettertype(11, false, d, "Segoe UI"),
            klein: lettertype(10, false, d, "Segoe UI"),
            symbool: lettertype(26, false, d, "Segoe MDL2 Assets"),
        }
    }
    fn weg(&self) {
        for f in [self.titel, self.onder, self.tegel_kop, self.tegel_tekst, self.klein, self.symbool] {
            weg(f as _);
        }
    }
}

#[derive(Clone)]
struct Staat {
    hwnd: HWND,
    tegels: [HWND; 2],
    letters: Letters,
    kleuren: Kleuren,
    opties: Opties,
    toetsen: HACCEL,
}

thread_local! {
    static STAAT: RefCell<Option<Staat>> = const { RefCell::new(None) };
    static ZWEEFT: Cell<HWND> = const { Cell::new(std::ptr::null_mut()) };
    static LAATSTE_FOCUS: Cell<HWND> = const { Cell::new(std::ptr::null_mut()) };
}

/// Korte wijzigingen aan de staat. Roep hierbinnen nooit Windows-functies aan die berichten sturen.
fn met<R>(f: impl FnOnce(&mut Staat) -> R) -> Option<R> {
    STAAT.with(|s| s.borrow_mut().as_mut().map(f))
}

/// Een kopie van de staat, om veilig Windows-functies mee aan te roepen.
fn lees() -> Option<Staat> {
    STAAT.with(|s| s.borrow().clone())
}

/// De kleuren die nu gelden (ook voor andere vensters).
pub fn kleuren() -> Kleuren {
    met(|s| s.kleuren).unwrap_or_else(|| thema::kies(&instellingen::get("thema").unwrap_or_default()))
}

pub fn hoofd_hwnd() -> HWND {
    met(|s| s.hwnd).unwrap_or(std::ptr::null_mut())
}

/// Een mail aan de maker in het eigen mailprogramma, met de versie erbij.
pub fn mail(onderwerp: &str, tekst: &str) {
    let tekst = format!("{tekst}\r\n\r\n---\r\nPaper Pilot {}", crate::VERSIE);
    let adres = format!("mailto:{}?subject={}&body={}", crate::MAIL, adrescode(onderwerp), adrescode(&tekst));
    if !open_adres(&adres) {
        dialogen::melding(hoofd_hwnd(), &t("GeenMailprogramma"), &tf("GeenMailprogrammaTekst", &[crate::MAIL]), dialogen::ICOON_WAARSCHUWING);
    }
}

fn kies_taal_bij_eerste_start() -> String {
    let taal = unsafe { windows_sys::Win32::Globalization::GetUserDefaultUILanguage() } & 0x3FF;
    if taal == 0x13 || taal == 0x62 { "nl".into() } else { "en".into() }
}

pub fn start(opties: Opties) -> i32 {
    if instellingen::get("taal").is_none() {
        instellingen::zet("taal", &kies_taal_bij_eerste_start());
    }
    let taal = opties.taal.clone().or_else(|| instellingen::get("taal")).unwrap_or_else(|| "nl".into());
    crate::teksten::zet_taal(&taal);
    let hwnd = maak(opties);
    if hwnd.is_null() {
        return 1;
    }
    berichtenlus()
}

/// Maakt het hoofdvenster (ook gebruikt door de proefstart).
pub fn maak(opties: Opties) -> HWND {
    unsafe {
        let icc = INITCOMMONCONTROLSEX { dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32, dwICC: ICC_STANDARD_CLASSES };
        InitCommonControlsEx(&icc);
    }
    tekstvenster::registreer();
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
    let titel = w("Paper Pilot");
    let foto = opties.schermafbeelding.is_some();
    let hwnd = unsafe {
        CreateWindowExW(0, klasse.as_ptr(), titel.as_ptr(), WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
            CW_USEDEFAULT, CW_USEDEFAULT, 1100, 720, std::ptr::null_mut(), std::ptr::null_mut(), instantie(), std::ptr::null())
    };
    if hwnd.is_null() {
        logboek::schrijf("Het hoofdvenster kon niet worden gemaakt.");
        return hwnd;
    }
    let d = dpi(hwnd);
    let keuze = opties.thema.clone().or_else(|| instellingen::get("thema")).unwrap_or_else(|| "automatisch".into());
    let kleuren = thema::kies(&keuze);
    thema::titelbalk(hwnd, kleuren.donker);
    let knop = w("BUTTON");
    let mut tegels = [std::ptr::null_mut(); 2];
    for (i, id) in [ID_LEZEN, ID_CONTROLEREN].iter().enumerate() {
        let leeg = w("");
        tegels[i] = unsafe {
            CreateWindowExW(0, knop.as_ptr(), leeg.as_ptr(),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | if i == 0 { WS_GROUP } else { 0 } | BS_OWNERDRAW as u32,
                0, 0, 10, 10, hwnd, *id as isize as _, instantie(), std::ptr::null())
        };
        unsafe { SetWindowSubclass(tegels[i], Some(tegel_proc), 1, 0) };
    }
    let toetsen = unsafe {
        let a = [ACCEL { fVirt: FVIRTKEY as u8, key: VK_F1, cmd: ID_HANDLEIDING as u16 }];
        CreateAcceleratorTableW(a.as_ptr(), a.len() as i32)
    };
    STAAT.with(|s| *s.borrow_mut() = Some(Staat { hwnd, tegels, letters: Letters::maak(d), kleuren, opties, toetsen }));
    zet_teksten();
    if foto {
        let (b, h) = (px(1100, d), px(720, d));
        unsafe {
            SetWindowPos(hwnd, std::ptr::null_mut(), 40, 40, b, h, SWP_NOZORDER);
            ShowWindow(hwnd, SW_SHOWNORMAL);
        }
    } else {
        unsafe { ShowWindow(hwnd, SW_SHOWMAXIMIZED) };
    }
    unsafe {
        UpdateWindow(hwnd);
        SetFocus(tegels[0]);
        PostMessageW(hwnd, WM_START, 0, 0);
    }
    logboek::schrijf(&format!("Paper Pilot {} gestart (taal {}, thema {}).", crate::VERSIE, crate::teksten::taal(), keuze));
    hwnd
}

/// Alle teksten (menu, tegels) in de gekozen taal zetten. Ook na het wisselen van taal.
fn zet_teksten() {
    if let Some(s) = lees() {
        let oud = unsafe { GetMenu(s.hwnd) };
        let menu = maak_menu();
        unsafe {
            SetMenu(s.hwnd, menu);
            if !oud.is_null() {
                DestroyMenu(oud);
            }
        }
        zet_tekst(s.tegels[0], &format!("{}. {} {}", t("TegelLezen"), t("TegelLezenOver"), t("Binnenkort")));
        zet_tekst(s.tegels[1], &format!("{}. {} {}", t("TegelControleren"), t("TegelControlerenOver"), t("Binnenkort")));
        unsafe { InvalidateRect(s.hwnd, std::ptr::null(), 1) };
    }
    vink_menu();
}

fn maak_menu() -> HMENU {
    unsafe {
        let balk = CreateMenu();
        let voeg = |m: HMENU, id: i32, sleutel: &str| {
            let tekst = w(&t(sleutel));
            AppendMenuW(m, MF_STRING, id as usize, tekst.as_ptr());
        };
        let streep = |m: HMENU| {
            AppendMenuW(m, MF_SEPARATOR, 0, std::ptr::null());
        };
        let sub = |balk: HMENU, m: HMENU, sleutel: &str| {
            let tekst = w(&t(sleutel));
            AppendMenuW(balk, MF_POPUP, m as usize, tekst.as_ptr());
        };

        let bestand = CreatePopupMenu();
        voeg(bestand, ID_AFSLUITEN, "MenuAfsluiten");
        sub(balk, bestand, "MenuBestand");

        let inst = CreatePopupMenu();
        let taal = CreatePopupMenu();
        voeg(taal, ID_TAAL_NL, "MenuNederlands");
        voeg(taal, ID_TAAL_EN, "MenuEngels");
        sub(inst, taal, "MenuTaal");
        let weergave = CreatePopupMenu();
        voeg(weergave, ID_THEMA_AUTO, "MenuThemaAuto");
        voeg(weergave, ID_THEMA_LICHT, "MenuThemaLicht");
        voeg(weergave, ID_THEMA_DONKER, "MenuThemaDonker");
        sub(inst, weergave, "MenuWeergave");
        streep(inst);
        voeg(inst, ID_UPDATES_START, "MenuUpdatesBijStart");
        sub(balk, inst, "MenuInstellingen");

        let help = CreatePopupMenu();
        voeg(help, ID_HANDLEIDING, "MenuHandleiding");
        voeg(help, ID_RONDLEIDING, "MenuRondleiding");
        voeg(help, ID_WIJZIGINGEN, "MenuWijzigingen");
        voeg(help, ID_PRIVACY, "MenuPrivacy");
        streep(help);
        voeg(help, ID_MAIL, "MenuMail");
        voeg(help, ID_MEER, "MenuMeer");
        streep(help);
        voeg(help, ID_UPDATES_NU, "MenuUpdatesNu");
        voeg(help, ID_OVER, "MenuOver");
        sub(balk, help, "MenuHelp");
        balk
    }
}

/// Vinkjes en keuzerondjes in het menu bijwerken.
fn vink_menu() {
    if let Some(s) = lees() { unsafe {
        let menu = GetMenu(s.hwnd);
        let taal = if crate::teksten::taal() == "en" { ID_TAAL_EN } else { ID_TAAL_NL };
        CheckMenuRadioItem(menu, ID_TAAL_NL as u32, ID_TAAL_EN as u32, taal as u32, MF_BYCOMMAND);
        let thema = match instellingen::get("thema").as_deref() {
            Some("licht") => ID_THEMA_LICHT,
            Some("donker") => ID_THEMA_DONKER,
            _ => ID_THEMA_AUTO,
        };
        CheckMenuRadioItem(menu, ID_THEMA_AUTO as u32, ID_THEMA_DONKER as u32, thema as u32, MF_BYCOMMAND);
        let aan = instellingen::aan("updates_bij_start", true);
        CheckMenuItem(menu, ID_UPDATES_START as u32, MF_BYCOMMAND | if aan { MF_CHECKED } else { MF_UNCHECKED });
    } }
}

fn herlaad_kleuren() {
    met(|s| {
        let keuze = s.opties.thema.clone().or_else(|| instellingen::get("thema")).unwrap_or_else(|| "automatisch".into());
        s.kleuren = thema::kies(&keuze);
    });
    if let Some(s) = lees() {
        thema::titelbalk(s.hwnd, s.kleuren.donker);
        unsafe { InvalidateRect(s.hwnd, std::ptr::null(), 1) };
        for t in s.tegels {
            unsafe { InvalidateRect(t, std::ptr::null(), 1) };
        }
    }
}

/// De plaats van de tegels: naast elkaar als er ruimte is, anders onder elkaar.
fn schik() {
    if let Some(s) = lees() {
        let mut rc: RECT = unsafe { std::mem::zeroed() };
        unsafe { GetClientRect(s.hwnd, &mut rc) };
        let d = dpi(s.hwnd);
        let rand = px(32, d);
        let boven = px(140, d) + rand;
        let ruimte = rc.right - 2 * rand;
        if ruimte >= px(720, d) {
            let b = ((ruimte - rand) / 2).min(px(500, d));
            let h = px(210, d);
            let links = (rc.right - (2 * b + rand)) / 2;
            unsafe {
                MoveWindow(s.tegels[0], links, boven, b, h, 1);
                MoveWindow(s.tegels[1], links + b + rand, boven, b, h, 1);
            }
        } else {
            let b = ruimte.max(px(200, d));
            let h = px(180, d);
            unsafe {
                MoveWindow(s.tegels[0], rand, boven, b, h, 1);
                MoveWindow(s.tegels[1], rand, boven + h + rand / 2, b, h, 1);
            }
        }
    }
}

fn teken_tekst(dc: HDC, tekst: &str, rc: &mut RECT, letter: HFONT, kleur: u32, vlag: u32) {
    let t: Vec<u16> = tekst.encode_utf16().collect();
    unsafe {
        let oud = SelectObject(dc, letter as _);
        SetTextColor(dc, kleur);
        SetBkMode(dc, TRANSPARENT as _);
        DrawTextW(dc, t.as_ptr(), t.len() as i32, rc, vlag);
        SelectObject(dc, oud);
    }
}

fn teken_venster(hwnd: HWND) {
    let Some(s) = lees() else { return };
    unsafe {
        let mut ps: PAINTSTRUCT = std::mem::zeroed();
        let echt = BeginPaint(hwnd, &mut ps);
        let mut rc: RECT = std::mem::zeroed();
        GetClientRect(hwnd, &mut rc);
        let dc = CreateCompatibleDC(echt);
        let bmp = CreateCompatibleBitmap(echt, rc.right.max(1), rc.bottom.max(1));
        let oud = SelectObject(dc, bmp as _);
        let k = s.kleuren;
        let d = dpi(hwnd);
        thema::vul(dc, &rc, k.achter);
        // De kop met het vliegtuigje, de naam en de ondertitel.
        let kop = RECT { left: 0, top: 0, right: rc.right, bottom: px(140, d) };
        thema::vul(dc, &kop, k.kop);
        if k.hoog_contrast {
            let lijn = RECT { left: 0, top: kop.bottom - px(2, d), right: rc.right, bottom: kop.bottom };
            thema::vul(dc, &lijn, k.tekst);
        }
        let rand = px(32, d);
        let maat = px(64, d);
        let icoon = LoadImageW(instantie(), 1 as _, IMAGE_ICON, maat, maat, LR_DEFAULTCOLOR) as HICON;
        let iy = (kop.bottom - maat) / 2;
        if !k.hoog_contrast {
            // Wit vlak achter het pictogram, zodat het blauwe vliegtuigje ook op de blauwe kop goed te zien is.
            let m = px(6, d);
            thema::rond_vlak(dc, rand - m, iy - m, rand + maat + m, iy + maat + m, px(22, d), rgb(255, 255, 255), rgb(255, 255, 255), 1);
        }
        if !icoon.is_null() {
            DrawIconEx(dc, rand, iy, icoon, maat, maat, 0, std::ptr::null_mut(), DI_NORMAL);
            DestroyIcon(icoon);
        }
        let tx = rand + maat + px(20, d);
        let mut r1 = RECT { left: tx, top: iy - px(6, d), right: rc.right - rand, bottom: iy + px(44, d) };
        teken_tekst(dc, "Paper Pilot", &mut r1, s.letters.titel, k.kop_tekst, DT_LEFT | DT_SINGLELINE | DT_NOPREFIX);
        let mut r2 = RECT { left: tx, top: iy + px(42, d), right: rc.right - rand, bottom: iy + px(70, d) };
        teken_tekst(dc, &t("Ondertitel"), &mut r2, s.letters.onder, k.kop_zacht, DT_LEFT | DT_SINGLELINE | DT_NOPREFIX | DT_END_ELLIPSIS);
        // Regel onderaan.
        let mut r3 = RECT { left: rand, top: rc.bottom - px(44, d), right: rc.right - rand, bottom: rc.bottom - px(12, d) };
        let voet = tf("Voetregel", &[crate::VERSIE]);
        teken_tekst(dc, &voet, &mut r3, s.letters.klein, k.zacht, DT_CENTER | DT_SINGLELINE | DT_NOPREFIX | DT_VCENTER);
        BitBlt(echt, 0, 0, rc.right, rc.bottom, dc, 0, 0, SRCCOPY);
        SelectObject(dc, oud);
        DeleteObject(bmp as _);
        DeleteDC(dc);
        EndPaint(hwnd, &ps);
    }
}

fn teken_tegel(di: &DRAWITEMSTRUCT) {
    let Some(s) = lees() else { return };
    unsafe {
        let k = s.kleuren;
        let d = dpi(s.hwnd);
        let rc = di.rcItem;
        let b = rc.right - rc.left;
        let h = rc.bottom - rc.top;
        let dc = CreateCompatibleDC(di.hDC);
        let bmp = CreateCompatibleBitmap(di.hDC, b.max(1), h.max(1));
        let oud = SelectObject(dc, bmp as _);
        let vlak = RECT { left: 0, top: 0, right: b, bottom: h };
        thema::vul(dc, &vlak, k.achter);
        let focus = di.itemState & ODS_FOCUS != 0;
        let ingedrukt = di.itemState & ODS_SELECTED != 0;
        let zweeft = ZWEEFT.with(|z| z.get()) == di.hwndItem;
        let vul = if ingedrukt { k.ingedrukt } else if zweeft { k.zweven } else { k.tegel };
        let (rand, dikte) = if focus { (k.focus, px(4, d)) } else if zweeft { (k.accent, px(2, d)) } else { (k.tegel_rand, px(1, d)) };
        thema::rond_vlak(dc, 0, 0, b, h, px(18, d), vul, rand, dikte);
        let lezen = di.CtlID as i32 == ID_LEZEN;
        let binnen = px(24, d);
        let tekstkleur = if k.hoog_contrast && (zweeft || ingedrukt) { GetSysColor(COLOR_HIGHLIGHTTEXT) } else { k.tekst };
        let zachtkleur = if k.hoog_contrast && (zweeft || ingedrukt) { GetSysColor(COLOR_HIGHLIGHTTEXT) } else { k.zacht };
        // Symbool: een document (lezen) of een lijst met vinkjes (controleren).
        let mut rs = RECT { left: binnen, top: binnen, right: binnen + px(48, d), bottom: binnen + px(44, d) };
        teken_tekst(dc, if lezen { "\u{E8A5}" } else { "\u{E9D5}" }, &mut rs, s.letters.symbool,
            if k.hoog_contrast { tekstkleur } else { k.accent }, DT_LEFT | DT_SINGLELINE | DT_NOPREFIX);
        // "Binnenkort" als label rechtsboven.
        let label = t("BinnenkortLabel");
        let lb = px(110, d);
        let lh = px(28, d);
        let lx = b - binnen - lb;
        let label_vul = if k.hoog_contrast { k.tekst } else { k.accent };
        let label_tekst = if k.hoog_contrast { k.achter } else if k.donker { k.achter } else { rgb(255, 255, 255) };
        thema::rond_vlak(dc, lx, binnen, lx + lb, binnen + lh, lh, label_vul, label_vul, 1);
        let mut rl = RECT { left: lx, top: binnen, right: lx + lb, bottom: binnen + lh };
        teken_tekst(dc, &label, &mut rl, s.letters.klein, label_tekst, DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);
        // Titel en omschrijving.
        let geen_streep = if di.itemState & ODS_NOACCEL != 0 { DT_HIDEPREFIX } else { 0 };
        let mut rt = RECT { left: binnen, top: binnen + px(56, d), right: b - binnen, bottom: binnen + px(90, d) };
        teken_tekst(dc, &t(if lezen { "TegelLezen" } else { "TegelControleren" }), &mut rt, s.letters.tegel_kop, tekstkleur,
            DT_LEFT | DT_SINGLELINE | geen_streep);
        let mut ro = RECT { left: binnen, top: binnen + px(94, d), right: b - binnen, bottom: h - px(12, d) };
        teken_tekst(dc, &t(if lezen { "TegelLezenOver" } else { "TegelControlerenOver" }), &mut ro, s.letters.tegel_tekst, zachtkleur,
            DT_LEFT | DT_WORDBREAK | DT_NOPREFIX | DT_END_ELLIPSIS);
        BitBlt(di.hDC, rc.left, rc.top, b, h, dc, 0, 0, SRCCOPY);
        SelectObject(dc, oud);
        DeleteObject(bmp as _);
        DeleteDC(dc);
    }
}

unsafe extern "system" fn tegel_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM, _id: usize, _data: usize) -> LRESULT {
    match msg {
        WM_MOUSEMOVE => {
            if ZWEEFT.with(|z| z.get()) != hwnd {
                let vorige = ZWEEFT.with(|z| z.replace(hwnd));
                if !vorige.is_null() {
                    InvalidateRect(vorige, std::ptr::null(), 0);
                }
                let mut tme = TRACKMOUSEEVENT { cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32, dwFlags: TME_LEAVE, hwndTrack: hwnd, dwHoverTime: 0 };
                TrackMouseEvent(&mut tme);
                InvalidateRect(hwnd, std::ptr::null(), 0);
            }
        }
        WM_MOUSELEAVE => {
            if ZWEEFT.with(|z| z.get()) == hwnd {
                ZWEEFT.with(|z| z.set(std::ptr::null_mut()));
                InvalidateRect(hwnd, std::ptr::null(), 0);
            }
        }
        WM_SETCURSOR => {
            SetCursor(LoadCursorW(std::ptr::null_mut(), IDC_HAND));
            return 1;
        }
        WM_ERASEBKGND => return 1,
        _ => {}
    }
    DefSubclassProc(hwnd, msg, wp, lp)
}

fn tekst_tonen(titel_sleutel: &str, tekst: &str) {
    tekstvenster::toon(hoofd_hwnd(), &t(titel_sleutel), tekst);
}

fn komt_later(hwnd: HWND, sleutel: &str) {
    dialogen::melding(hwnd, &t(sleutel), &t("KomtLaterTekst"), dialogen::ICOON_INFO);
}

fn zoek_updates(handmatig: bool) {
    let hwnd = hoofd_hwnd() as isize;
    std::thread::spawn(move || {
        let uitkomst = crate::bijwerken::zoek();
        let doos = Box::into_raw(Box::new(uitkomst));
        unsafe {
            if PostMessageW(hwnd as HWND, WM_UPDATE, handmatig as usize, doos as isize) == 0 {
                drop(Box::from_raw(doos));
            }
        }
    });
}

fn update_klaar(hwnd: HWND, handmatig: bool, uitkomst: crate::bijwerken::Uitkomst) {
    use crate::bijwerken::Uitkomst;
    match uitkomst {
        Uitkomst::Nieuw { versie, pad } => {
            if dialogen::ja_nee(hwnd, &tf("UpdateGevonden", &[&versie]), &tf("UpdateVraag", &[&versie, crate::VERSIE]), true) {
                if crate::bijwerken::installeer(&pad) {
                    unsafe { DestroyWindow(hwnd) };
                } else {
                    dialogen::melding(hwnd, &t("UpdateMislukt"), &t("UpdateMisluktTekst"), dialogen::ICOON_WAARSCHUWING);
                }
            }
        }
        Uitkomst::Actueel => {
            if handmatig {
                dialogen::melding(hwnd, &t("UpdateActueel"), &tf("UpdateActueelTekst", &[crate::VERSIE]), dialogen::ICOON_INFO);
            }
        }
        Uitkomst::Fout(_) => {
            if handmatig {
                dialogen::melding(hwnd, &t("UpdateFout"), &t("UpdateFoutTekst"), dialogen::ICOON_WAARSCHUWING);
            }
        }
    }
}

/// Na het tonen: rondleiding (nieuwe gebruiker), "wat is er nieuw" (na een update), updates zoeken.
fn na_start(hwnd: HWND) {
    let opties = met(|s| s.opties.clone()).unwrap_or_default();
    if let Some(pad) = opties.schermafbeelding.clone() {
        let _ = pad;
        unsafe { SetTimer(hwnd, TIMER_FOTO, 700, None) };
        return;
    }
    if opties.demo {
        return;
    }
    let vorige = instellingen::get("laatste_versie");
    if instellingen::eerste_start() || !instellingen::aan("rondleiding_gehad", false) {
        instellingen::zet_aan("rondleiding_gehad", true);
        instellingen::zet("laatste_versie", crate::VERSIE);
        dialogen::rondleiding(hwnd);
    } else if vorige.as_deref() != Some(crate::VERSIE) {
        instellingen::zet("laatste_versie", crate::VERSIE);
        let nieuw = crate::teksten::wijzigingen_sinds(vorige.as_deref().unwrap_or("0.0.0.0"));
        if !nieuw.is_empty() {
            tekstvenster::toon(hwnd, &tf("WatIsNieuw", &[crate::VERSIE]), &nieuw);
        }
    }
    if instellingen::aan("updates_bij_start", true) {
        let vandaag = logboek::nu()[..10].to_string();
        if instellingen::get("laatste_updatecontrole").as_deref() != Some(&vandaag) {
            instellingen::zet("laatste_updatecontrole", &vandaag);
            zoek_updates(false);
        }
    }
}

fn opdracht(hwnd: HWND, id: i32) {
    match id {
        ID_LEZEN => komt_later(hwnd, "TegelLezen"),
        ID_CONTROLEREN => komt_later(hwnd, "TegelControleren"),
        ID_AFSLUITEN => unsafe {
            DestroyWindow(hwnd);
        },
        ID_TAAL_NL | ID_TAAL_EN => {
            let taal = if id == ID_TAAL_EN { "en" } else { "nl" };
            instellingen::zet("taal", taal);
            crate::teksten::zet_taal(taal);
            zet_teksten();
            dialogen::melding(hwnd, &t("TaalGekozen"), &t("TaalGekozenTekst"), dialogen::ICOON_INFO);
        }
        ID_THEMA_AUTO | ID_THEMA_LICHT | ID_THEMA_DONKER => {
            let keuze = match id { ID_THEMA_LICHT => "licht", ID_THEMA_DONKER => "donker", _ => "automatisch" };
            instellingen::zet("thema", keuze);
            met(|s| s.opties.thema = None);
            herlaad_kleuren();
            vink_menu();
        }
        ID_UPDATES_START => {
            let aan = !instellingen::aan("updates_bij_start", true);
            instellingen::zet_aan("updates_bij_start", aan);
            vink_menu();
        }
        ID_HANDLEIDING => tekst_tonen("MenuHandleidingTitel", crate::teksten::document("handleiding")),
        ID_PRIVACY => tekst_tonen("MenuPrivacyTitel", crate::teksten::document("privacy")),
        ID_WIJZIGINGEN => tekst_tonen("MenuWijzigingenTitel", crate::teksten::document("wijzigingen")),
        ID_RONDLEIDING => dialogen::rondleiding(hwnd),
        ID_MEER => dialogen::meer_programmas(hwnd),
        ID_MAIL => mail(&tf("MailOnderwerp", &[crate::VERSIE]), &t("MailTekst")),
        ID_UPDATES_NU => zoek_updates(true),
        ID_OVER => dialogen::over(hwnd),
        _ => {}
    }
}

unsafe extern "system" fn venster_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_SIZE => {
            schik();
            InvalidateRect(hwnd, std::ptr::null(), 0);
            0
        }
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            teken_venster(hwnd);
            0
        }
        WM_DRAWITEM => {
            teken_tegel(&*(lp as *const DRAWITEMSTRUCT));
            1
        }
        WM_COMMAND => {
            opdracht(hwnd, (wp & 0xFFFF) as i32);
            0
        }
        WM_ACTIVATE => {
            if (wp & 0xFFFF) as u32 == WA_INACTIVE {
                let f = GetFocus();
                if !f.is_null() && IsChild(hwnd, f) != 0 {
                    LAATSTE_FOCUS.with(|l| l.set(f));
                }
            } else {
                let f = LAATSTE_FOCUS.with(|l| l.get());
                let doel = if !f.is_null() && IsWindow(f) != 0 { f } else { met(|s| s.tegels[0]).unwrap_or(std::ptr::null_mut()) };
                if !doel.is_null() {
                    SetFocus(doel);
                }
            }
            0
        }
        WM_START => {
            na_start(hwnd);
            0
        }
        WM_UPDATE => {
            let uitkomst = Box::from_raw(lp as *mut crate::bijwerken::Uitkomst);
            update_klaar(hwnd, wp != 0, *uitkomst);
            0
        }
        WM_TIMER if wp == TIMER_FOTO => {
            KillTimer(hwnd, TIMER_FOTO);
            if let Some(pad) = met(|s| s.opties.schermafbeelding.clone()).flatten() {
                if let Err(e) = crate::proef::schermafbeelding(hwnd, &pad) {
                    logboek::schrijf(&format!("Schermafbeelding: {e}"));
                }
            }
            DestroyWindow(hwnd);
            0
        }
        WM_SETTINGCHANGE | WM_SYSCOLORCHANGE | WM_THEMECHANGED => {
            herlaad_kleuren();
            DefWindowProcW(hwnd, msg, wp, lp)
        }
        WM_DPICHANGED => {
            let r = &*(lp as *const RECT);
            SetWindowPos(hwnd, std::ptr::null_mut(), r.left, r.top, r.right - r.left, r.bottom - r.top, SWP_NOZORDER | SWP_NOACTIVATE);
            met(|s| {
                s.letters.weg();
                s.letters = Letters::maak(dpi(hwnd));
            });
            schik();
            InvalidateRect(hwnd, std::ptr::null(), 1);
            0
        }
        WM_GETMINMAXINFO => {
            let mmi = &mut *(lp as *mut MINMAXINFO);
            let d = dpi(hwnd);
            mmi.ptMinTrackSize.x = px(420, d);
            mmi.ptMinTrackSize.y = px(520, d);
            0
        }
        WM_DESTROY => {
            if let Some(s) = lees() {
                s.letters.weg();
                DestroyAcceleratorTable(s.toetsen);
            }
            logboek::schrijf("Paper Pilot afgesloten.");
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

/// De letter na & in een tekst (voor Alt+letter op de tegels).
fn sneltoets(tekst: &str) -> Option<char> {
    let mut it = tekst.chars();
    while let Some(c) = it.next() {
        if c == '&' {
            return it.next().map(|c| c.to_ascii_lowercase());
        }
    }
    None
}

fn berichtenlus() -> i32 {
    let mut msg: MSG = unsafe { std::mem::zeroed() };
    loop {
        let r = unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) };
        if r <= 0 {
            break;
        }
        let hoofd = hoofd_hwnd();
        let boven = unsafe { GetAncestor(msg.hwnd, GA_ROOT) };
        if !hoofd.is_null() && boven == hoofd {
            let toetsen = met(|s| s.toetsen).unwrap_or(std::ptr::null_mut());
            if unsafe { TranslateAcceleratorW(hoofd, toetsen, &msg) } != 0 {
                continue;
            }
            // Alt+letter: eerst de tegels, anders het menu (gewone afhandeling door Windows).
            if msg.message == WM_SYSCHAR {
                let letter = char::from_u32(msg.wParam as u32).map(|c| c.to_ascii_lowercase());
                let tegel = [("TegelLezen", ID_LEZEN), ("TegelControleren", ID_CONTROLEREN)]
                    .iter().find(|(k, _)| sneltoets(&t(k)) == letter).map(|(_, id)| *id);
                if let Some(id) = tegel {
                    if let Some(s) = lees() {
                        unsafe { SetFocus(s.tegels[(id - 1) as usize]) };
                    }
                    unsafe { PostMessageW(hoofd, WM_COMMAND, id as usize, 0) };
                    continue;
                }
            } else if unsafe { IsDialogMessageW(hoofd, &msg) } != 0 {
                continue;
            }
        } else if tekstvenster::is_tekstvenster(boven) && unsafe { IsDialogMessageW(boven, &msg) } != 0 {
            continue;
        }
        unsafe {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    msg.wParam as i32
}
