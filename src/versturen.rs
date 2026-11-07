// Paper Pilot - een document mailen als bijlage.
//
// Eerst via Simple MAPI (MAPISendMailW): dat opent een nieuwe mail met de bijlage in het eigen mailprogramma
// (bijvoorbeeld Outlook classic of Thunderbird). Heeft het mailprogramma geen MAPI (zoals de nieuwe Outlook of Mail
// van Windows), dan zet Paper Pilot het bestand op het klembord en opent een nieuwe mail: plakken met Ctrl+V.

use crate::win::w;
use std::path::Path;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::System::Mapi::*;

pub enum Uitkomst {
    /// Het mailprogramma heeft de nieuwe mail geopend (en is weer dicht).
    Mapi,
    /// Het bestand staat op het klembord en er is een nieuwe mail geopend.
    Klembord,
    Mislukt,
}

fn mapi(eigenaar: HWND, pad: &Path, onderwerp: &str) -> Result<(), u32> {
    use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
    unsafe {
        let dll = w("mapi32.dll");
        let h = LoadLibraryW(dll.as_ptr());
        if h.is_null() {
            return Err(1);
        }
        let Some(f) = GetProcAddress(h, c"MAPISendMailW".as_ptr() as *const u8) else { return Err(2) };
        let f: unsafe extern "system" fn(usize, usize, *const MapiMessageW, u32, u32) -> u32 = std::mem::transmute(f);
        let mut volledig = w(&pad.to_string_lossy());
        let mut naam = w(&pad.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default());
        let mut onderwerp = w(onderwerp);
        let mut bijlage = MapiFileDescW { nPosition: u32::MAX, lpszPathName: volledig.as_mut_ptr(), lpszFileName: naam.as_mut_ptr(), ..Default::default() };
        let bericht = MapiMessageW { lpszSubject: onderwerp.as_mut_ptr(), nFileCount: 1, lpFiles: &mut bijlage, ..Default::default() };
        let r = f(0, eigenaar as usize, &bericht, MAPI_DIALOG | MAPI_LOGON_UI, 0);
        // 0 = gelukt, 1 = de gebruiker heeft de mail weggegooid (ook goed).
        if r == 0 || r == 1 { Ok(()) } else { Err(r) }
    }
}

/// Mailt het bestand. De gebruiker kiest zelf aan wie, in het eigen mailprogramma.
pub fn mail(eigenaar: HWND, pad: &Path, onderwerp: &str) -> Uitkomst {
    match mapi(eigenaar, pad, onderwerp) {
        Ok(()) => {
            crate::logboek::schrijf("Mailen via MAPI gelukt.");
            Uitkomst::Mapi
        }
        Err(code) => {
            crate::logboek::schrijf(&format!("Mailen via MAPI lukte niet ({code}); bestand op het klembord."));
            if !crate::win::kopieer_bestand(eigenaar, pad) {
                return Uitkomst::Mislukt;
            }
            crate::win::open_adres(&format!("mailto:?subject={}", crate::win::adrescode(onderwerp)));
            Uitkomst::Klembord
        }
    }
}

/// Mailen met de meldingen erbij (voor het leesvenster en het invulvenster).
pub fn mail_met_uitleg(eigenaar: HWND, pad: &Path, onderwerp: &str) {
    use crate::teksten::{t, tf};
    let naam = pad.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    match mail(eigenaar, pad, onderwerp) {
        Uitkomst::Mapi => {}
        Uitkomst::Klembord => crate::dialogen::melding(eigenaar, &t("MailKlembord"), &tf("MailKlembordTekst", &[&naam]), crate::dialogen::ICOON_INFO),
        Uitkomst::Mislukt => crate::dialogen::melding(eigenaar, &t("MailMislukt"), &tf("MailMisluktTekst", &[&pad.to_string_lossy()]), crate::dialogen::ICOON_WAARSCHUWING),
    }
}
