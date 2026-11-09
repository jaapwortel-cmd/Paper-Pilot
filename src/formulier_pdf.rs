// Paper Pilot - invulvelden van een PDF-formulier lezen en invullen.
//
// Dit gebruikt de "formulier-omgeving" van PDFium zelf (zoals een PDF-lezer die je laat typen): Paper Pilot zet
// de focus op een veld, selecteert alles en vervangt het door de nieuwe tekst. Zo tekent PDFium de tekst ook echt
// in het veld, en ziet elke PDF-lezer het ingevulde formulier. Vinkjes en keuzerondjes worden "aangeklikt".
// Rechtstreeks via pdfium.dll (dezelfde als voor het lezen), omdat pdfium-render deze functies niet aanbiedt.

use std::ffi::c_void;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

pub(crate) type Doc = *mut c_void;
type Form = *mut c_void;
pub(crate) type Page = *mut c_void;
type Annot = *mut c_void;

#[repr(C)]
struct FormFillInfo {
    version: i32,
    velden: [*mut c_void; 16],
    reserve: [*mut c_void; 48],
}

#[repr(C)]
struct Rect {
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
}

#[repr(C)]
pub(crate) struct FileWrite {
    pub version: i32,
    pub write_block: unsafe extern "C" fn(*mut FileWrite, *const c_void, u32) -> i32,
    pub data: Vec<u8>,
}

pub(crate) unsafe extern "C" fn schrijf_blok(zelf: *mut FileWrite, data: *const c_void, n: u32) -> i32 {
    (*zelf).data.extend_from_slice(std::slice::from_raw_parts(data as *const u8, n as usize));
    1
}

