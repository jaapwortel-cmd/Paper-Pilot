// Paper Pilot - een PDF-formulier invullen, vraag voor vraag.
//
// Bovenaan "Vraag 3 van 12", daaronder de vraag (de naam van het invulvak voor de schermlezer) en het invulvak:
// een gewoon tekstvak (ook om in te dicteren), een selectievakje, of een lijst met keuzes.
// Enter = Volgende, Escape = Sluiten. Knoppen: Volgende, Vorige, Overzicht (Ctrl+L), Opslaan (Ctrl+S), Sluiten.
// Bij vragen als naam, adres of BSN staat Mijn gegevens al ingevuld (als het veld nog leeg is).

use crate::formulier_pdf::{Soort, Veld};
use crate::teksten::{t, tf};
use crate::win::*;
use crate::{dialogen, keuzevensters, logboek};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::UI::Controls::*;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const KLASSE: &str = "PaperPilotInvullen";
const ID_VOLGENDE: i32 = IDOK;
const ID_SLUITEN: i32 = IDCANCEL;
const ID_VORIGE: i32 = 201;
const ID_OVERZICHT: i32 = 202;
const ID_OPSLAAN: i32 = 203;
const ID_INVOER: i32 = 210;

#[derive(Clone)]
struct Staat {
    hwnd: HWND,
    kop: HWND,
    voortgang: HWND,
    label: HWND,
    invoer: HWND,
    hint: HWND,
    knoppen: Vec<HWND>,
    letter: HFONT,
    kopletter: HFONT,
    kwast: HBRUSH,
    toetsen: HACCEL,
    velden: Vec<Veld>,
    /// Bij elk veld: kwam de waarde uit Mijn gegevens?
    voorgesteld: Vec<bool>,
    nr: usize,
    bron: PathBuf,
    naam: String,
    gewijzigd: bool,
}

thread_local! {
    static STAAT: RefCell<Option<Staat>> = const { RefCell::new(None) };
}

fn lees() -> Option<Staat> {
    STAAT.with(|s| s.borrow().clone())
}

fn met<R>(f: impl FnOnce(&mut Staat) -> R) -> Option<R> {
    STAAT.with(|s| s.borrow_mut().as_mut().map(f))
}

pub fn hwnd() -> HWND {
    lees().map(|s| s.hwnd).unwrap_or(std::ptr::null_mut())
}

pub fn toetsen() -> HACCEL {
    lees().map(|s| s.toetsen).unwrap_or(std::ptr::null_mut())
}

/// Opent het invulvenster voor dit PDF-formulier.
pub fn open(eigenaar: HWND, bron: &Path, naam: &str) {
    if let Some(s) = lees() {
        unsafe { SetForegroundWindow(s.hwnd) };
        return;
    }
    let pad = bron.to_path_buf();
    let velden = dialogen::werk(eigenaar, &t("InvullenTitel"), &t("InvullenZoeken"), move |_| crate::formulier_pdf::lees_velden(&pad));
    let mut velden = match velden {
        Some(Ok(v)) if !v.is_empty() => v,
        Some(Ok(_)) => {
            dialogen::melding(eigenaar, &t("GeenFormulier"), &t("GeenFormulierTekst"), dialogen::ICOON_INFO);
            return;
        }
        Some(Err(e)) => {
            dialogen::melding(eigenaar, &t("InvullenMislukt"), &e, dialogen::ICOON_WAARSCHUWING);
            return;
        }
        None => return,
    };
    // Mijn gegevens alvast invullen in lege velden.
    let gegevens = crate::gegevens::laad();
    let mut voorgesteld = vec![false; velden.len()];
    for (i, v) in velden.iter_mut().enumerate() {
        if v.soort == Soort::Tekst && v.waarde.is_empty() {
            if let Some(w) = crate::gegevens::soort_van(&v.vraag).and_then(|s| crate::gegevens::waarde(&gegevens, s)) {
                v.waarde = w;
                voorgesteld[i] = true;
            }
        }
    }
    logboek::schrijf(&format!("Formulier invullen: {} velden, {} uit Mijn gegevens.", velden.len(), voorgesteld.iter().filter(|x| **x).count()));
    maak(eigenaar, velden, voorgesteld, bron.to_path_buf(), naam);
}

