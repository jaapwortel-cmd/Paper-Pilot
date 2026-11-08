// Paper Pilot - een brief terugschrijven: bezwaar maken, iets opzeggen, een vraag stellen, uitstel van betaling vragen.
//
// Met het taalmodel (op de eigen computer) schrijft Paper Pilot een voorstel op basis van de brief die je leest en wat
// jij erover zegt. Zonder taalmodel komt er een nette voorbeeldbrief met je eigen gegevens (Mijn gegevens), de datum,
// de afzender en het kenmerk. De brief komt in een gewoon tekstvak om aan te passen; opslaan als Word of tekst,
// afdrukken, kopiëren of mailen.

use crate::teksten::{t, tf};
use crate::win::*;
use crate::{dialogen, keuzevensters};
use std::cell::RefCell;
use std::path::Path;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::UI::Controls::*;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

/// De soorten brieven (sleutels in de teksten: Brief_<soort>, BriefTekst_<soort>).
pub const SOORTEN: [&str; 6] = ["bezwaar", "opzeggen", "vraag", "uitstel", "fout", "vrij"];

/// Een kenmerk of klantnummer uit de brief ("Ons kenmerk: 2026/45678").
fn kenmerk(tekst: &str) -> Option<String> {
    for regel in tekst.lines() {
        let l = regel.trim().to_lowercase();
        for label in ["ons kenmerk", "uw kenmerk", "kenmerk", "dossiernummer", "klantnummer", "relatienummer", "polisnummer", "referentie", "our reference", "reference"] {
            if let Some(rest) = l.strip_prefix(label) {
                let rest = rest.trim_start_matches([':', ' ', '|', '\t']);
                if rest.chars().any(|c| c.is_ascii_digit()) {
                    let n = regel.trim().len() - rest.len();
                    return regel.trim().get(n..).map(|x| x.split("  ").next().unwrap_or(x).trim().to_string());
                }
            }
        }
    }
    None
}

/// Een voorbeeldbrief zonder taalmodel.
pub fn sjabloon(soort: &str, wens: &str, afzender: &str, doc_naam: &str, brieftekst: &str) -> String {
    let g = crate::gegevens::laad();
    let w = |s: &str| crate::gegevens::waarde(&g, s).unwrap_or_default();
    let naam = w("naam");
    let mut r: Vec<String> = Vec::new();
    for regel in [naam.clone(), w("straat"), w("postcode_plaats"), w("telefoon"), w("email")] {
        if !regel.trim().is_empty() {
            r.push(regel);
        }
    }
    if r.is_empty() {
        r.push(t("BriefJeNaam"));
        r.push(t("BriefJeAdres"));
    }
    r.push(String::new());
    r.push(if afzender.is_empty() { t("BriefAanWie") } else { afzender.to_string() });
    r.push(String::new());
    let plaats = w("woonplaats");
    let datum = crate::herinneringen::mooi(crate::herinneringen::vandaag());
    r.push(if plaats.is_empty() { datum } else { format!("{plaats}, {datum}") });
    r.push(String::new());
    r.push(tf("BriefBetreft", &[&t(&format!("Brief_{soort}")), doc_naam]));
    if let Some(k) = kenmerk(brieftekst) {
        r.push(tf("BriefUwKenmerk", &[&k]));
    }
    r.push(String::new());
    r.push(t("BriefAanhef"));
    r.push(String::new());
    r.push(t(&format!("BriefTekst_{soort}")));
    if !wens.trim().is_empty() {
        r.push(String::new());
        r.push(wens.trim().to_string());
    }
    r.push(String::new());
    r.push(t("BriefAfsluiting"));
    r.push(String::new());
    r.push(t("BriefGroet"));
    r.push(String::new());
    r.push(String::new());
    r.push(if naam.is_empty() { t("BriefJeNaam") } else { naam });
    r.join("\r\n")
}