#[allow(non_snake_case)]
pub(crate) struct Pdfium {
    pub FPDF_LoadMemDocument: unsafe extern "C" fn(*const c_void, i32, *const i8) -> Doc,
    pub FPDF_CloseDocument: unsafe extern "C" fn(Doc),
    pub FPDF_GetPageCount: unsafe extern "C" fn(Doc) -> i32,
    pub FPDF_LoadPage: unsafe extern "C" fn(Doc, i32) -> Page,
    pub FPDF_ClosePage: unsafe extern "C" fn(Page),
    FPDFDOC_InitFormFillEnvironment: unsafe extern "C" fn(Doc, *mut FormFillInfo) -> Form,
    FPDFDOC_ExitFormFillEnvironment: unsafe extern "C" fn(Form),
    FORM_OnAfterLoadPage: unsafe extern "C" fn(Page, Form),
    FORM_OnBeforeClosePage: unsafe extern "C" fn(Page, Form),
    FPDFPage_GetAnnotCount: unsafe extern "C" fn(Page) -> i32,
    pub FPDFPage_GetAnnot: unsafe extern "C" fn(Page, i32) -> Annot,
    pub FPDFPage_CloseAnnot: unsafe extern "C" fn(Annot),
    FPDFAnnot_GetSubtype: unsafe extern "C" fn(Annot) -> i32,
    FPDFAnnot_GetRect: unsafe extern "C" fn(Annot, *mut Rect) -> i32,
    FPDFAnnot_GetFormFieldType: unsafe extern "C" fn(Form, Annot) -> i32,
    FPDFAnnot_GetFormFieldFlags: unsafe extern "C" fn(Form, Annot) -> i32,
    FPDFAnnot_GetFormFieldName: unsafe extern "C" fn(Form, Annot, *mut u16, u32) -> u32,
    FPDFAnnot_GetFormFieldAlternateName: unsafe extern "C" fn(Form, Annot, *mut u16, u32) -> u32,
    FPDFAnnot_GetFormFieldValue: unsafe extern "C" fn(Form, Annot, *mut u16, u32) -> u32,
    FPDFAnnot_GetFormFieldExportValue: unsafe extern "C" fn(Form, Annot, *mut u16, u32) -> u32,
    FPDFAnnot_IsChecked: unsafe extern "C" fn(Form, Annot) -> i32,
    FPDFAnnot_GetOptionCount: unsafe extern "C" fn(Form, Annot) -> i32,
    FPDFAnnot_GetOptionLabel: unsafe extern "C" fn(Form, Annot, i32, *mut u16, u32) -> u32,
    FPDFAnnot_IsOptionSelected: unsafe extern "C" fn(Form, Annot, i32) -> i32,
    FORM_SetFocusedAnnot: unsafe extern "C" fn(Form, Annot) -> i32,
    FORM_SelectAllText: unsafe extern "C" fn(Form, Page) -> i32,
    FORM_ReplaceSelection: unsafe extern "C" fn(Form, Page, *const u16),
    FORM_SetIndexSelected: unsafe extern "C" fn(Form, Page, i32, i32) -> i32,
    FORM_ForceToKillFocus: unsafe extern "C" fn(Form) -> i32,
    FORM_OnLButtonDown: unsafe extern "C" fn(Form, Page, i32, f64, f64) -> i32,
    FORM_OnLButtonUp: unsafe extern "C" fn(Form, Page, i32, f64, f64) -> i32,
    pub FPDF_SaveAsCopy: unsafe extern "C" fn(Doc, *mut FileWrite, u32) -> i32,
    pub FPDFText_LoadPage: unsafe extern "C" fn(Page) -> *mut c_void,
    pub FPDFText_ClosePage: unsafe extern "C" fn(*mut c_void),
    FPDFText_GetBoundedText: unsafe extern "C" fn(*mut c_void, f64, f64, f64, f64, *mut u16, i32) -> i32,
    // Voor formulieren zonder invulvelden (plat.rs):
    pub FPDF_GetPageWidthF: unsafe extern "C" fn(Page) -> f32,
    pub FPDF_GetPageHeightF: unsafe extern "C" fn(Page) -> f32,
    pub FPDFText_CountChars: unsafe extern "C" fn(*mut c_void) -> i32,
    pub FPDFText_GetUnicode: unsafe extern "C" fn(*mut c_void, i32) -> u32,
    pub FPDFText_GetCharBox: unsafe extern "C" fn(*mut c_void, i32, *mut f64, *mut f64, *mut f64, *mut f64) -> i32,
    pub FPDFBitmap_Create: unsafe extern "C" fn(i32, i32, i32) -> *mut c_void,
    pub FPDFBitmap_FillRect: unsafe extern "C" fn(*mut c_void, i32, i32, i32, i32, u32) -> i32,
    pub FPDFBitmap_GetBuffer: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
    pub FPDFBitmap_GetStride: unsafe extern "C" fn(*mut c_void) -> i32,
    pub FPDFBitmap_Destroy: unsafe extern "C" fn(*mut c_void),
    pub FPDF_RenderPageBitmap: unsafe extern "C" fn(*mut c_void, Page, i32, i32, i32, i32, i32, i32),
    pub FPDFPageObj_NewTextObj: unsafe extern "C" fn(Doc, *const i8, f32) -> *mut c_void,
    pub FPDFText_SetText: unsafe extern "C" fn(*mut c_void, *const u16) -> i32,
    pub FPDFPageObj_Transform: unsafe extern "C" fn(*mut c_void, f64, f64, f64, f64, f64, f64),
    pub FPDFPage_InsertObject: unsafe extern "C" fn(Page, *mut c_void),
    pub FPDFPage_GenerateContent: unsafe extern "C" fn(Page) -> i32,
    // Voor de handtekening:
    pub FPDFPageObj_NewImageObj: unsafe extern "C" fn(Doc) -> *mut c_void,
    pub FPDFImageObj_SetBitmap: unsafe extern "C" fn(*mut Page, i32, *mut c_void, *mut c_void) -> i32,
    pub FPDFImageObj_SetMatrix: unsafe extern "C" fn(*mut c_void, f64, f64, f64, f64, f64, f64) -> i32,
    pub FPDFAnnot_GetFlags: unsafe extern "C" fn(Annot) -> i32,
    pub FPDFAnnot_SetFlags: unsafe extern "C" fn(Annot, i32) -> i32,
    // Voor Controleren (toegankelijkheid van een PDF):
    pub FPDF_GetMetaText: unsafe extern "C" fn(Doc, *const u8, *mut u16, u32) -> u32,
    pub FPDFCatalog_IsTagged: unsafe extern "C" fn(Doc) -> i32,
    pub FPDFCatalog_GetLanguage: unsafe extern "C" fn(Doc, *mut u16, u32) -> u32,
    pub FPDFBookmark_GetFirstChild: unsafe extern "C" fn(Doc, *mut c_void) -> *mut c_void,
    pub FPDF_GetDocPermissions: unsafe extern "C" fn(Doc) -> u32,
    pub FPDF_StructTree_GetForPage: unsafe extern "C" fn(Page) -> *mut c_void,
    pub FPDF_StructTree_Close: unsafe extern "C" fn(*mut c_void),
    pub FPDF_StructTree_CountChildren: unsafe extern "C" fn(*mut c_void) -> i32,
    pub FPDF_StructTree_GetChildAtIndex: unsafe extern "C" fn(*mut c_void, i32) -> *mut c_void,
    pub FPDF_StructElement_GetType: unsafe extern "C" fn(*mut c_void, *mut u16, u32) -> u32,
    pub FPDF_StructElement_GetAltText: unsafe extern "C" fn(*mut c_void, *mut u16, u32) -> u32,
    pub FPDF_StructElement_GetActualText: unsafe extern "C" fn(*mut c_void, *mut u16, u32) -> u32,
    pub FPDF_StructElement_CountChildren: unsafe extern "C" fn(*mut c_void) -> i32,
    pub FPDF_StructElement_GetChildAtIndex: unsafe extern "C" fn(*mut c_void, i32) -> *mut c_void,
    pub FPDFPage_CountObjects: unsafe extern "C" fn(Page) -> i32,
    pub FPDFPage_GetObject: unsafe extern "C" fn(Page, i32) -> *mut c_void,
    pub FPDFPageObj_GetType: unsafe extern "C" fn(*mut c_void) -> i32,
}

