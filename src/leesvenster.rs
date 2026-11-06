// Paper Pilot - het leesvenster: een document openen en lezen.
//
// Bovenaan de naam van het document (ook de naam van het tekstvak voor de schermlezer) en een regel met
// het aantal pagina's, koppen en invulvelden. Daaronder knoppen, en het grote tekstvak (alleen lezen) met de tekst.
// Sneltoetsen (nooit Insert of Caps Lock): Ctrl+O openen, Ctrl+F zoeken, F3 en Shift+F3 volgende/vorige,
// Ctrl+K koppen, Ctrl+G naar pagina, Ctrl+S opslaan als tekst, Escape sluiten.
// Een document openen gebeurt op de achtergrond, zodat het venster blijft reageren.

use crate::lezen::Document;
use crate::teksten::{t, tf};
use crate::win::*;
use crate::{dialogen, instellingen, keuzevensters, logboek};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::UI::Controls::Dialogs::*;
use windows_sys::Win32::UI::Controls::*;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::Shell::SetWindowSubclass;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const KLASSE: &str = "PaperPilotLezen";
const ID_TEKST: i32 = 10;
const ID_OPENEN: i32 = 101;
const ID_ZOEKEN: i32 = 102;
const ID_VOLGENDE: i32 = 103;
const ID_VORIGE: i32 = 104;
const ID_KOPPEN: i32 = 105;
const ID_PAGINA: i32 = 106;
const ID_OPSLAAN: i32 = 107;
const ID_BEELD: i32 = 108;
const ID_HANDSCHRIFT: i32 = 109;
const ID_BESCHRIJF: i32 = 110;
const ID_SCAN: i32 = 111;
const ID_AFDRUK: i32 = 112;
const ID_SLUITEN: i32 = IDCANCEL;
const WM_GELADEN: u32 = WM_APP + 10;
const WM_VOORTGANG: u32 = WM_APP + 11;

const KNOPPEN: [(i32, &str); 11] = [
    (ID_OPENEN, "KnopOpenen"),
    (ID_SCAN, "KnopScannen"),
    (ID_AFDRUK, "KnopAfdrukken"),
    (ID_HANDSCHRIFT, "KnopHandschrift"),
    (ID_BESCHRIJF, "KnopBeschrijven"),
    (ID_ZOEKEN, "KnopZoeken"),
    (ID_KOPPEN, "KnopKoppen"),
    (ID_PAGINA, "KnopPagina"),
    (ID_OPSLAAN, "KnopOpslaanTekst"),
    (ID_BEELD, "KnopOpslaanBeeld"),
    (ID_SLUITEN, "KnopSluitenEsc"),
];

