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
    // Elke waarschuwing ook in het logboek, zodat de maker kan zien wat er misging.
    if icoon == ICOON_WAARSCHUWING {
        crate::logboek::schrijf(&format!("Melding: {kop} | {}", tekst.replace('\n', " ")));
    }
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

/// Meer programma's van Jaap: eerst de lijst, dan per programma een uitleg met een knop om Jaap te mailen
/// of je het mag uitproberen. Na de uitleg kom je terug in de lijst.
pub fn meer_programmas(eigenaar: HWND) {
    const PROGRAMMAS: [&str; 5] = ["Verhalenschrijver", "SpraakNaarTekst", "Maatje", "AudioEditor", "Taxi"];
    const MAILEN: i32 = 2100;
    const TERUG: i32 = 2101;
    const DOWNLOAD: i32 = 2102;
    /// De Verhalenschrijver is vrij te downloaden (Dropbox-map van Jaap, alleen bekijken).
    const VERHALENSCHRIJVER: &str = "https://www.dropbox.com/scl/fo/rjtldoglzc5mh4rpzppid/AD07Kwa7IBq-AIH3C7OgUW8?rlkey=yt8ipqtdn2jug232sus5nfs2p&dl=0";
    let mut vorige = 2000;
    loop {
        let titel = t("MeerProgrammas");
        let kop = t("MeerProgrammasKop");
        let tekst = t("MeerProgrammasTekst");
        let mut v = Vraag::nieuw(&titel, &kop, &tekst);
        v.opdrachtlinks = true;
        v.gewone_knoppen = TDCBF_CLOSE_BUTTON;
        v.standaard = vorige;
        for (i, p) in PROGRAMMAS.iter().enumerate() {
            v.knoppen.push(Knop { id: 2000 + i as i32, tekst: format!("{}\n{}", t(&format!("Prog{p}Naam")), t(&format!("Prog{p}Over"))) });
        }
        let gekozen = v.toon(eigenaar);
        if !(2000..2000 + PROGRAMMAS.len() as i32).contains(&gekozen) {
            return;
        }
        vorige = gekozen;
        let p = PROGRAMMAS[(gekozen - 2000) as usize];
        let naam = t(&format!("Prog{p}Naam"));
        let uitleg = t(&format!("Prog{p}Uitleg"));
        let mut u = Vraag::nieuw(&titel, &naam, &uitleg);
        u.icoon = ICOON_INFO;
        u.opdrachtlinks = true;
        let download = p == "Verhalenschrijver";
        if download {
            u.knoppen.push(Knop { id: DOWNLOAD, tekst: crate::teksten::tf("KnopProgrammaDownloaden", &[&naam]) });
        }
        u.knoppen.push(Knop { id: MAILEN, tekst: crate::teksten::tf(if download { "KnopProgrammaVraag" } else { "KnopProgrammaMailen" }, &[&naam]) });
        u.knoppen.push(Knop { id: TERUG, tekst: t("KnopTerugNaarLijst") });
        u.gewone_knoppen = TDCBF_CLOSE_BUTTON;
        u.standaard = if download { DOWNLOAD } else { MAILEN };
        match u.toon(eigenaar) {
            DOWNLOAD => {
                melding(eigenaar, &t("DownloadOpenenKop"), &crate::teksten::tf("DownloadOpenenTekst", &[&naam]), ICOON_INFO);
                crate::win::open_adres(VERHALENSCHRIJVER);
                return;
            }
            MAILEN => {
                // Een mailprogramma opent soms pas na een paar seconden: eerst zeggen wat er gaat gebeuren.
                melding(eigenaar, &t("MailOpenenKop"), &crate::teksten::tf("MailOpenenTekst", &[&naam]), ICOON_INFO);
                crate::hoofdvenster::mail(&crate::teksten::tf("MailOnderwerpProgramma", &[&naam]),
                    &crate::teksten::tf("MailTekstProgramma", &[&naam]));
                return;
            }
            TERUG => continue,
            _ => return,
        }
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

// ------------------------------------------------------------------ werk met een voortgangsbalk

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Gedeelde stand tussen het werk (op een eigen draad) en het voortgangsvenster.
pub struct Voortgang {
    pub gestopt: AtomicBool,
    /// Het voortgangsvenster zelf (voor meldingen aan de schermlezer vanuit het werk).
    pub venster: std::sync::atomic::AtomicIsize,
    /// Een onverwachte fout in het werk.
    fout: Mutex<Option<String>>,
    stand: Mutex<(String, u32, bool)>,
    getoond: Mutex<(String, u32)>,
}

impl Voortgang {
    pub fn nieuw() -> Voortgang {
        Voortgang { gestopt: AtomicBool::new(false), venster: std::sync::atomic::AtomicIsize::new(0), fout: Mutex::new(None), stand: Mutex::new((String::from("\u{a0}"), 0, false)), getoond: Mutex::new((String::new(), 0)) }
    }

    pub fn meld(&self, tekst: String, procent: u32) {
        let mut s = self.stand.lock().unwrap();
        s.0 = tekst;
        s.1 = procent.min(100);
    }
}

const TDN_TIMER: u32 = 4;
const TDN_BUTTON_CLICKED: u32 = 2;
const TDM_SET_ELEMENT_TEXT: u32 = 0x0400 + 108;
const TDM_SET_PROGRESS_BAR_POS: u32 = 0x0400 + 106;
const TDM_SET_PROGRESS_BAR_MARQUEE: u32 = 0x0400 + 107;

unsafe extern "system" fn werk_terug(hwnd: HWND, msg: u32, wp: WPARAM, _lp: LPARAM, data: isize) -> windows_sys::core::HRESULT {
    use windows_sys::Win32::UI::WindowsAndMessaging::{PostMessageW, SendMessageW, WM_CLOSE};
    let v = &*(data as *const Voortgang);
    match msg {
        TDN_TIMER => {
            v.venster.store(hwnd as isize, Ordering::Relaxed);
            let (tekst, pct, klaar) = v.stand.lock().unwrap().clone();
            let mut getoond = v.getoond.lock().unwrap();
            if tekst != getoond.0 {
                let t = w(&tekst);
                SendMessageW(hwnd, TDM_SET_ELEMENT_TEXT, 0, t.as_ptr() as LPARAM);
                getoond.0 = tekst;
            }
            if pct != getoond.1 && pct > 0 {
                SendMessageW(hwnd, TDM_SET_PROGRESS_BAR_MARQUEE, 0, 0);
                SendMessageW(hwnd, TDM_SET_PROGRESS_BAR_POS, pct as WPARAM, 0);
                getoond.1 = pct;
            }
            if klaar {
                PostMessageW(hwnd, WM_CLOSE, 0, 0);
            }
        }
        TDN_BUTTON_CLICKED => {
            // Let op: Windows meldt ook IDCANCEL als Paper Pilot het venster zelf sluit omdat het werk klaar is
            // (WM_CLOSE). Dan is het geen Annuleren van de gebruiker.
            if wp as i32 == IDCANCEL && !v.stand.lock().unwrap().2 {
                v.gestopt.store(true, Ordering::Relaxed);
            }
        }
        _ => {}
    }
    0
}

/// Voert werk uit op een eigen draad en toont zolang een venster met een voortgangsbalk en Annuleren.
/// Geeft None als de gebruiker annuleert.
pub fn werk<T: Send + 'static>(eigenaar: HWND, titel: &str, kop: &str, taak: impl FnOnce(&Voortgang) -> T + Send + 'static) -> Option<T> {
    let v = Arc::new(Voortgang::nieuw());
    let uitkomst: Arc<Mutex<Option<T>>> = Arc::new(Mutex::new(None));
    {
        let v = v.clone();
        let uitkomst = uitkomst.clone();
        std::thread::spawn(move || {
            crate::beeld::com_start();
            // Gaat er onverwacht iets mis (een paniek), dan geen stilte: het venster sluit en de gebruiker hoort het.
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| taak(&v))) {
                Ok(r) => *uitkomst.lock().unwrap() = Some(r),
                Err(e) => {
                    let tekst = e.downcast_ref::<String>().cloned()
                        .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default();
                    crate::logboek::schrijf(&format!("Fout in het werk op de achtergrond: {tekst}"));
                    *v.fout.lock().unwrap() = Some(tekst);
                }
            }
            v.stand.lock().unwrap().2 = true;
        });
    }
    let titel_w = w(titel);
    let kop_w = w(kop);
    let tekst_w = w("\u{a0}");
    let mut c: TASKDIALOGCONFIG = unsafe { std::mem::zeroed() };
    c.cbSize = std::mem::size_of::<TASKDIALOGCONFIG>() as u32;
    c.hwndParent = eigenaar;
    c.hInstance = crate::win::instantie();
    c.dwFlags = TDF_ALLOW_DIALOG_CANCELLATION | TDF_POSITION_RELATIVE_TO_WINDOW | TDF_SHOW_MARQUEE_PROGRESS_BAR
        | TDF_SHOW_PROGRESS_BAR | TDF_CALLBACK_TIMER;
    c.dwCommonButtons = TDCBF_CANCEL_BUTTON;
    c.pszWindowTitle = titel_w.as_ptr();
    c.pszMainInstruction = kop_w.as_ptr();
    c.pszContent = tekst_w.as_ptr();
    c.Anonymous1.pszMainIcon = ICOON_INFO as usize as *const u16;
    c.pfCallback = Some(werk_terug);
    c.lpCallbackData = Arc::as_ptr(&v) as isize;
    // Is het werk al klaar voordat het venster verschijnt, dan sluit de timer het meteen.
    let r = unsafe { TaskDialogIndirect(&c, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut()) };
    if r != 0 {
        crate::logboek::schrijf(&format!("Voortgangsvenster lukte niet ({r:#x}); Paper Pilot wacht zonder venster."));
    }
    // Is het werk klaar, dan telt de uitkomst altijd (ook als het venster als "Annuleren" sloot).
    if let Some(r) = uitkomst.lock().unwrap().take() {
        return Some(r);
    }
    if v.gestopt.load(Ordering::Relaxed) {
        crate::logboek::schrijf(&format!("Geannuleerd: {kop}"));
        return None;
    }
    // Het venster is dicht: wachten op het werk (meestal al klaar).
    loop {
        if let Some(r) = uitkomst.lock().unwrap().take() {
            return Some(r);
        }
        if let Some(fout) = v.fout.lock().unwrap().take() {
            melding(eigenaar, &t("WerkMisluktKop"), &crate::teksten::tf("WerkMisluktTekst", &[kop, &fout]), ICOON_WAARSCHUWING);
            return None;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}