unsafe impl Send for Pdfium {}
unsafe impl Sync for Pdfium {}

pub(crate) fn pdfium() -> Result<&'static Pdfium, String> {
    static P: OnceLock<Option<Pdfium>> = OnceLock::new();
    // pdfium-render laadt de bibliotheek ook; zo weten we zeker dat dezelfde is geladen.
    let _ = crate::lezen::pdfium();
    P.get_or_init(|| unsafe {
        let pad = crate::win::w(&crate::lezen::onderdelen().join("pdfium.dll").to_string_lossy());
        let m = LoadLibraryW(pad.as_ptr());
        if m.is_null() {
            return None;
        }
        macro_rules! f {
            ($n:ident) => {{
                let naam = concat!(stringify!($n), "\0");
                std::mem::transmute(GetProcAddress(m, naam.as_ptr())?)
            }};
        }
        Some(Pdfium {
            FPDF_LoadMemDocument: f!(FPDF_LoadMemDocument),
            FPDF_CloseDocument: f!(FPDF_CloseDocument),
            FPDF_GetPageCount: f!(FPDF_GetPageCount),
            FPDF_LoadPage: f!(FPDF_LoadPage),
            FPDF_ClosePage: f!(FPDF_ClosePage),
            FPDFDOC_InitFormFillEnvironment: f!(FPDFDOC_InitFormFillEnvironment),
            FPDFDOC_ExitFormFillEnvironment: f!(FPDFDOC_ExitFormFillEnvironment),
            FORM_OnAfterLoadPage: f!(FORM_OnAfterLoadPage),
            FORM_OnBeforeClosePage: f!(FORM_OnBeforeClosePage),
            FPDFPage_GetAnnotCount: f!(FPDFPage_GetAnnotCount),
            FPDFPage_GetAnnot: f!(FPDFPage_GetAnnot),
            FPDFPage_CloseAnnot: f!(FPDFPage_CloseAnnot),
            FPDFAnnot_GetSubtype: f!(FPDFAnnot_GetSubtype),
            FPDFAnnot_GetRect: f!(FPDFAnnot_GetRect),
            FPDFAnnot_GetFormFieldType: f!(FPDFAnnot_GetFormFieldType),
            FPDFAnnot_GetFormFieldFlags: f!(FPDFAnnot_GetFormFieldFlags),
            FPDFAnnot_GetFormFieldName: f!(FPDFAnnot_GetFormFieldName),
            FPDFAnnot_GetFormFieldAlternateName: f!(FPDFAnnot_GetFormFieldAlternateName),
            FPDFAnnot_GetFormFieldValue: f!(FPDFAnnot_GetFormFieldValue),
            FPDFAnnot_GetFormFieldExportValue: f!(FPDFAnnot_GetFormFieldExportValue),
            FPDFAnnot_IsChecked: f!(FPDFAnnot_IsChecked),
            FPDFAnnot_GetOptionCount: f!(FPDFAnnot_GetOptionCount),
            FPDFAnnot_GetOptionLabel: f!(FPDFAnnot_GetOptionLabel),
            FPDFAnnot_IsOptionSelected: f!(FPDFAnnot_IsOptionSelected),
            FORM_SetFocusedAnnot: f!(FORM_SetFocusedAnnot),
            FORM_SelectAllText: f!(FORM_SelectAllText),
            FORM_ReplaceSelection: f!(FORM_ReplaceSelection),
            FORM_SetIndexSelected: f!(FORM_SetIndexSelected),
            FORM_ForceToKillFocus: f!(FORM_ForceToKillFocus),
            FORM_OnLButtonDown: f!(FORM_OnLButtonDown),
            FORM_OnLButtonUp: f!(FORM_OnLButtonUp),
            FPDF_SaveAsCopy: f!(FPDF_SaveAsCopy),
            FPDFText_LoadPage: f!(FPDFText_LoadPage),
            FPDFText_ClosePage: f!(FPDFText_ClosePage),
            FPDFText_GetBoundedText: f!(FPDFText_GetBoundedText),
            FPDF_GetPageWidthF: f!(FPDF_GetPageWidthF),
            FPDF_GetPageHeightF: f!(FPDF_GetPageHeightF),
            FPDFText_CountChars: f!(FPDFText_CountChars),
            FPDFText_GetUnicode: f!(FPDFText_GetUnicode),
            FPDFText_GetCharBox: f!(FPDFText_GetCharBox),
            FPDFBitmap_Create: f!(FPDFBitmap_Create),
            FPDFBitmap_FillRect: f!(FPDFBitmap_FillRect),
            FPDFBitmap_GetBuffer: f!(FPDFBitmap_GetBuffer),
            FPDFBitmap_GetStride: f!(FPDFBitmap_GetStride),
            FPDFBitmap_Destroy: f!(FPDFBitmap_Destroy),
            FPDF_RenderPageBitmap: f!(FPDF_RenderPageBitmap),
            FPDFPageObj_NewTextObj: f!(FPDFPageObj_NewTextObj),
            FPDFText_SetText: f!(FPDFText_SetText),
            FPDFPageObj_Transform: f!(FPDFPageObj_Transform),
            FPDFPage_InsertObject: f!(FPDFPage_InsertObject),
            FPDFPage_GenerateContent: f!(FPDFPage_GenerateContent),
            FPDFPageObj_NewImageObj: f!(FPDFPageObj_NewImageObj),
            FPDFImageObj_SetBitmap: f!(FPDFImageObj_SetBitmap),
            FPDFImageObj_SetMatrix: f!(FPDFImageObj_SetMatrix),
            FPDFAnnot_GetFlags: f!(FPDFAnnot_GetFlags),
            FPDFAnnot_SetFlags: f!(FPDFAnnot_SetFlags),
            FPDF_GetMetaText: f!(FPDF_GetMetaText),
            FPDFCatalog_IsTagged: f!(FPDFCatalog_IsTagged),
            FPDFCatalog_GetLanguage: f!(FPDFCatalog_GetLanguage),
            FPDFBookmark_GetFirstChild: f!(FPDFBookmark_GetFirstChild),
            FPDF_GetDocPermissions: f!(FPDF_GetDocPermissions),
            FPDF_StructTree_GetForPage: f!(FPDF_StructTree_GetForPage),
            FPDF_StructTree_Close: f!(FPDF_StructTree_Close),
            FPDF_StructTree_CountChildren: f!(FPDF_StructTree_CountChildren),
            FPDF_StructTree_GetChildAtIndex: f!(FPDF_StructTree_GetChildAtIndex),
            FPDF_StructElement_GetType: f!(FPDF_StructElement_GetType),
            FPDF_StructElement_GetAltText: f!(FPDF_StructElement_GetAltText),
            FPDF_StructElement_GetActualText: f!(FPDF_StructElement_GetActualText),
            FPDF_StructElement_CountChildren: f!(FPDF_StructElement_CountChildren),
            FPDF_StructElement_GetChildAtIndex: f!(FPDF_StructElement_GetChildAtIndex),
            FPDFPage_CountObjects: f!(FPDFPage_CountObjects),
            FPDFPage_GetObject: f!(FPDFPage_GetObject),
            FPDFPageObj_GetType: f!(FPDFPageObj_GetType),
        })
    })
    .as_ref()
    .ok_or_else(|| crate::teksten::t("LezenGeenPdfium"))
}