/// Ctrl+B in het leesvenster: kies wat voor brief, zeg wat erin moet, en Paper Pilot maakt een voorstel.
pub fn start(eigenaar: HWND, doc: &crate::lezen::Document) {
    let namen: Vec<String> = SOORTEN.iter().map(|s| t(&format!("Brief_{s}"))).collect();
    let Some(k) = keuzevensters::kies_uit_lijst(eigenaar, &t("BriefTitel"), &t("BriefKiesSoort"), &namen, 0) else { return };
    let soort = SOORTEN[k];
    let Some(wens) = keuzevensters::vraag_tekst(eigenaar, &t("BriefTitel"), &t("BriefWens"), "") else { return };
    let afzender = crate::post::afzender(doc);
    let voorbeeld = sjabloon(soort, &wens, &afzender, &doc.naam, &doc.tekst);
    let mut brief = voorbeeld.clone();
    if crate::beeldmodel::geinstalleerd() {
        let opdracht = tf("BriefOpdracht", &[&t(&format!("Brief_{soort}")), &wens, &voorbeeld]);
        let tekst = doc.tekst.chars().take(6000).collect::<String>();
        match dialogen::werk(eigenaar, &t("BriefTitel"), &t("BriefBezig"), move |v| {
            v.meld(t("KijkDuurt"), 0);
            crate::beeldmodel::vraag_vrij(&opdracht, &tekst, 1200)
        }) {
            Some(Ok(a)) if a.trim().len() > 80 => brief = a.replace("**", "").replace("\r\n", "\n").replace('\n', "\r\n"),
            Some(Err(e)) => crate::logboek::schrijf(&format!("Brief schrijven met het taalmodel: {e}")),
            _ => {}
        }
    }
    let stam = Path::new(&doc.naam).file_stem().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
    open(eigenaar, &tf("BriefNaam", &[&t(&format!("Brief_{soort}")), &stam]), &brief);
}

// ------------------------------------------------------------------ Word-bestand (.docx)

