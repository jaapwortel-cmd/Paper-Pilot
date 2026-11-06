// Paper Pilot - vensters met een vraag of melding (TaskDialog van Windows).
// Schermlezers lezen zo'n venster in zijn geheel voor zodra het opent, en voor zienden ziet het er netjes uit.

use crate::teksten::t;
use crate::win::w;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::UI::Controls::*;
use windows_sys::Win32::UI::WindowsAndMessaging::{IDCANCEL, IDNO, IDYES};

pub const ICOON_INFO: u16 = 0xFFFD; // TD_INFORMATION_ICON
pub const ICOON_WAARSCHUWING: u16 = 0xFFFF; // TD_WARNING_ICON

pub struct Knop {
    pub id: i32,
    pub tekst: String,
}

pub struct Vraag<'a> {
    pub titel: &'a str,
    pub kop: &'a str,
    pub tekst: &'a str,
    pub icoon: u16,
    pub knoppen: Vec<Knop>,
    pub opdrachtlinks: bool,
    pub standaard: i32,
    /// TDCBF_OK_BUTTON enz.
    pub gewone_knoppen: TASKDIALOG_COMMON_BUTTON_FLAGS,
    pub onderschrift: Option<&'a str>,
}

impl<'a> Vraag<'a> {
    pub fn nieuw(titel: &'a str, kop: &'a str, tekst: &'a str) -> Self {
        Vraag { titel, kop, tekst, icoon: 0, knoppen: Vec::new(), opdrachtlinks: false, standaard: 0,
            gewone_knoppen: 0, onderschrift: None }
    }

    /// Toont het venster en geeft het nummer van de gekozen knop (IDCANCEL bij Escape).
    pub fn toon(&self, eigenaar: HWND) -> i32 {
        let titel = w(self.titel);
        let kop = w(self.kop);
        let tekst = w(self.tekst);
        let onder = self.onderschrift.map(w);
        let teksten: Vec<Vec<u16>> = self.knoppen.iter().map(|k| w(&k.tekst)).collect();
        let knoppen: Vec<TASKDIALOG_BUTTON> = self.knoppen.iter().zip(&teksten)
            .map(|(k, t)| TASKDIALOG_BUTTON { nButtonID: k.id, pszButtonText: t.as_ptr() }).collect();
        let mut c: TASKDIALOGCONFIG = unsafe { std::mem::zeroed() };
        c.cbSize = std::mem::size_of::<TASKDIALOGCONFIG>() as u32;
        c.hwndParent = eigenaar;
        c.hInstance = crate::win::instantie();
        c.dwFlags = TDF_ALLOW_DIALOG_CANCELLATION | TDF_POSITION_RELATIVE_TO_WINDOW
            | if self.opdrachtlinks { TDF_USE_COMMAND_LINKS } else { 0 };
        c.dwCommonButtons = self.gewone_knoppen;
        c.pszWindowTitle = titel.as_ptr();
        c.pszMainInstruction = kop.as_ptr();
        c.pszContent = tekst.as_ptr();
        if self.icoon != 0 {
            c.Anonymous1.pszMainIcon = self.icoon as usize as *const u16;
        }
        if !knoppen.is_empty() {
            c.cButtons = knoppen.len() as u32;
            c.pButtons = knoppen.as_ptr();
        }
        c.nDefaultButton = self.standaard;
        if let Some(o) = &onder {
            c.pszFooter = o.as_ptr();
        }
        let mut gekozen = 0i32;
        let r = unsafe { TaskDialogIndirect(&c, &mut gekozen, std::ptr::null_mut(), std::ptr::null_mut()) };
        if r != 0 {
            crate::logboek::schrijf(&format!("TaskDialog lukte niet ({r:#x})."));
            return IDCANCEL;
        }
        gekozen
    }
}