/// PDFium mag maar door één draad tegelijk gebruikt worden.
pub(crate) static SLOT: Mutex<()> = Mutex::new(());

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Soort {
    Tekst,
    Vinkje,
    /// Keuzerondjes: één keuze uit een groep.
    Rondje,
    /// Keuzelijst (vervolgkeuzelijst of lijst).
    Lijst,
}

#[derive(Clone, Debug)]
pub struct Veld {
    pub pagina: i32,
    /// De widgets (bij keuzerondjes meer dan één), als index op de pagina.
    pub annots: Vec<i32>,
    pub soort: Soort,
    pub naam: String,
    pub vraag: String,
    pub opties: Vec<String>,
    pub meerregelig: bool,
    pub verplicht: bool,
    /// Tekst, of "1"/"" bij een vinkje, of het nummer van de keuze bij rondjes en lijsten.
    pub waarde: String,
    pub links: f32,
    pub boven: f32,
    /// Een vraag naar de handtekening (een handtekeningveld, of een tekstveld met "handtekening" erin).
    pub handtekening: bool,
    /// Het vak van het veld in PDF-punten: links, onder, rechts, boven.
    pub vak: [f32; 4],
}

/// Een open document met formulier-omgeving; alles wordt netjes gesloten bij het weggooien.
struct Open<'a> {
    p: &'a Pdfium,
    _data: Vec<u8>,
    doc: Doc,
    form: Form,
    _info: Box<FormFillInfo>,
}