#[derive(Clone)]
struct Staat {
    hwnd: HWND,
    kop: HWND,
    status: HWND,
    vak: HWND,
    knoppen: Vec<HWND>,
    letter: HFONT,
    kopletter: HFONT,
    kwast: HBRUSH,
    toetsen: HACCEL,
    doc: Option<Document>,
    pad: Option<PathBuf>,
    zoekwoord: String,
    bezig: bool,
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

/// Het leesvenster (als het open is), voor de berichtenlus.
pub fn hwnd() -> HWND {
    lees().map(|s| s.hwnd).unwrap_or(std::ptr::null_mut())
}

pub fn toetsen() -> HACCEL {
    lees().map(|s| s.toetsen).unwrap_or(std::ptr::null_mut())
}

/// Opent het leesvenster en laat meteen een document kiezen.
pub fn open_en_kies(eigenaar: HWND) {
    let h = maak(eigenaar);
    if h.is_null() {
        return;
    }
    // Een bestand openen of scannen?
    let titel = t("Lezen");
    let kop = t("KeuzeKop");
    let tekst = t("KeuzeTekst");
    let mut v = dialogen::Vraag::nieuw(&titel, &kop, &tekst);
    v.opdrachtlinks = true;
    v.knoppen.push(dialogen::Knop { id: 3001, tekst: t("KeuzeOpenen") });
    v.knoppen.push(dialogen::Knop { id: 3002, tekst: t("KeuzeScannen") });
    v.gewone_knoppen = windows_sys::Win32::UI::Controls::TDCBF_CANCEL_BUTTON;
    v.standaard = 3001;
    match v.toon(h) {
        3001 => kies_bestand(),
        3002 => scannen(),
        _ => unsafe {
            if let Some(s) = lees() {
                SetFocus(s.vak);
            }
        },
    }
}

/// Scannen: scanner zoeken (en kiezen), glasplaat of documentinvoer, pagina's scannen, en de tekst herkennen.
fn scannen() {
    let Some(s) = lees() else { return };
    let hwnd = s.hwnd;
    let terug = || unsafe {
        SetFocus(s.vak);
    };
    let Some(scanners) = dialogen::werk(hwnd, &t("ScanTitel"), &t("ScannersZoeken"), |v| {
        v.meld(t("ScannersZoekenTekst"), 0);
        crate::scanner::zoek()
    }) else { return terug() };
    if scanners.is_empty() {
        dialogen::melding(hwnd, &t("GeenScanner"), &t("GeenScannerTekst"), dialogen::ICOON_WAARSCHUWING);
        return terug();
    }
    let scanner = if scanners.len() == 1 {
        scanners[0].clone()
    } else {
        let namen: Vec<String> = scanners.iter().map(|s| s.naam.clone()).collect();
        let vorige = instellingen::get("scanner").and_then(|n| namen.iter().position(|x| *x == n)).unwrap_or(0);
        match keuzevensters::kies_uit_lijst(hwnd, &t("ScanTitel"), &t("ScannerKiezen"), &namen, vorige) {
            Some(i) => scanners[i].clone(),
            None => return terug(),
        }
    };
    instellingen::zet("scanner", &scanner.naam);
    let invoer = if scanner.glasplaat && scanner.invoer {
        let keuzes = vec![t("BronGlasplaat"), t("BronInvoer")];
        let vorige = if instellingen::aan("scan_invoer", false) { 1 } else { 0 };
        match keuzevensters::kies_uit_lijst(hwnd, &t("ScanTitel"), &t("BronKiezen"), &keuzes, vorige) {
            Some(i) => i == 1,
            None => return terug(),
        }
    } else {
        scanner.invoer && !scanner.glasplaat
    };
    instellingen::zet_aan("scan_invoer", invoer);
    let mut beelden = Vec::new();
    loop {
        let sc = scanner.clone();
        let uitkomst = dialogen::werk(hwnd, &t("ScanTitel"), &t("ScanBezig"), move |v| {
            v.meld(tf("ScanBezigTekst", &[&sc.naam]), 0);
            crate::scanner::scan(&sc, invoer)
        });
        match uitkomst {
            Some(Ok(b)) if !b.is_empty() => beelden.extend(b),
            Some(Ok(_)) => {
                dialogen::melding(hwnd, &t("ScanLeeg"), &t("ScanLeegTekst"), dialogen::ICOON_WAARSCHUWING);
            }
            Some(Err(e)) => {
                dialogen::melding(hwnd, &t("ScanMislukt"), &tf("ScanMisluktTekst", &[&e]), dialogen::ICOON_WAARSCHUWING);
            }
            None => {}
        }
        if invoer || beelden.is_empty() || !dialogen::ja_nee(hwnd, &tf("ScanNogEen", &[&beelden.len().to_string()]), &t("ScanNogEenTekst"), false) {
            break;
        }
    }
    if beelden.is_empty() {
        return terug();
    }
    let naam = format!("{} {}", t("ScanNaam"), crate::logboek::nu().replace(':', ".").chars().take(16).collect::<String>());
    let n = naam.clone();
    let doc = dialogen::werk(hwnd, &t("ScanTitel"), &t("ScanHerkennen"), move |v| {
        crate::lezen::van_beelden(&n, &beelden, &|tekst| v.meld(tekst, 0))
    });
    if let Some(doc) = doc {
        logboek::schrijf(&format!("Gescand: {} pagina('s).", doc.beelden.len()));
        geladen(Ok((doc, PathBuf::new())));
        dialogen::melding(hwnd, &t("ScanKlaar"), &t("ScanKlaarTekst"), dialogen::ICOON_INFO);
        terug();
    }
}

/// Afdrukken: de tekst, of (bij een scan of foto) de afbeelding.
fn afdrukken() {
    let Some(s) = lees() else { return };
    let Some(doc) = s.doc.clone() else { return };
    let afbeelding = if doc.beelden.is_empty() {
        false
    } else {
        let keuzes = vec![t("AfdrukTekst"), t("AfdrukBeeld")];
        match keuzevensters::kies_uit_lijst(s.hwnd, &t("AfdrukTitel"), &t("AfdrukKiezen"), &keuzes, 0) {
            Some(i) => i == 1,
            None => {
                unsafe { SetFocus(s.vak) };
                return;
            }
        }
    };
    let uitkomst = if afbeelding {
        crate::afdrukken::beelden(s.hwnd, &doc.naam, &doc.beelden)
    } else {
        crate::afdrukken::tekst(s.hwnd, &doc.naam, &doc.tekst)
    };
    match uitkomst {
        Some(Ok(n)) => dialogen::melding(s.hwnd, &t("Afgedrukt"), &tf("AfgedruktTekst", &[&n.to_string()]), dialogen::ICOON_INFO),
        Some(Err(e)) => dialogen::melding(s.hwnd, &t("AfdrukkenMislukt"), &e, dialogen::ICOON_WAARSCHUWING),
        None => {}
    }
    unsafe { SetFocus(s.vak) };
}

/// Opent het leesvenster met dit bestand (bijvoorbeeld via "Openen met").
pub fn open_bestand(eigenaar: HWND, pad: &Path) {
    let h = maak(eigenaar);
    if !h.is_null() {
        laad(pad.to_path_buf());
    }
}

fn maak(eigenaar: HWND) -> HWND {
    if let Some(s) = lees() {
        unsafe {
            if IsIconic(s.hwnd) != 0 {
                ShowWindow(s.hwnd, SW_RESTORE);
            }
            SetForegroundWindow(s.hwnd);
        }
        return s.hwnd;
    }
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
    let titel = w(&format!("{} - Paper Pilot", t("Lezen")));
    let hwnd = unsafe {
        CreateWindowExW(0, klasse.as_ptr(), titel.as_ptr(), WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
            CW_USEDEFAULT, CW_USEDEFAULT, 1000, 760, eigenaar, std::ptr::null_mut(), instantie(), std::ptr::null())
    };
    if hwnd.is_null() {
        return hwnd;
    }
    let d = dpi(hwnd);
    let kleuren = crate::hoofdvenster::kleuren();
    crate::thema::titelbalk(hwnd, kleuren.donker);
    let letter = lettertype(12, false, d, "Segoe UI");
    let kopletter = lettertype(16, true, d, "Segoe UI");
    let maak = |klasse: &str, tekst: &str, stijl: u32, ex: u32, id: i32| unsafe {
        let k = w(klasse);
        let t = w(tekst);
        let c = CreateWindowExW(ex, k.as_ptr(), t.as_ptr(), WS_CHILD | WS_VISIBLE | stijl, 0, 0, 10, 10, hwnd,
            id as isize as _, instantie(), std::ptr::null());
        zet_lettertype(c, letter);
        c
    };
    // Volgorde = Tab-volgorde: eerst de kop (naam van het tekstvak), dan het tekstvak, dan de knoppen.
    let kop = maak("STATIC", &t("LezenGeenDocument"), 0x80 | 0x4000, 0, -1);
    zet_lettertype(kop, kopletter);
    let vak = maak("EDIT", "", WS_TABSTOP | WS_VSCROLL | (ES_MULTILINE | ES_READONLY | ES_AUTOVSCROLL | ES_NOHIDESEL) as u32,
        WS_EX_CLIENTEDGE, ID_TEKST);
    let status = maak("STATIC", "", 0x80 | 0x4000, 0, -1);
    let mut knoppen = Vec::new();
    for (i, (id, sleutel)) in KNOPPEN.iter().enumerate() {
        knoppen.push(maak("BUTTON", &t(sleutel), WS_TABSTOP | if i == 0 { WS_GROUP } else { 0 } | BS_PUSHBUTTON as u32, 0, *id));
    }
    unsafe {
        SetWindowSubclass(vak, Some(crate::tekstvenster::vak_proc), 1, 0);
        SendMessageW(vak, EM_SETLIMITTEXT, 0, 0);
    }
    let toetsen = unsafe {
        let ctrl = (FVIRTKEY | FCONTROL) as u8;
        let a = [
            ACCEL { fVirt: ctrl, key: b'O' as u16, cmd: ID_OPENEN as u16 },
            ACCEL { fVirt: ctrl, key: b'F' as u16, cmd: ID_ZOEKEN as u16 },
            ACCEL { fVirt: FVIRTKEY as u8, key: VK_F3, cmd: ID_VOLGENDE as u16 },
            ACCEL { fVirt: (FVIRTKEY | FSHIFT) as u8, key: VK_F3, cmd: ID_VORIGE as u16 },
            ACCEL { fVirt: ctrl, key: b'K' as u16, cmd: ID_KOPPEN as u16 },
            ACCEL { fVirt: ctrl, key: b'G' as u16, cmd: ID_PAGINA as u16 },
            ACCEL { fVirt: ctrl, key: b'H' as u16, cmd: ID_HANDSCHRIFT as u16 },
            ACCEL { fVirt: ctrl, key: b'N' as u16, cmd: ID_SCAN as u16 },
            ACCEL { fVirt: ctrl, key: b'P' as u16, cmd: ID_AFDRUK as u16 },
            ACCEL { fVirt: ctrl, key: b'D' as u16, cmd: ID_BESCHRIJF as u16 },
            ACCEL { fVirt: ctrl, key: b'S' as u16, cmd: ID_OPSLAAN as u16 },
            ACCEL { fVirt: (FVIRTKEY | FCONTROL | FSHIFT) as u8, key: b'S' as u16, cmd: ID_BEELD as u16 },
        ];
        CreateAcceleratorTableW(a.as_ptr(), a.len() as i32)
    };
    let kwast = unsafe { CreateSolidBrush(kleuren.vak) };
    STAAT.with(|s| *s.borrow_mut() = Some(Staat { hwnd, kop, status, vak, knoppen, letter, kopletter, kwast, toetsen,
        doc: None, pad: None, zoekwoord: String::new(), bezig: false }));
    schik();
    unsafe {
        ShowWindow(hwnd, SW_SHOWMAXIMIZED);
        SetFocus(vak);
    }
    hwnd
}

fn schik() {
    let Some(s) = lees() else { return };
    let mut rc: RECT = unsafe { std::mem::zeroed() };
    unsafe { GetClientRect(s.hwnd, &mut rc) };
    let d = dpi(s.hwnd);
    let m = px(16, d);
    let mut y = m;
    unsafe {
        MoveWindow(s.kop, m, y, rc.right - 2 * m, px(34, d), 1);
        y += px(36, d);
        MoveWindow(s.status, m, y, rc.right - 2 * m, px(24, d), 1);
        y += px(32, d);
    }
    // Knoppen naast elkaar; past het niet, dan op een volgende rij.
    let kh = px(40, d);
    let mut x = m;
    let dc = unsafe { GetDC(s.hwnd) };
    let oud = unsafe { SelectObject(dc, s.letter as _) };
    for k in &s.knoppen {
        let tekst: Vec<u16> = lees_tekst(*k).encode_utf16().collect();
        let mut maat = SIZE { cx: 0, cy: 0 };
        unsafe { GetTextExtentPoint32W(dc, tekst.as_ptr(), tekst.len() as i32, &mut maat) };
        let b = maat.cx + px(32, d);
        if x + b > rc.right - m && x > m {
            x = m;
            y += kh + px(8, d);
        }
        unsafe { MoveWindow(*k, x, y, b, kh, 1) };
        x += b + px(8, d);
    }
    unsafe {
        SelectObject(dc, oud);
        ReleaseDC(s.hwnd, dc);
    }
    y += kh + px(12, d);
    unsafe { MoveWindow(s.vak, m, y, rc.right - 2 * m, (rc.bottom - m - y).max(px(60, d)), 1) };
}

fn kies_bestand() {
    let Some(s) = lees() else { return };
    let map = instellingen::get("laatste_map").unwrap_or_default();
    if let Some(pad) = open_venster(s.hwnd, &map) {
        laad(pad);
    }
}

/// Het gewone Windows-venster "Openen".
fn open_venster(eigenaar: HWND, map: &str) -> Option<PathBuf> {
    let beelden = crate::beeld::EXTENSIES.iter().map(|e| format!("*.{e}")).collect::<Vec<_>>().join(";");
    let filter: Vec<u16> = format!("{}\0*.pdf;*.docx;*.txt;{beelden}\0PDF\0*.pdf\0Word (.docx)\0*.docx\0{}\0{beelden}\0{}\0*.txt\0{}\0*.*\0\0",
        t("FilterDocumenten"), t("FilterAfbeeldingen"), t("FilterTekst"), t("FilterAlles")).encode_utf16().collect();
    let mut buf = vec![0u16; 32768];
    let titel = w(&t("OpenenTitel"));
    let begin = w(map);
    let mut ofn: OPENFILENAMEW = unsafe { std::mem::zeroed() };
    ofn.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    ofn.hwndOwner = eigenaar;
    ofn.lpstrFilter = filter.as_ptr();
    ofn.lpstrFile = buf.as_mut_ptr();
    ofn.nMaxFile = buf.len() as u32;
    ofn.lpstrTitle = titel.as_ptr();
    if !map.is_empty() {
        ofn.lpstrInitialDir = begin.as_ptr();
    }
    ofn.Flags = OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_HIDEREADONLY | OFN_EXPLORER;
    if unsafe { GetOpenFileNameW(&mut ofn) } == 0 {
        return None;
    }
    let n = buf.iter().position(|&c| c == 0).unwrap_or(0);
    Some(PathBuf::from(String::from_utf16_lossy(&buf[..n])))
}

/// Het gewone Windows-venster "Opslaan als".
fn opslaan_venster(eigenaar: HWND, naam: &str) -> Option<PathBuf> {
    let filter = format!("{}\0*.txt\0\0", t("FilterTekst"));
    opslaan_als(eigenaar, naam, &filter, "txt")
}

fn opslaan_als(eigenaar: HWND, naam: &str, filter: &str, standaard_ext: &str) -> Option<PathBuf> {
    let filter: Vec<u16> = filter.encode_utf16().collect();
    let mut buf = vec![0u16; 32768];
    for (i, c) in naam.encode_utf16().take(buf.len() - 1).enumerate() {
        buf[i] = c;
    }
    let ext = w(standaard_ext);
    let mut ofn: OPENFILENAMEW = unsafe { std::mem::zeroed() };
    ofn.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    ofn.hwndOwner = eigenaar;
    ofn.lpstrFilter = filter.as_ptr();
    ofn.lpstrFile = buf.as_mut_ptr();
    ofn.nMaxFile = buf.len() as u32;
    ofn.lpstrDefExt = ext.as_ptr();
    ofn.Flags = OFN_OVERWRITEPROMPT | OFN_PATHMUSTEXIST | OFN_HIDEREADONLY | OFN_EXPLORER;
    if unsafe { GetSaveFileNameW(&mut ofn) } == 0 {
        return None;
    }
    let n = buf.iter().position(|&c| c == 0).unwrap_or(0);
    Some(PathBuf::from(String::from_utf16_lossy(&buf[..n])))
}

/// Opent een document op de achtergrond.
fn laad(pad: PathBuf) {
    let Some(s) = lees() else { return };
    if s.bezig {
        return;
    }
    if let Some(map) = pad.parent() {
        instellingen::zet("laatste_map", &map.to_string_lossy());
    }
    let naam = pad.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    met(|s| s.bezig = true);
    zet_tekst(s.status, &tf("LezenBezig", &[&naam]));
    zet_tekst(s.vak, &tf("LezenBezig", &[&naam]));
    logboek::schrijf(&format!("Document openen: {naam}"));
    let hwnd = s.hwnd as isize;
    std::thread::spawn(move || {
        crate::beeld::com_start();
        let voortgang = move |tekst: String| {
            let doos = Box::into_raw(Box::new(tekst));
            unsafe {
                if PostMessageW(hwnd as HWND, WM_VOORTGANG, 0, doos as isize) == 0 {
                    drop(Box::from_raw(doos));
                }
            }
        };
        let uitkomst = crate::lezen::open(&pad, &voortgang).map(|d| (d, pad));
        let doos = Box::into_raw(Box::new(uitkomst));
        unsafe {
            if PostMessageW(hwnd as HWND, WM_GELADEN, 0, doos as isize) == 0 {
                drop(Box::from_raw(doos));
            }
        }
    });
}

fn geladen(uitkomst: Result<(Document, PathBuf), String>) {
    met(|s| s.bezig = false);
    let Some(s) = lees() else { return };
    match uitkomst {
        Ok((doc, pad)) => {
            let mut delen = Vec::new();
            if !doc.paginas.is_empty() {
                delen.push(tf(if doc.paginas.len() == 1 { "LezenEenPagina" } else { "LezenPaginas" }, &[&doc.paginas.len().to_string()]));
            }
            delen.push(tf(if doc.koppen.len() == 1 { "LezenEenKop" } else { "LezenKoppen" }, &[&doc.koppen.len().to_string()]));
            if doc.invulvelden > 0 {
                delen.push(tf("LezenInvulvelden", &[&doc.invulvelden.to_string()]));
            }
            zet_tekst(s.kop, &doc.naam);
            zet_tekst(s.status, &delen.join("  ·  "));
            zet_tekst(s.vak, &doc.tekst);
            let titel = format!("{} - Paper Pilot", doc.naam);
            zet_tekst(s.hwnd, &titel);
            logboek::schrijf(&format!("Document geopend: {} ({} pagina's, {} koppen, {} invulvelden).", doc.naam,
                doc.paginas.len(), doc.koppen.len(), doc.invulvelden));
            let zonder = doc.zonder_tekst.clone();
            let meldingen = doc.meldingen.clone();
            let velden = doc.invulvelden;
            met(|st| {
                st.doc = Some(doc);
                st.pad = Some(pad);
            });
            unsafe {
                SetFocus(s.vak);
                SendMessageW(s.vak, EM_SETSEL, 0, 0);
                SendMessageW(s.vak, EM_SCROLLCARET, 0, 0);
            }
            if !meldingen.is_empty() {
                dialogen::melding(s.hwnd, &t("StandKop"), &meldingen.join("\n"), dialogen::ICOON_INFO);
            }
            if !zonder.is_empty() {
                let lijst = zonder.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(", ");
                dialogen::melding(s.hwnd, &t("LezenScanKop"), &tf("LezenScanTekst", &[&lijst]), dialogen::ICOON_INFO);
            } else if velden > 0 {
                dialogen::melding(s.hwnd, &tf("LezenFormulierKop", &[&velden.to_string()]), &t("LezenFormulierTekst"), dialogen::ICOON_INFO);
            }
        }
        Err(e) => {
            zet_tekst(s.status, "");
            let oud = lees().and_then(|s| s.doc).map(|d| d.tekst).unwrap_or_else(|| t("LezenGeenDocument"));
            zet_tekst(s.vak, &oud);
            dialogen::melding(s.hwnd, &t("LezenMislukt"), &e, dialogen::ICOON_WAARSCHUWING);
            unsafe { SetFocus(s.vak) };
        }
    }
}

/// Een stuk tekst selecteren en in beeld brengen; de schermlezer leest de selectie voor.
fn selecteer(vak: HWND, begin: usize, eind: usize) {
    unsafe {
        SetFocus(vak);
        SendMessageW(vak, EM_SETSEL, begin, eind as isize);
        SendMessageW(vak, EM_SCROLLCARET, 0, 0);
    }
}

/// Kleine letters, met evenveel UTF-16-tekens als het origineel (zodat de plaatsen kloppen).
fn klein(s: &str) -> Vec<u16> {
    let mut uit = Vec::new();
    for c in s.chars() {
        let mut l = c.to_lowercase();
        let k = match (l.next(), l.next()) {
            (Some(k), None) if k.len_utf16() == c.len_utf16() => k,
            _ => c,
        };
        let mut b = [0u16; 2];
        uit.extend_from_slice(k.encode_utf16(&mut b));
    }
    uit
}

fn zoek(vooruit: bool, vragen: bool) {
    let Some(s) = lees() else { return };
    let Some(doc) = s.doc.clone() else { return };
    let mut woord = s.zoekwoord.clone();
    if vragen || woord.is_empty() {
        match keuzevensters::vraag_tekst(s.hwnd, &t("ZoekenTitel"), &t("ZoekenLabel"), &woord) {
            Some(w) if !w.trim().is_empty() => woord = w.trim().to_string(),
            _ => {
                unsafe { SetFocus(s.vak) };
                return;
            }
        }
        met(|st| st.zoekwoord = woord.clone());
    }
    let tekst = klein(&doc.tekst);
    let naald = klein(&woord);
    let (mut begin, mut eind) = (0u32, 0u32);
    unsafe { SendMessageW(s.vak, EM_GETSEL, &mut begin as *mut u32 as WPARAM, &mut eind as *mut u32 as LPARAM) };
    let n = naald.len();
    let past = |i: usize| i + n <= tekst.len() && tekst[i..i + n] == naald[..];
    let gevonden = if vooruit {
        let start = if vragen { begin as usize } else { eind as usize };
        (start..tekst.len()).find(|&i| past(i)).or_else(|| (0..start.min(tekst.len())).find(|&i| past(i)))
    } else {
        let start = (begin as usize).min(tekst.len());
        (0..start).rev().find(|&i| past(i)).or_else(|| (start..tekst.len()).rev().find(|&i| past(i)))
    };
    match gevonden {
        Some(i) => selecteer(s.vak, i, i + n),
        None => {
            dialogen::melding(s.hwnd, &t("ZoekenNiet"), &tf("ZoekenNietTekst", &[&woord]), dialogen::ICOON_INFO);
            unsafe { SetFocus(s.vak) };
        }
    }
}

fn naar_kop() {
    let Some(s) = lees() else { return };
    let Some(doc) = s.doc.clone() else { return };
    if doc.koppen.is_empty() {
        dialogen::melding(s.hwnd, &t("GeenKoppen"), &t("GeenKoppenTekst"), dialogen::ICOON_INFO);
        unsafe { SetFocus(s.vak) };
        return;
    }
    let lijst: Vec<String> = doc.koppen.iter().map(|(k, _, _)| k.clone()).collect();
    // Begin bij de kop waar je nu bent.
    let mut begin = 0u32;
    unsafe { SendMessageW(s.vak, EM_GETSEL, &mut begin as *mut u32 as WPARAM, 0) };
    let huidig = doc.koppen.iter().rposition(|(_, b, _)| *b <= begin as usize).unwrap_or(0);
    match keuzevensters::kies_uit_lijst(s.hwnd, &t("KoppenTitel"), &t("KoppenLabel"), &lijst, huidig) {
        Some(i) => selecteer(s.vak, doc.koppen[i].1, doc.koppen[i].2),
        None => unsafe {
            SetFocus(s.vak);
        },
    }
}

fn naar_pagina() {
    let Some(s) = lees() else { return };
    let Some(doc) = s.doc.clone() else { return };
    if doc.paginas.len() < 2 {
        dialogen::melding(s.hwnd, &t("GeenPaginas"), &t("GeenPaginasTekst"), dialogen::ICOON_INFO);
        unsafe { SetFocus(s.vak) };
        return;
    }
    let totaal = doc.paginas.len().to_string();
    let lijst: Vec<String> = (1..=doc.paginas.len()).map(|i| tf("PaginaVan", &[&i.to_string(), &totaal])).collect();
    let mut begin = 0u32;
    unsafe { SendMessageW(s.vak, EM_GETSEL, &mut begin as *mut u32 as WPARAM, 0) };
    let huidig = doc.paginas.iter().rposition(|b| *b <= begin as usize).unwrap_or(0);
    match keuzevensters::kies_uit_lijst(s.hwnd, &t("PaginaTitel"), &t("PaginaLabel"), &lijst, huidig) {
        Some(i) => {
            let b = doc.paginas[i];
            let e = b + lijst[i].encode_utf16().count();
            selecteer(s.vak, b, e);
        }
        None => unsafe {
            SetFocus(s.vak);
        },
    }
}

fn opslaan() {
    let Some(s) = lees() else { return };
    let Some(doc) = s.doc.clone() else { return };
    let naam = Path::new(&doc.naam).with_extension("txt").to_string_lossy().to_string();
    if let Some(pad) = opslaan_venster(s.hwnd, &naam) {
        match std::fs::write(&pad, format!("\u{feff}{}", doc.tekst)) {
            Ok(_) => dialogen::melding(s.hwnd, &t("Opgeslagen"), &tf("OpgeslagenTekst", &[&pad.to_string_lossy()]), dialogen::ICOON_INFO),
            Err(e) => dialogen::melding(s.hwnd, &t("OpslaanMislukt"), &e.to_string(), dialogen::ICOON_WAARSCHUWING),
        }
    }
    unsafe { SetFocus(s.vak) };
}

/// De gescande pagina's of foto, rechtgezet, als afbeelding opslaan (PNG, JPG, TIFF met alle pagina's, BMP).
fn opslaan_beeld() {
    let Some(s) = lees() else { return };
    let Some(doc) = s.doc.clone() else { return };
    if doc.beelden.is_empty() {
        dialogen::melding(s.hwnd, &t("GeenBeeld"), &t("GeenBeeldTekst"), dialogen::ICOON_INFO);
        unsafe { SetFocus(s.vak) };
        return;
    }
    let ext = "pdf";
    let naam = Path::new(&doc.naam).with_extension(ext).to_string_lossy().to_string();
    let filter = format!("PDF ({})\0*.pdf\0TIFF ({})\0*.tif\0PNG\0*.png\0JPG\0*.jpg\0BMP\0*.bmp\0GIF\0*.gif\0\0",
        t("FilterPdfMetTekst"), t("FilterAllePaginas"));
    if let Some(pad) = opslaan_als(s.hwnd, &naam, &filter, ext) {
        let is_pdf = pad.extension().map(|e| e.eq_ignore_ascii_case("pdf")).unwrap_or(false);
        let uitkomst = if is_pdf {
            crate::pdf_maken::schrijf(&pad, &doc.beelden, &doc.beeld_regels)
        } else {
            crate::beeld::schrijf(&pad, &doc.beelden)
        };
        match uitkomst {
            Ok(_) => dialogen::melding(s.hwnd, &t("Opgeslagen"), &tf("BeeldOpgeslagenTekst", &[&pad.to_string_lossy()]), dialogen::ICOON_INFO),
            Err(e) => dialogen::melding(s.hwnd, &t("OpslaanMislukt"), &e, dialogen::ICOON_WAARSCHUWING),
        }
    }
    unsafe { SetFocus(s.vak) };
}

/// Welke scan of foto hoort bij de plek van de cursor? (Bij meer pagina's met een scan: die van deze pagina.)
fn huidig_beeld(s: &Staat, doc: &Document) -> Option<crate::beeld::Beeld> {
    if doc.beelden.is_empty() {
        return None;
    }
    if doc.beelden.len() == 1 || doc.paginas.is_empty() {
        return doc.beelden.first().cloned();
    }
    let mut begin = 0u32;
    unsafe { SendMessageW(s.vak, EM_GETSEL, &mut begin as *mut u32 as WPARAM, 0) };
    let pagina = doc.paginas.iter().rposition(|b| *b <= begin as usize).map(|i| i + 1).unwrap_or(1);
    let i = doc.beeld_paginas.iter().position(|p| *p == pagina).unwrap_or(0);
    doc.beelden.get(i).cloned()
}

/// Handschrift lezen of beschrijven wat er op de afbeelding staat, met het taalmodel op deze computer.
fn kijk(soort: crate::beeldmodel::Soort) {
    use crate::beeldmodel::Soort;
    let Some(s) = lees() else { return };
    let Some(doc) = s.doc.clone() else { return };
    let Some(beeld) = huidig_beeld(&s, &doc) else {
        dialogen::melding(s.hwnd, &t("GeenBeeld"), &t("KijkAlleenBeeld"), dialogen::ICOON_INFO);
        unsafe { SetFocus(s.vak) };
        return;
    };
    if !crate::beeldmodel::geinstalleerd() && !model_downloaden(s.hwnd) {
        unsafe { SetFocus(s.vak) };
        return;
    }
    let (titel, kop) = match soort {
        Soort::Handschrift => (t("HandschriftTitel"), t("HandschriftBezig")),
        Soort::Beschrijven => (t("BeschrijvenTitel"), t("BeschrijvenBezig")),
    };
    let uitkomst = dialogen::werk(s.hwnd, &titel, &kop, move |v| {
        v.meld(t("KijkDuurt"), 0);
        crate::beeldmodel::vraag(&beeld, soort)
    });
    match uitkomst {
        Some(Ok(tekst)) => {
            logboek::schrijf(&format!("Taalmodel: {} tekens antwoord.", tekst.chars().count()));
            crate::tekstvenster::toon(s.hwnd, &format!("{titel} - {}", doc.naam), &tekst);
        }
        Some(Err(e)) => {
            dialogen::melding(s.hwnd, &t("ModelMislukt"), &e, dialogen::ICOON_WAARSCHUWING);
            unsafe { SetFocus(s.vak) };
        }
        None => unsafe {
            SetFocus(s.vak);
        },
    }
}

/// Vraagt of het taalmodel gedownload mag worden, en doet dat. Geeft true als het er daarna is.
pub fn model_downloaden(eigenaar: HWND) -> bool {
    if !dialogen::ja_nee(eigenaar, &t("ModelNodig"), &t("ModelNodigTekst"), true) {
        return false;
    }
    let uitkomst = dialogen::werk(eigenaar, &t("ModelDownloadTitel"), &t("ModelDownloadKop"), |v| crate::beeldmodel::installeer(v));
    match uitkomst {
        Some(Ok(())) => {
            dialogen::melding(eigenaar, &t("ModelKlaar"), &t("ModelKlaarTekst"), dialogen::ICOON_INFO);
            true
        }
        Some(Err(e)) => {
            logboek::schrijf(&format!("Taalmodel downloaden: {e}"));
            dialogen::melding(eigenaar, &t("ModelDownloadMislukt"), &crate::teksten::tf("ModelDownloadMisluktTekst", &[&e]), dialogen::ICOON_WAARSCHUWING);
            false
        }
        None => {
            dialogen::melding(eigenaar, &t("ModelDownloadGestopt"), &t("ModelDownloadGestoptTekst"), dialogen::ICOON_INFO);
            false
        }
    }
}

fn opdracht(hwnd: HWND, id: i32) {
    let doc_open = lees().map(|s| s.doc.is_some()).unwrap_or(false);
    match id {
        ID_OPENEN => kies_bestand(),
        ID_SCAN => scannen(),
        ID_SLUITEN => unsafe {
            DestroyWindow(hwnd);
        },
        _ if !doc_open => {
            dialogen::melding(hwnd, &t("LezenGeenDocument"), &t("LezenEerstOpenen"), dialogen::ICOON_INFO);
        }
        ID_ZOEKEN => zoek(true, true),
        ID_VOLGENDE => zoek(true, false),
        ID_VORIGE => zoek(false, false),
        ID_KOPPEN => naar_kop(),
        ID_PAGINA => naar_pagina(),
        ID_OPSLAAN => opslaan(),
        ID_BEELD => opslaan_beeld(),
        ID_HANDSCHRIFT => kijk(crate::beeldmodel::Soort::Handschrift),
        ID_AFDRUK => afdrukken(),
        ID_BESCHRIJF => kijk(crate::beeldmodel::Soort::Beschrijven),
        _ => {}
    }
}

unsafe extern "system" fn venster_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_SIZE => {
            schik();
            0
        }
        WM_COMMAND => {
            let id = (wp & 0xFFFF) as i32;
            if id != ID_TEKST {
                opdracht(hwnd, id);
            }
            0
        }
        WM_VOORTGANG => {
            let tekst = Box::from_raw(lp as *mut String);
            if let Some(s) = lees() {
                zet_tekst(s.status, &tekst);
            }
            0
        }
        WM_GELADEN => {
            let uitkomst = Box::from_raw(lp as *mut Result<(Document, PathBuf), String>);
            geladen(*uitkomst);
            0
        }
        WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT => {
            let k = crate::hoofdvenster::kleuren();
            let Some(s) = lees() else { return DefWindowProcW(hwnd, msg, wp, lp) };
            SetTextColor(wp as HDC, if lp as HWND == s.status { k.zacht } else { k.tekst });
            if lp as HWND == s.vak {
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
                        SetFocus(s.vak);
                    }
                }
            }
            0
        }
        WM_CLOSE => {
            DestroyWindow(hwnd);
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
            let hoofd = crate::hoofdvenster::hoofd_hwnd();
            if !hoofd.is_null() {
                SetForegroundWindow(hoofd);
            }
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}