fn xml(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Een eenvoudig, toegankelijk Word-bestand: elke regel een alinea, met titel en taal.
pub fn schrijf_docx(pad: &Path, titel: &str, tekst: &str) -> Result<(), String> {
    use std::io::Write;
    let taal = if crate::teksten::taal() == "en" { "en-GB" } else { "nl-NL" };
    let alineas: String = tekst.replace("\r\n", "\n").split('\n')
        .map(|r| if r.is_empty() { "<w:p/>".to_string() } else { format!("<w:p><w:r><w:t xml:space=\"preserve\">{}</w:t></w:r></w:p>", xml(r)) })
        .collect();
    let f = std::fs::File::create(pad).map_err(|e| e.to_string())?;
    let mut z = zip::ZipWriter::new(f);
    let o = zip::write::SimpleFileOptions::default();
    let delen: [(&str, String); 5] = [
        ("[Content_Types].xml", r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/><Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/></Types>"#.to_string()),
        ("_rels/.rels", r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/></Relationships>"#.to_string()),
        ("word/_rels/document.xml.rels", r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#.to_string()),
        ("word/styles.xml", format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="Calibri" w:hAnsi="Calibri" w:cs="Calibri"/><w:sz w:val="24"/><w:lang w:val="{taal}"/></w:rPr></w:rPrDefault><w:pPrDefault><w:pPr><w:spacing w:after="0" w:line="276" w:lineRule="auto"/></w:pPr></w:pPrDefault></w:docDefaults></w:styles>"#)),
        ("docProps/core.xml", format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>{}</dc:title><dc:language>{taal}</dc:language><dc:creator>Paper Pilot</dc:creator></cp:coreProperties>"#, xml(titel))),
    ];
    for (naam, inhoud) in delen.iter() {
        z.start_file(*naam, o).map_err(|e| e.to_string())?;
        z.write_all(inhoud.as_bytes()).map_err(|e| e.to_string())?;
    }
    z.start_file("word/document.xml", o).map_err(|e| e.to_string())?;
    z.write_all(format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>{alineas}<w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1418" w:right="1418" w:bottom="1418" w:left="1418" w:header="709" w:footer="709" w:gutter="0"/></w:sectPr></w:body></w:document>"#).as_bytes()).map_err(|e| e.to_string())?;
    z.finish().map_err(|e| e.to_string())?;
    Ok(())
}

// ------------------------------------------------------------------ het venster

const KLASSE: &str = "PaperPilotSchrijven";
const ID_VAK: i32 = 10;
const ID_OPSLAAN: i32 = 101;
const ID_AFDRUK: i32 = 102;
const ID_KOPIEER: i32 = 103;
const ID_MAIL: i32 = 104;
const ID_SLUITEN: i32 = IDCANCEL;
const KNOPPEN: [(i32, &str); 5] = [(ID_OPSLAAN, "KnopBriefOpslaan"), (ID_AFDRUK, "KnopAfdrukken"), (ID_KOPIEER, "KnopBriefKopieren"),
    (ID_MAIL, "KnopMailen"), (ID_SLUITEN, "KnopSluitenEsc")];

#[derive(Clone)]
struct Staat {
    hwnd: HWND,
    eigenaar: HWND,
    kop: HWND,
    vak: HWND,
    uitleg: HWND,
    knoppen: Vec<HWND>,
    letter: HFONT,
    kopletter: HFONT,
    vakletter: HFONT,
    kwast: HBRUSH,
    toetsen: HACCEL,
    naam: String,
    opgeslagen: bool,
}

thread_local! {
    static STAAT: RefCell<Option<Staat>> = const { RefCell::new(None) };
}

fn lees() -> Option<Staat> {
    STAAT.with(|s| s.borrow().clone())
}

pub fn hwnd() -> HWND {
    lees().map(|s| s.hwnd).unwrap_or(std::ptr::null_mut())
}

pub fn toetsen() -> HACCEL {
    lees().map(|s| s.toetsen).unwrap_or(std::ptr::null_mut())
}

fn registreer() {
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

/// Opent het schrijfvenster met de brief.
pub fn open(eigenaar: HWND, naam: &str, brief: &str) {
    if let Some(s) = lees() {
        unsafe { SetForegroundWindow(s.hwnd) };
        return;
    }
    registreer();
    let klasse = w(KLASSE);
    let titel = w(&format!("{naam} - Paper Pilot"));
    let hwnd = unsafe {
        CreateWindowExW(0, klasse.as_ptr(), titel.as_ptr(), WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN, CW_USEDEFAULT, CW_USEDEFAULT, 900, 720,
            std::ptr::null_mut(), std::ptr::null_mut(), instantie(), std::ptr::null())
    };
    if hwnd.is_null() {
        return;
    }
    let d = dpi(hwnd);
    let kleuren = crate::hoofdvenster::kleuren();
    crate::thema::titelbalk(hwnd, kleuren.donker);
    let letter = lettertype(12, false, d, "Segoe UI");
    let kopletter = lettertype(16, true, d, "Segoe UI");
    let vakletter = lettertype(tekstgrootte(), false, d, "Segoe UI");
    let maak = |klasse: &str, tekst: &str, stijl: u32, ex: u32, id: i32| unsafe {
        let k = w(klasse);
        let tt = w(tekst);
        let c = CreateWindowExW(ex, k.as_ptr(), tt.as_ptr(), WS_CHILD | WS_VISIBLE | stijl, 0, 0, 10, 10, hwnd, id as isize as _, instantie(), std::ptr::null());
        zet_lettertype(c, letter);
        c
    };
    // De kop is ook de naam van het tekstvak voor de schermlezer.
    let kop = maak("STATIC", naam, 0x80, 0, -1);
    zet_lettertype(kop, kopletter);
    let vak = maak("EDIT", "", WS_TABSTOP | WS_VSCROLL | (ES_MULTILINE | ES_AUTOVSCROLL | ES_WANTRETURN | ES_NOHIDESEL) as u32, WS_EX_CLIENTEDGE, ID_VAK);
    zet_lettertype(vak, vakletter);
    let uitleg = maak("STATIC", &t("BriefUitleg"), 0x80, 0, -1);
    let knoppen: Vec<HWND> = KNOPPEN.iter().enumerate()
        .map(|(i, (id, k))| maak("BUTTON", &t(k), WS_TABSTOP | if i == 0 { WS_GROUP } else { 0 } | BS_PUSHBUTTON as u32, 0, *id)).collect();
    zet_tekst(vak, &regels(brief));
    unsafe {
        SendMessageW(vak, EM_SETLIMITTEXT, 0, 0);
        windows_sys::Win32::UI::Shell::SetWindowSubclass(vak, Some(vak_proc), 1, 0);
    }
    let toetsen = unsafe {
        let ctrl = (FVIRTKEY | FCONTROL) as u8;
        let a = [
            ACCEL { fVirt: ctrl, key: b'S' as u16, cmd: ID_OPSLAAN as u16 },
            ACCEL { fVirt: ctrl, key: b'P' as u16, cmd: ID_AFDRUK as u16 },
            ACCEL { fVirt: (FVIRTKEY | FCONTROL | FSHIFT) as u8, key: b'C' as u16, cmd: ID_KOPIEER as u16 },
            ACCEL { fVirt: ctrl, key: b'M' as u16, cmd: ID_MAIL as u16 },
        ];
        CreateAcceleratorTableW(a.as_ptr(), a.len() as i32)
    };
    let kwast = unsafe { CreateSolidBrush(kleuren.vak) };
    STAAT.with(|s| *s.borrow_mut() = Some(Staat { hwnd, eigenaar, kop, vak, uitleg, knoppen, letter, kopletter, vakletter, kwast, toetsen,
        naam: naam.to_string(), opgeslagen: false }));
    schik();
    unsafe {
        ShowWindow(hwnd, SW_SHOWMAXIMIZED);
        SetFocus(vak);
        SendMessageW(vak, EM_SETSEL, 0, 0);
        SendMessageW(vak, EM_SCROLLCARET, 0, 0);
    }
}

fn schik() {
    let Some(s) = lees() else { return };
    let mut rc: RECT = unsafe { std::mem::zeroed() };
    unsafe { GetClientRect(s.hwnd, &mut rc) };
    let d = dpi(s.hwnd);
    let m = px(16, d);
    let kh = px(40, d);
    unsafe {
        MoveWindow(s.kop, m, m, rc.right - 2 * m, px(34, d), 1);
        MoveWindow(s.uitleg, m, m + px(36, d), rc.right - 2 * m, px(24, d), 1);
    }
    let mut x = m;
    let y = m + px(66, d);
    let dc = unsafe { GetDC(s.hwnd) };
    let oud = unsafe { SelectObject(dc, s.letter as _) };
    for k in &s.knoppen {
        let tekst: Vec<u16> = lees_tekst(*k).encode_utf16().collect();
        let mut maat = SIZE { cx: 0, cy: 0 };
        unsafe { GetTextExtentPoint32W(dc, tekst.as_ptr(), tekst.len() as i32, &mut maat) };
        let b = maat.cx + px(32, d);
        unsafe { MoveWindow(*k, x, y, b, kh, 1) };
        x += b + px(8, d);
    }
    unsafe {
        SelectObject(dc, oud);
        ReleaseDC(s.hwnd, dc);
        let top = y + kh + px(12, d);
        MoveWindow(s.vak, m, top, rc.right - 2 * m, (rc.bottom - m - top).max(px(60, d)), 1);
    }
}

fn opslaan(s: &Staat) -> Option<std::path::PathBuf> {
    let filter = format!("{}\0*.docx\0{}\0*.txt\0\0", t("FilterWord"), t("FilterTekst"));
    let pad = crate::leesvenster::opslaan_als(s.hwnd, &format!("{}.docx", s.naam.replace(['\\', '/', ':', '*', '?', '"', '<', '>', '|'], "-")), &filter, "docx")?;
    let tekst = lees_tekst(s.vak);
    let uitkomst = if pad.extension().map(|e| e.eq_ignore_ascii_case("txt")).unwrap_or(false) {
        std::fs::write(&pad, format!("\u{feff}{}", regels(&tekst))).map_err(|e| e.to_string())
    } else {
        schrijf_docx(&pad, &s.naam, &tekst)
    };
    match uitkomst {
        Ok(_) => {
            STAAT.with(|st| if let Some(st) = st.borrow_mut().as_mut() { st.opgeslagen = true });
            unsafe { SendMessageW(s.vak, EM_SETMODIFY, 0, 0) };
            dialogen::melding(s.hwnd, &t("BriefOpgeslagen"), &tf("BriefOpgeslagenTekst", &[&pad.to_string_lossy()]), dialogen::ICOON_INFO);
            Some(pad)
        }
        Err(e) => {
            dialogen::melding(s.hwnd, &t("OpslaanMislukt"), &e, dialogen::ICOON_WAARSCHUWING);
            None
        }
    }
}

fn opdracht(id: i32) {
    let Some(s) = lees() else { return };
    match id {
        ID_OPSLAAN => {
            opslaan(&s);
        }
        ID_AFDRUK => {
            match crate::afdrukken::tekst(s.hwnd, &s.naam, &lees_tekst(s.vak)) {
                Some(Ok(n)) => dialogen::melding(s.hwnd, &t("Afgedrukt"), &tf("AfgedruktTekst", &[&n.to_string()]), dialogen::ICOON_INFO),
                Some(Err(e)) => dialogen::melding(s.hwnd, &t("AfdrukkenMislukt"), &e, dialogen::ICOON_WAARSCHUWING),
                None => {}
            }
        }
        ID_KOPIEER => {
            if kopieer(s.hwnd, &lees_tekst(s.vak)) {
                dialogen::melding(s.hwnd, &t("BriefTitel"), &t("BriefGekopieerd"), dialogen::ICOON_INFO);
            }
        }
        ID_MAIL => {
            let map = std::env::temp_dir().join("Paper Pilot");
            let _ = std::fs::create_dir_all(&map);
            let pad = map.join(format!("{}.docx", s.naam.replace(['\\', '/', ':', '*', '?', '"', '<', '>', '|'], "-")));
            match schrijf_docx(&pad, &s.naam, &lees_tekst(s.vak)) {
                Ok(_) => crate::versturen::mail_met_uitleg(s.hwnd, &pad, &s.naam),
                Err(e) => dialogen::melding(s.hwnd, &t("MailMislukt"), &e, dialogen::ICOON_WAARSCHUWING),
            }
        }
        ID_SLUITEN => {
            let gewijzigd = unsafe { SendMessageW(s.vak, EM_GETMODIFY, 0, 0) } != 0 || !s.opgeslagen;
            if gewijzigd {
                let titel = t("BriefTitel");
                let kop = t("BriefNietOpgeslagen");
                let tekst = t("BriefNietOpgeslagenTekst");
                let mut v = dialogen::Vraag::nieuw(&titel, &kop, &tekst);
                v.knoppen.push(dialogen::Knop { id: 6001, tekst: t("BriefNuOpslaan") });
                v.knoppen.push(dialogen::Knop { id: 6002, tekst: t("BriefNietOpslaan") });
                v.gewone_knoppen = windows_sys::Win32::UI::Controls::TDCBF_CANCEL_BUTTON;
                v.standaard = 6001;
                match v.toon(s.hwnd) {
                    6001 => {
                        if opslaan(&s).is_none() {
                            return;
                        }
                    }
                    6002 => {}
                    _ => return,
                }
            }
            unsafe { DestroyWindow(s.hwnd) };
            return;
        }
        _ => return,
    }
    unsafe { SetFocus(s.vak) };
}

/// Het tekstvak: Enter maakt een nieuwe regel, maar Tab gaat naar de knoppen en Escape sluit het venster.
unsafe extern "system" fn vak_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM, _id: usize, _data: usize) -> LRESULT {
    use windows_sys::Win32::UI::Shell::DefSubclassProc;
    if msg == WM_GETDLGCODE {
        let r = DefSubclassProc(hwnd, msg, wp, lp);
        let bericht = lp as *const MSG;
        if !bericht.is_null() && (*bericht).message == WM_KEYDOWN && ((*bericht).wParam as u16 == VK_TAB || (*bericht).wParam as u16 == VK_ESCAPE) {
            return r & !((DLGC_WANTALLKEYS | DLGC_WANTTAB | DLGC_WANTMESSAGE) as isize);
        }
        return r & !(DLGC_HASSETSEL as isize);
    }
    DefSubclassProc(hwnd, msg, wp, lp)
}

unsafe extern "system" fn venster_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_SIZE => {
            schik();
            0
        }
        WM_COMMAND => {
            let id = (wp & 0xFFFF) as i32;
            if id != ID_VAK {
                opdracht(id);
            }
            0
        }
        WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT => {
            let k = crate::hoofdvenster::kleuren();
            let Some(s) = lees() else { return DefWindowProcW(hwnd, msg, wp, lp) };
            SetTextColor(wp as HDC, k.tekst);
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
            opdracht(ID_SLUITEN);
            0
        }
        WM_DESTROY => {
            if let Some(s) = lees() {
                weg(s.letter as _);
                weg(s.kopletter as _);
                weg(s.vakletter as _);
                weg(s.kwast as _);
                DestroyAcceleratorTable(s.toetsen);
                if !s.eigenaar.is_null() {
                    SetForegroundWindow(s.eigenaar);
                }
            }
            STAAT.with(|s| *s.borrow_mut() = None);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}