impl<'a> Open<'a> {
    fn nieuw(p: &'a Pdfium, pad: &Path) -> Result<Open<'a>, String> {
        let data = std::fs::read(pad).map_err(|e| e.to_string())?;
        unsafe {
            let doc = (p.FPDF_LoadMemDocument)(data.as_ptr() as *const c_void, data.len() as i32, std::ptr::null());
            if doc.is_null() {
                return Err(crate::teksten::t("LezenKapot"));
            }
            let mut info = Box::new(FormFillInfo { version: 1, velden: [std::ptr::null_mut(); 16], reserve: [std::ptr::null_mut(); 48] });
            let form = (p.FPDFDOC_InitFormFillEnvironment)(doc, &mut *info);
            Ok(Open { p, _data: data, doc, form, _info: info })
        }
    }

    fn pagina(&self, nr: i32) -> Option<Page> {
        unsafe {
            let page = (self.p.FPDF_LoadPage)(self.doc, nr);
            if page.is_null() {
                return None;
            }
            (self.p.FORM_OnAfterLoadPage)(page, self.form);
            Some(page)
        }
    }

    fn sluit_pagina(&self, page: Page) {
        unsafe {
            (self.p.FORM_OnBeforeClosePage)(page, self.form);
            (self.p.FPDF_ClosePage)(page);
        }
    }
}

impl Drop for Open<'_> {
    fn drop(&mut self) {
        unsafe {
            if !self.form.is_null() {
                (self.p.FPDFDOC_ExitFormFillEnvironment)(self.form);
            }
            (self.p.FPDF_CloseDocument)(self.doc);
        }
    }
}

fn tekst_van(f: impl Fn(*mut u16, u32) -> u32) -> String {
    let n = f(std::ptr::null_mut(), 0);
    if n <= 2 {
        return String::new();
    }
    let mut buf = vec![0u16; (n as usize) / 2];
    f(buf.as_mut_ptr(), n);
    let eind = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..eind]).trim().to_string()
}

/// Tekst op de pagina binnen een rechthoek (pagina-punten, y omhoog).
fn tekst_in(p: &Pdfium, tekstpagina: *mut c_void, links: f64, boven: f64, rechts: f64, onder: f64) -> String {
    unsafe {
        let n = (p.FPDFText_GetBoundedText)(tekstpagina, links, boven, rechts, onder, std::ptr::null_mut(), 0);
        if n <= 0 {
            return String::new();
        }
        let mut buf = vec![0u16; n as usize + 1];
        let m = (p.FPDFText_GetBoundedText)(tekstpagina, links, boven, rechts, onder, buf.as_mut_ptr(), n + 1);
        let t = String::from_utf16_lossy(&buf[..m.max(0) as usize]);
        // pdfium telt het nul-teken aan het eind mee; dat en andere stuurtekens horen niet in een vraag.
        let t: String = t.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
        t.split_whitespace().collect::<Vec<_>>().join(" ")
    }
}

/// Een technische veldnaam leesbaar maken ("geboorte_datum" -> "geboorte datum"); "Text12" telt niet.
fn nette_naam(naam: &str) -> Option<String> {
    let laatste = naam.rsplit('.').next().unwrap_or(naam);
    let t = laatste.replace(['_', '-'], " ");
    let t = t.trim_end_matches(|c: char| c.is_ascii_digit() || c == '[' || c == ']').trim().to_string();
    let saai = ["text", "tekst", "textfield", "tekstveld", "field", "veld", "check box", "checkbox", "selectievakje", "radio", "button", "group", "fill", ""];
    if saai.contains(&t.to_lowercase().as_str()) || t.chars().filter(|c| c.is_alphabetic()).count() < 2 {
        None
    } else {
        Some(t)
    }
}