pub fn melding(eigenaar: HWND, kop: &str, tekst: &str, icoon: u16) {
    let titel = t("Programma");
    let mut v = Vraag::nieuw(&titel, kop, tekst);
    v.icoon = icoon;
    v.gewone_knoppen = TDCBF_OK_BUTTON;
    v.toon(eigenaar);
}

/// Een vraag met Ja en Nee. Geeft true bij Ja.
pub fn ja_nee(eigenaar: HWND, kop: &str, tekst: &str, standaard_ja: bool) -> bool {
    let titel = t("Programma");
    let mut v = Vraag::nieuw(&titel, kop, tekst);
    v.icoon = ICOON_INFO;
    v.gewone_knoppen = TDCBF_YES_BUTTON | TDCBF_NO_BUTTON;
    v.standaard = if standaard_ja { IDYES } else { IDNO };
    v.toon(eigenaar) == IDYES
}

/// De rondleiding: een paar korte pagina's met Vorige, Volgende en Sluiten.
pub fn rondleiding(eigenaar: HWND) {
    const VORIGE: i32 = 1000;
    const VOLGENDE: i32 = 1001;
    const SLUITEN: i32 = 1002;
    let aantal = 5;
    let mut pagina = 1;
    loop {
        let titel = crate::teksten::tf("RondleidingTitel", &[&pagina.to_string(), &aantal.to_string()]);
        let kop = t(&format!("Rondleiding{pagina}Kop"));
        let tekst = t(&format!("Rondleiding{pagina}"));
        let mut v = Vraag::nieuw(&titel, &kop, &tekst);
        v.icoon = ICOON_INFO;
        if pagina > 1 {
            v.knoppen.push(Knop { id: VORIGE, tekst: t("KnopVorige") });
        }
        if pagina < aantal {
            v.knoppen.push(Knop { id: VOLGENDE, tekst: t("KnopVolgende") });
            v.knoppen.push(Knop { id: SLUITEN, tekst: t("KnopSluiten") });
            v.standaard = VOLGENDE;
        } else {
            v.knoppen.push(Knop { id: SLUITEN, tekst: t("KnopBeginnen") });
            v.standaard = SLUITEN;
        }
        match v.toon(eigenaar) {
            VORIGE => pagina -= 1,
            VOLGENDE => pagina += 1,
            _ => break,
        }
    }
}

/// Meer programma's van Jaap: elk programma is een knop. Kiezen = Jaap er een mail over sturen.
pub fn meer_programmas(eigenaar: HWND) {
    let programmas = ["SpraakNaarTekst", "Maatje", "AudioEditor", "Taxi"];
    let titel = t("MeerProgrammas");
    let kop = t("MeerProgrammasKop");
    let tekst = t("MeerProgrammasTekst");
    let mut v = Vraag::nieuw(&titel, &kop, &tekst);
    v.opdrachtlinks = true;
    v.gewone_knoppen = TDCBF_CLOSE_BUTTON;
    for (i, p) in programmas.iter().enumerate() {
        v.knoppen.push(Knop { id: 2000 + i as i32, tekst: format!("{}\n{}", t(&format!("Prog{p}Naam")), t(&format!("Prog{p}Over"))) });
    }
    let gekozen = v.toon(eigenaar);
    if (2000..2000 + programmas.len() as i32).contains(&gekozen) {
        let naam = t(&format!("Prog{}Naam", programmas[(gekozen - 2000) as usize]));
        crate::hoofdvenster::mail(&crate::teksten::tf("MailOnderwerpProgramma", &[&naam]),
            &crate::teksten::tf("MailTekstProgramma", &[&naam]));
    }
}

pub fn over(eigenaar: HWND) {
    let titel = t("Over");
    let kop = format!("Paper Pilot {}", crate::VERSIE);
    let tekst = t("OverTekst");
    let mut v = Vraag::nieuw(&titel, &kop, &tekst);
    v.icoon = ICOON_INFO;
    v.gewone_knoppen = TDCBF_OK_BUTTON;
    v.toon(eigenaar);
}