fn maak(eigenaar: HWND, velden: Vec<Veld>, voorgesteld: Vec<bool>, bron: PathBuf, naam: &str) {
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
    let titel = w(&format!("{} - {naam} - Paper Pilot", t("InvullenTitel")));
    let hwnd = unsafe {
        CreateWindowExW(0, klasse.as_ptr(), titel.as_ptr(), WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
            CW_USEDEFAULT, CW_USEDEFAULT, 900, 620, eigenaar, std::ptr::null_mut(), instantie(), std::ptr::null())
    };
    if hwnd.is_null() {
        return;
    }
    let d = dpi(hwnd);
    let kleuren = crate::hoofdvenster::kleuren();
    crate::thema::titelbalk(hwnd, kleuren.donker);
    let letter = lettertype(13, false, d, "Segoe UI");
    let kopletter = lettertype(16, true, d, "Segoe UI");
    let maak = |klasse: &str, tekst: &str, stijl: u32, id: i32| unsafe {
        let k = w(klasse);
        let tt = w(tekst);
        let c = CreateWindowExW(0, k.as_ptr(), tt.as_ptr(), WS_CHILD | WS_VISIBLE | stijl, 0, 0, 10, 10, hwnd, id as isize as _,
            instantie(), std::ptr::null());
        zet_lettertype(c, letter);
        c
    };
    let kop = maak("STATIC", &tf("InvullenKop", &[naam]), 0x80 | 0x4000, -1);
    zet_lettertype(kop, kopletter);
    let voortgang = maak("STATIC", "", 0x80, -1);
    let hint = maak("STATIC", "", 0x80, -1);
    let mut knoppen = Vec::new();
    for (id, sleutel, stijl) in [
        (ID_VOLGENDE, "KnopVolgendeVraag", BS_DEFPUSHBUTTON),
        (ID_VORIGE, "KnopVorigeVraag", BS_PUSHBUTTON),
        (ID_OVERZICHT, "KnopOverzicht", BS_PUSHBUTTON),
        (ID_OPSLAAN, "KnopFormulierOpslaan", BS_PUSHBUTTON),
        (ID_SLUITEN, "KnopSluitenEsc", BS_PUSHBUTTON),
    ] {
        knoppen.push(maak("BUTTON", &t(sleutel), WS_TABSTOP | stijl as u32, id));
    }
    let toetsen = unsafe {
        let ctrl = (FVIRTKEY | FCONTROL) as u8;
        let a = [
            ACCEL { fVirt: ctrl, key: b'S' as u16, cmd: ID_OPSLAAN as u16 },
            ACCEL { fVirt: ctrl, key: b'L' as u16, cmd: ID_OVERZICHT as u16 },
            ACCEL { fVirt: (FVIRTKEY | FALT) as u8, key: VK_LEFT, cmd: ID_VORIGE as u16 },
            ACCEL { fVirt: (FVIRTKEY | FALT) as u8, key: VK_RIGHT, cmd: ID_VOLGENDE as u16 },
        ];
        CreateAcceleratorTableW(a.as_ptr(), a.len() as i32)
    };
    let kwast = unsafe { CreateSolidBrush(kleuren.vak) };
    STAAT.with(|s| *s.borrow_mut() = Some(Staat { hwnd, kop, voortgang, label: std::ptr::null_mut(), invoer: std::ptr::null_mut(), hint,
        knoppen, letter, kopletter, kwast, toetsen, velden, voorgesteld, nr: 0, bron, naam: naam.to_string(), gewijzigd: false }));
    unsafe { ShowWindow(hwnd, SW_SHOWNORMAL) };
    toon_vraag(0);
}

/// De waarde uit het invulvak terug in het veld zetten.
fn bewaar_huidige() {
    let Some(s) = lees() else { return };
    if s.invoer.is_null() {
        return;
    }
    let v = &s.velden[s.nr];
    let nieuw = match v.soort {
        Soort::Tekst => lees_tekst(s.invoer),
        Soort::Vinkje => {
            let aan = unsafe { SendMessageW(s.invoer, BM_GETCHECK, 0, 0) } == BST_CHECKED as isize;
            if aan { "1".into() } else { String::new() }
        }
        Soort::Rondje | Soort::Lijst => {
            let i = unsafe { SendMessageW(s.invoer, LB_GETCURSEL, 0, 0) };
            // De eerste regel is "(niets gekozen)".
            if i <= 0 { String::new() } else { (i - 1).to_string() }
        }
    };
    met(|st| {
        if st.velden[st.nr].waarde != nieuw {
            st.velden[st.nr].waarde = nieuw;
            st.voorgesteld[st.nr] = false;
            st.gewijzigd = true;
        }
    });
}