/// De vraag bij een veld: de beschrijving in de PDF (tooltip), of de tekst links of boven het veld, of de naam.
fn zoek_vraag(p: &Pdfium, tp: *mut c_void, soort: Soort, r: &Rect, alternatief: &str, naam: &str) -> String {
    if !alternatief.is_empty() {
        return alternatief.to_string();
    }
    let (l, b, rr, o) = (r.left as f64, r.top as f64, r.right as f64, r.bottom as f64);
    let schoon = |s: String| s.trim().trim_end_matches(['.', '…', '_']).trim().trim_end_matches(':').trim().to_string();
    let mut v = String::new();
    if soort == Soort::Vinkje || soort == Soort::Rondje {
        v = schoon(tekst_in(p, tp, rr + 1.0, b + 2.0, rr + 260.0, o - 2.0));
    }
    if v.is_empty() {
        v = schoon(tekst_in(p, tp, (l - 260.0).max(0.0), b + 2.0, l - 1.0, o - 2.0));
        // Alleen het laatste stuk (de vraag vlak voor het veld).
        if v.chars().count() > 80 {
            v = v.chars().rev().take(80).collect::<Vec<_>>().into_iter().rev().collect::<String>().trim().to_string();
        }
    }
    if v.is_empty() {
        v = schoon(tekst_in(p, tp, l - 4.0, b + 22.0, rr + 40.0, b + 1.0));
    }
    if v.is_empty() {
        v = nette_naam(naam).unwrap_or_default();
    }
    v
}

pub fn lees_velden(pad: &Path) -> Result<Vec<Veld>, String> {
    let p = pdfium()?;
    let _slot = SLOT.lock().unwrap();
    let open = Open::nieuw(p, pad)?;
    if open.form.is_null() {
        return Ok(Vec::new());
    }
    let mut velden: Vec<Veld> = Vec::new();
    unsafe {
        for nr in 0..(p.FPDF_GetPageCount)(open.doc) {
            let Some(page) = open.pagina(nr) else { continue };
            let tp = (p.FPDFText_LoadPage)(page);
            let mut op_pagina: Vec<Veld> = Vec::new();
            for i in 0..(p.FPDFPage_GetAnnotCount)(page) {
                let a = (p.FPDFPage_GetAnnot)(page, i);
                if a.is_null() {
                    continue;
                }
                if (p.FPDFAnnot_GetSubtype)(a) != 20 {
                    (p.FPDFPage_CloseAnnot)(a);
                    continue;
                }
                let f = open.form;
                let soort = match (p.FPDFAnnot_GetFormFieldType)(f, a) {
                    2 => Some(Soort::Vinkje),
                    3 => Some(Soort::Rondje),
                    4 | 5 => Some(Soort::Lijst),
                    6 | 7 => Some(Soort::Tekst), // 7 = handtekeningveld
                    _ => None, // knoppen
                };
                let vlaggen = (p.FPDFAnnot_GetFormFieldFlags)(f, a);
                let Some(soort) = soort else {
                    (p.FPDFPage_CloseAnnot)(a);
                    continue;
                };
                if vlaggen & 1 != 0 {
                    (p.FPDFPage_CloseAnnot)(a);
                    continue; // alleen lezen
                }
                let naam = tekst_van(|b, n| (p.FPDFAnnot_GetFormFieldName)(f, a, b, n));
                let alternatief = tekst_van(|b, n| (p.FPDFAnnot_GetFormFieldAlternateName)(f, a, b, n));
                let mut r = Rect { left: 0.0, top: 0.0, right: 0.0, bottom: 0.0 };
                (p.FPDFAnnot_GetRect)(a, &mut r);
                if r.top < r.bottom {
                    std::mem::swap(&mut r.top, &mut r.bottom);
                }
                let vraag = zoek_vraag(p, tp, soort, &r, &alternatief, &naam);
                match soort {
                    Soort::Rondje => {
                        let optie = {
                            let e = tekst_van(|b, n| (p.FPDFAnnot_GetFormFieldExportValue)(f, a, b, n));
                            let naast = tekst_in(p, tp, r.right as f64 + 1.0, r.top as f64 + 2.0, r.right as f64 + 200.0, r.bottom as f64 - 2.0);
                            if !naast.is_empty() { naast } else { e }
                        };
                        let aan = (p.FPDFAnnot_IsChecked)(f, a) != 0;
                        if let Some(g) = op_pagina.iter_mut().find(|v| v.soort == Soort::Rondje && v.naam == naam) {
                            if aan {
                                g.waarde = g.opties.len().to_string();
                            }
                            g.annots.push(i);
                            g.opties.push(optie);
                        } else {
                            let groep = if !alternatief.is_empty() { alternatief.clone() } else {
                                let boven = tekst_in(p, tp, r.left as f64 - 4.0, r.top as f64 + 22.0, r.left as f64 + 300.0, r.top as f64 + 1.0);
                                if !boven.is_empty() { boven } else { nette_naam(&naam).unwrap_or_else(|| vraag.clone()) }
                            };
                            op_pagina.push(Veld { pagina: nr, annots: vec![i], soort, naam, vraag: groep, opties: vec![optie],
                                meerregelig: false, verplicht: vlaggen & 2 != 0, waarde: if aan { "0".into() } else { String::new() },
                                links: r.left, boven: r.top, handtekening: false, vak: [r.left, r.bottom, r.right, r.top] });
                        }
                    }
                    _ => {
                        let mut opties = Vec::new();
                        let mut waarde = String::new();
                        match soort {
                            Soort::Lijst => {
                                for k in 0..(p.FPDFAnnot_GetOptionCount)(f, a).max(0) {
                                    opties.push(tekst_van(|b, n| (p.FPDFAnnot_GetOptionLabel)(f, a, k, b, n)));
                                    if (p.FPDFAnnot_IsOptionSelected)(f, a, k) != 0 {
                                        waarde = k.to_string();
                                    }
                                }
                            }
                            Soort::Vinkje => waarde = if (p.FPDFAnnot_IsChecked)(f, a) != 0 { "1".into() } else { String::new() },
                            _ => waarde = tekst_van(|b, n| (p.FPDFAnnot_GetFormFieldValue)(f, a, b, n)),
                        }
                        let handtekening = (p.FPDFAnnot_GetFormFieldType)(f, a) == 7
                            || (soort == Soort::Tekst && (crate::handtekening::is_vraag(&vraag) || crate::handtekening::is_vraag(&naam)));
                        op_pagina.push(Veld { pagina: nr, annots: vec![i], soort, naam, vraag, opties,
                            meerregelig: vlaggen & 4096 != 0, verplicht: vlaggen & 2 != 0, waarde, links: r.left, boven: r.top,
                            handtekening, vak: [r.left, r.bottom, r.right, r.top] });
                    }
                }
                (p.FPDFPage_CloseAnnot)(a);
            }
            // Leesvolgorde: van boven naar beneden, en op dezelfde regel van links naar rechts.
            op_pagina.sort_by(|x, y| {
                if (x.boven - y.boven).abs() < 6.0 {
                    x.links.partial_cmp(&y.links).unwrap_or(std::cmp::Ordering::Equal)
                } else {
                    y.boven.partial_cmp(&x.boven).unwrap_or(std::cmp::Ordering::Equal)
                }
            });
            velden.extend(op_pagina);
            if !tp.is_null() {
                (p.FPDFText_ClosePage)(tp);
            }
            open.sluit_pagina(page);
        }
    }
    // Lege vragen een nummer geven.
    let ontbreekt = crate::teksten::t("VeldZonderNaam");
    for (i, v) in velden.iter_mut().enumerate() {
        if v.vraag.is_empty() {
            v.vraag = crate::teksten::tf("VeldNummer", &[&ontbreekt, &(i + 1).to_string()]);
        }
    }
    Ok(velden)
}

/// Voor Controleren: alle invulvelden met (pagina, naam, beschrijving voor de schermlezer = TU).
pub fn velden_met_beschrijving(pad: &Path) -> Result<Vec<(i32, String, String)>, String> {
    let p = pdfium()?;
    let _slot = SLOT.lock().unwrap();
    let open = Open::nieuw(p, pad)?;
    let mut uit = Vec::new();
    if open.form.is_null() {
        return Ok(uit);
    }
    unsafe {
        for nr in 0..(p.FPDF_GetPageCount)(open.doc) {
            let Some(page) = open.pagina(nr) else { continue };
            for i in 0..(p.FPDFPage_GetAnnotCount)(page) {
                let a = (p.FPDFPage_GetAnnot)(page, i);
                if a.is_null() {
                    continue;
                }
                if (p.FPDFAnnot_GetSubtype)(a) == 20 {
                    let soort = (p.FPDFAnnot_GetFormFieldType)(open.form, a);
                    // 1 = drukknop: telt ook mee; 0 = onbekend niet.
                    if soort > 0 {
                        let naam = tekst_van(|b, n| (p.FPDFAnnot_GetFormFieldName)(open.form, a, b, n));
                        let tu = tekst_van(|b, n| (p.FPDFAnnot_GetFormFieldAlternateName)(open.form, a, b, n));
                        if !uit.iter().any(|(_, x, _): &(i32, String, String)| *x == naam) {
                            uit.push((nr + 1, naam, tu));
                        }
                    }
                }
                (p.FPDFPage_CloseAnnot)(a);
            }
            open.sluit_pagina(page);
        }
    }
    Ok(uit)
}