fn toon_vraag(nr: usize) {
    let Some(s) = lees() else { return };
    let nr = nr.min(s.velden.len().saturating_sub(1));
    unsafe {
        if !s.label.is_null() {
            DestroyWindow(s.label);
        }
        if !s.invoer.is_null() {
            DestroyWindow(s.invoer);
        }
    }
    let v = s.velden[nr].clone();
    let vraagtekst = if v.verplicht { tf("VraagVerplicht", &[&v.vraag]) } else { v.vraag.clone() };
    let maak = |klasse: &str, tekst: &str, stijl: u32, ex: u32| unsafe {
        let k = w(klasse);
        let tt = w(tekst);
        let c = CreateWindowExW(ex, k.as_ptr(), tt.as_ptr(), WS_CHILD | WS_VISIBLE | stijl, 0, 0, 10, 10, s.hwnd, ID_INVOER as isize as _,
            instantie(), std::ptr::null());
        zet_lettertype(c, s.letter);
        c
    };
    // Eerst de vraag, dan het invulvak: zo noemt de schermlezer de vraag als naam van het invulvak.
    let label = if v.soort == Soort::Vinkje { std::ptr::null_mut() } else { maak("STATIC", &vraagtekst, 0x80, 0) };
    let invoer = match v.soort {
        Soort::Tekst => {
            let stijl = WS_TABSTOP | if v.meerregelig { (ES_MULTILINE | ES_AUTOVSCROLL | ES_WANTRETURN) as u32 | WS_VSCROLL } else { ES_AUTOHSCROLL as u32 };
            let e = maak("EDIT", &regels(&v.waarde), stijl, WS_EX_CLIENTEDGE);
            unsafe { SendMessageW(e, EM_SETSEL, 0, -1) };
            e
        }
        Soort::Vinkje => {
            let c = maak("BUTTON", &vraagtekst, WS_TABSTOP | BS_AUTOCHECKBOX as u32 | BS_MULTILINE as u32, 0);
            unsafe { SendMessageW(c, BM_SETCHECK, if v.waarde.is_empty() { BST_UNCHECKED } else { BST_CHECKED } as usize, 0) };
            c
        }
        Soort::Rondje | Soort::Lijst => {
            let l = maak("LISTBOX", "", WS_TABSTOP | WS_VSCROLL | WS_BORDER | (LBS_NOTIFY | LBS_NOINTEGRALHEIGHT) as u32, 0);
            let mut opties = vec![t("NietsGekozen")];
            opties.extend(v.opties.iter().cloned());
            for o in &opties {
                let tt = w(o);
                unsafe { SendMessageW(l, LB_ADDSTRING, 0, tt.as_ptr() as LPARAM) };
            }
            let keuze = v.waarde.parse::<usize>().map(|k| k + 1).unwrap_or(0);
            unsafe { SendMessageW(l, LB_SETCURSEL, keuze, 0) };
            l
        }
    };
    // Vooraan in de Tab-volgorde.
    unsafe {
        if !label.is_null() {
            SetWindowPos(label, HWND_TOP, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
            SetWindowPos(invoer, label, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
        } else {
            SetWindowPos(invoer, HWND_TOP, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
        }
    }
    zet_tekst(s.voortgang, &tf("VraagVan", &[&(nr + 1).to_string(), &s.velden.len().to_string()]));
    let hint = if s.voorgesteld[nr] { t("HintMijnGegevens") } else if v.soort == Soort::Lijst || v.soort == Soort::Rondje {
        t("HintKiezen") } else if v.soort == Soort::Vinkje { t("HintVinkje") } else { t("HintTypen") };
    zet_tekst(s.hint, &hint);
    met(|st| {
        st.nr = nr;
        st.label = label;
        st.invoer = invoer;
    });
    schik();
    unsafe { SetFocus(invoer) };
}

fn schik() {
    let Some(s) = lees() else { return };
    let mut rc: RECT = unsafe { std::mem::zeroed() };
    unsafe { GetClientRect(s.hwnd, &mut rc) };
    let d = dpi(s.hwnd);
    let m = px(20, d);
    let b = rc.right - 2 * m;
    let v = s.velden.get(s.nr);
    let groot = v.map(|v| v.meerregelig || v.soort == Soort::Lijst || v.soort == Soort::Rondje).unwrap_or(false);
    unsafe {
        MoveWindow(s.kop, m, m, b, px(34, d), 1);
        MoveWindow(s.voortgang, m, m + px(40, d), b, px(26, d), 1);
        let mut y = m + px(84, d);
        if !s.label.is_null() {
            MoveWindow(s.label, m, y, b, px(56, d), 1);
            y += px(60, d);
        }
        let h = if groot { px(180, d) } else if v.map(|v| v.soort == Soort::Vinkje).unwrap_or(false) { px(56, d) } else { px(38, d) };
        MoveWindow(s.invoer, m, y, b, h, 1);
        y += h + px(10, d);
        MoveWindow(s.hint, m, y, b, px(48, d), 1);
        let kh = px(44, d);
        let mut x = m;
        let ky = rc.bottom - m - kh;
        for k in &s.knoppen {
            let kb = px(160, d);
            MoveWindow(*k, x, ky, kb, kh, 1);
            x += kb + px(10, d);
        }
    }
}

fn volgende() {
    bewaar_huidige();
    let Some(s) = lees() else { return };
    if s.nr + 1 < s.velden.len() {
        toon_vraag(s.nr + 1);
    } else {
        dialogen::melding(s.hwnd, &t("LaatsteVraag"), &t("LaatsteVraagTekst"), dialogen::ICOON_INFO);
        unsafe { SetFocus(s.knoppen[3]) };
    }
}

fn vorige() {
    bewaar_huidige();
    let Some(s) = lees() else { return };
    if s.nr > 0 {
        toon_vraag(s.nr - 1);
    } else {
        unsafe { SetFocus(s.invoer) };
    }
}

fn waarde_tekst(v: &Veld) -> String {
    match v.soort {
        Soort::Tekst => if v.waarde.is_empty() { t("Leeg") } else { v.waarde.replace(['\r', '\n'], " ") },
        Soort::Vinkje => if v.waarde.is_empty() { t("NietAangevinkt") } else { t("Aangevinkt") },
        _ => v.waarde.parse::<usize>().ok().and_then(|k| v.opties.get(k).cloned()).unwrap_or_else(|| t("Leeg")),
    }
}

fn overzicht() {
    bewaar_huidige();
    let Some(s) = lees() else { return };
    let lijst: Vec<String> = s.velden.iter().enumerate().map(|(i, v)| format!("{}. {}: {}", i + 1, v.vraag, waarde_tekst(v))).collect();
    if let Some(k) = keuzevensters::kies_uit_lijst(s.hwnd, &t("OverzichtTitel"), &t("OverzichtLabel"), &lijst, s.nr) {
        toon_vraag(k);
    } else {
        unsafe { SetFocus(s.invoer) };
    }
}

fn opslaan() {
    bewaar_huidige();
    let Some(s) = lees() else { return };
    let leeg: Vec<&Veld> = s.velden.iter().filter(|v| v.verplicht && v.waarde.is_empty()).collect();
    if !leeg.is_empty()
        && !dialogen::ja_nee(s.hwnd, &tf("VerplichtLeeg", &[&leeg.len().to_string()]), &t("VerplichtLeegTekst"), false)
    {
        let eerste = s.velden.iter().position(|v| v.verplicht && v.waarde.is_empty()).unwrap_or(0);
        toon_vraag(eerste);
        return;
    }
    let stam = Path::new(&s.naam).file_stem().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
    let voorstel = format!("{stam} {}.pdf", t("Ingevuld"));
    let Some(doel) = crate::leesvenster::opslaan_als_pdf(s.hwnd, &voorstel) else {
        unsafe { SetFocus(s.invoer) };
        return;
    };
    let (bron, velden, d2) = (s.bron.clone(), s.velden.clone(), doel.clone());
    let uitkomst = dialogen::werk(s.hwnd, &t("InvullenTitel"), &t("FormulierBewaren"), move |_| crate::formulier_pdf::vul_in(&bron, &velden, &d2));
    match uitkomst {
        Some(Ok(())) => {
            met(|st| st.gewijzigd = false);
            logboek::schrijf("Ingevuld formulier opgeslagen.");
            dialogen::melding(s.hwnd, &t("FormulierOpgeslagen"), &tf("FormulierOpgeslagenTekst", &[&doel.to_string_lossy()]), dialogen::ICOON_INFO);
            bied_gegevens_aan(&s.velden);
        }
        Some(Err(e)) => dialogen::melding(s.hwnd, &t("OpslaanMislukt"), &e, dialogen::ICOON_WAARSCHUWING),
        None => {}
    }
    unsafe { SetFocus(s.invoer) };
}

/// Na het opslaan: nieuwe of andere persoonlijke gegevens bewaren in Mijn gegevens?
fn bied_gegevens_aan(velden: &[Veld]) {
    let Some(s) = lees() else { return };
    let mut gegevens = crate::gegevens::laad();
    let mut nieuw: BTreeMap<String, String> = BTreeMap::new();
    for v in velden.iter().filter(|v| v.soort == Soort::Tekst && !v.waarde.trim().is_empty()) {
        if let Some(soort) = crate::gegevens::soort_van(&v.vraag) {
            if crate::gegevens::SOORTEN.contains(&soort) && gegevens.get(soort).map(|g| g != v.waarde.trim()).unwrap_or(true) {
                nieuw.insert(soort.to_string(), v.waarde.trim().to_string());
            }
        }
    }
    if nieuw.is_empty() {
        return;
    }
    let lijst: Vec<String> = nieuw.iter().map(|(k, v)| format!("{}: {v}", t(&format!("Gegeven_{k}")))).collect();
    if dialogen::ja_nee(s.hwnd, &t("GegevensBewarenVraag"), &tf("GegevensBewarenTekst", &[&lijst.join("\n")]), false) {
        gegevens.extend(nieuw);
        if let Err(e) = crate::gegevens::bewaar(&gegevens) {
            dialogen::melding(s.hwnd, &t("OpslaanMislukt"), &e, dialogen::ICOON_WAARSCHUWING);
        }
    }
}

fn sluiten(hwnd: HWND) {
    bewaar_huidige();
    let gewijzigd = lees().map(|s| s.gewijzigd).unwrap_or(false);
    if gewijzigd && !dialogen::ja_nee(hwnd, &t("NietOpgeslagen"), &t("NietOpgeslagenTekst"), false) {
        if let Some(s) = lees() {
            unsafe { SetFocus(s.invoer) };
        }
        return;
    }
    unsafe { DestroyWindow(hwnd) };
}

unsafe extern "system" fn venster_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_SIZE => {
            schik();
            0
        }
        WM_COMMAND => {
            let id = (wp & 0xFFFF) as i32;
            let melding = ((wp >> 16) & 0xFFFF) as u32;
            match id {
                ID_VOLGENDE => volgende(),
                ID_VORIGE => vorige(),
                ID_OVERZICHT => overzicht(),
                ID_OPSLAAN => opslaan(),
                ID_SLUITEN => sluiten(hwnd),
                ID_INVOER if melding == 2 => volgende(), // dubbelklik in de lijst
                _ => {}
            }
            0
        }
        WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX | WM_CTLCOLORBTN => {
            let k = crate::hoofdvenster::kleuren();
            let Some(s) = lees() else { return DefWindowProcW(hwnd, msg, wp, lp) };
            SetTextColor(wp as HDC, if lp as HWND == s.hint || lp as HWND == s.voortgang { k.zacht } else { k.tekst });
            if msg == WM_CTLCOLOREDIT || msg == WM_CTLCOLORLISTBOX {
                SetBkColor(wp as HDC, k.vak);
                return s.kwast as LRESULT;
            }
            SetBkColor(wp as HDC, k.achter);
            SetDCBrushColor(wp as HDC, k.achter);
            GetStockObject(DC_BRUSH) as LRESULT
        }
        WM_ERASEBKGND => {
            let mut rc: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut rc);
            crate::thema::vul(wp as HDC, &rc, crate::hoofdvenster::kleuren().achter);
            1
        }
        WM_ACTIVATE => {
            if (wp & 0xFFFF) as u32 != WA_INACTIVE {
                if let Some(s) = lees() {
                    let f = GetFocus();
                    if f.is_null() || IsChild(hwnd, f) == 0 {
                        SetFocus(s.invoer);
                    }
                }
            }
            0
        }
        WM_CLOSE => {
            sluiten(hwnd);
            0
        }
        WM_DESTROY => {
            if let Some(s) = lees() {
                weg(s.letter as _);
                weg(s.kopletter as _);
                weg(s.kwast as _);
                DestroyAcceleratorTable(s.toetsen);
            }
            STAAT.with(|s| *s.borrow_mut() = None);
            let lees_hwnd = crate::leesvenster::hwnd();
            if !lees_hwnd.is_null() {
                SetForegroundWindow(lees_hwnd);
            }
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}