/// Vult de velden in en slaat het formulier op als nieuwe PDF.
pub fn vul_in(bron: &Path, velden: &[Veld], doel: &Path) -> Result<(), String> {
    let p = pdfium()?;
    let _slot = SLOT.lock().unwrap();
    let open = Open::nieuw(p, bron)?;
    if open.form.is_null() {
        return Err(crate::teksten::t("GeenFormulier"));
    }
    unsafe {
        let mut paginas: Vec<i32> = velden.iter().map(|v| v.pagina).collect();
        paginas.dedup();
        for nr in paginas {
            let Some(page) = open.pagina(nr) else { continue };
            for v in velden.iter().filter(|v| v.pagina == nr) {
                let pak = |i: i32| (p.FPDFPage_GetAnnot)(page, i);
                let klik = |a: Annot| {
                    let mut r = Rect { left: 0.0, top: 0.0, right: 0.0, bottom: 0.0 };
                    (p.FPDFAnnot_GetRect)(a, &mut r);
                    let (x, y) = (((r.left + r.right) / 2.0) as f64, ((r.top + r.bottom) / 2.0) as f64);
                    (p.FORM_OnLButtonDown)(open.form, page, 0, x, y);
                    (p.FORM_OnLButtonUp)(open.form, page, 0, x, y);
                };
                match v.soort {
                    Soort::Tekst => {
                        let a = pak(v.annots[0]);
                        if a.is_null() {
                            continue;
                        }
                        // In een echt handtekeningveld kan geen tekst; de handtekening komt er later als plaatje op.
                        if (p.FPDFAnnot_GetFormFieldType)(open.form, a) == 7 || crate::handtekening::is_merkteken(&v.waarde) {
                            (p.FPDFPage_CloseAnnot)(a);
                            continue;
                        }
                        let huidig = tekst_van(|b, n| (p.FPDFAnnot_GetFormFieldValue)(open.form, a, b, n));
                        if huidig != v.waarde {
                            (p.FORM_SetFocusedAnnot)(open.form, a);
                            (p.FORM_SelectAllText)(open.form, page);
                            let w: Vec<u16> = v.waarde.encode_utf16().chain(std::iter::once(0)).collect();
                            (p.FORM_ReplaceSelection)(open.form, page, w.as_ptr());
                            (p.FORM_ForceToKillFocus)(open.form);
                        }
                        (p.FPDFPage_CloseAnnot)(a);
                    }
                    Soort::Vinkje => {
                        let a = pak(v.annots[0]);
                        if a.is_null() {
                            continue;
                        }
                        let moet = !v.waarde.is_empty();
                        if ((p.FPDFAnnot_IsChecked)(open.form, a) != 0) != moet {
                            klik(a);
                            (p.FORM_ForceToKillFocus)(open.form);
                        }
                        (p.FPDFPage_CloseAnnot)(a);
                    }
                    Soort::Rondje => {
                        if let Ok(k) = v.waarde.parse::<usize>() {
                            if let Some(i) = v.annots.get(k) {
                                let a = pak(*i);
                                if !a.is_null() {
                                    if (p.FPDFAnnot_IsChecked)(open.form, a) == 0 {
                                        klik(a);
                                        (p.FORM_ForceToKillFocus)(open.form);
                                    }
                                    (p.FPDFPage_CloseAnnot)(a);
                                }
                            }
                        }
                    }
                    Soort::Lijst => {
                        if let Ok(k) = v.waarde.parse::<i32>() {
                            let a = pak(v.annots[0]);
                            if !a.is_null() {
                                (p.FORM_SetFocusedAnnot)(open.form, a);
                                (p.FORM_SetIndexSelected)(open.form, page, k, 1);
                                (p.FORM_ForceToKillFocus)(open.form);
                                (p.FPDFPage_CloseAnnot)(a);
                            }
                        }
                    }
                }
            }
            open.sluit_pagina(page);
        }
        let mut schrijver = FileWrite { version: 1, write_block: schrijf_blok, data: Vec::new() };
        if (p.FPDF_SaveAsCopy)(open.doc, &mut schrijver, 0) == 0 {
            return Err(crate::teksten::t("OpslaanMislukt"));
        }
        std::fs::write(doel, &schrijver.data).map_err(|e| e.to_string())
    }
}
