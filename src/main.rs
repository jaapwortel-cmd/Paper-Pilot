// =====================================================================
//  Paper Pilot - brieven lezen en formulieren invullen, helemaal zelf.
//  Gemaakt door Jaap Wortel. Gratis en open source (GPL-3.0).
//
//  Werkt met NVDA, JAWS en Verteller: gewone Windows-onderdelen, alles met het
//  toetsenbord en met de muis, en een mooie opmaak die de schermlezer niet hindert.
//  Het plan staat in PLAN.md.
// =====================================================================

#![windows_subsystem = "windows"]

mod bijwerken;
mod dialogen;
mod hoofdvenster;
mod instellingen;
mod logboek;
mod proef;
mod tekstvenster;
mod teksten;
mod thema;
mod win;

pub const VERSIE: &str = env!("PP_VERSIE");
pub const MAIL: &str = "jaap.wortel@gmail.com";

/// Opties vanaf de opdrachtregel (vooral voor de proeven op GitHub).
#[derive(Clone, Default)]
pub struct Opties {
    /// Geen rondleiding en geen updates (voor proeven en demo's).
    pub demo: bool,
    pub taal: Option<String>,
    pub thema: Option<String>,
    pub schermafbeelding: Option<String>,
}

fn main() {
    logboek::vang_fouten();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let waarde = |naam: &str| args.iter().position(|a| a == naam).and_then(|i| args.get(i + 1)).cloned();
    instellingen::laad();

    if let Some(verslag) = waarde("--proefstart") {
        std::process::exit(proef::proefstart(&verslag));
    }
    if let Some(verslag) = waarde("--proef-updates") {
        std::process::exit(proef::proef_updates(&verslag));
    }

    let opties = Opties {
        demo: args.iter().any(|a| a == "--demo"),
        taal: waarde("--taal"),
        thema: waarde("--thema"),
        schermafbeelding: waarde("--schermafbeelding"),
    };
    let proef = opties.demo || opties.schermafbeelding.is_some();

    // Maar één Paper Pilot tegelijk; het installatieprogramma kijkt ook naar deze naam.
    let naam = win::w("PaperPilotDraait");
    let mutex = unsafe { windows_sys::Win32::System::Threading::CreateMutexW(std::ptr::null(), 0, naam.as_ptr()) };
    if !proef && unsafe { windows_sys::Win32::Foundation::GetLastError() } == windows_sys::Win32::Foundation::ERROR_ALREADY_EXISTS {
        laat_ander_venster_zien();
        return;
    }
    if !proef {
        bijwerken::ruim_op();
    }
    let code = hoofdvenster::start(opties);
    let _ = mutex;
    std::process::exit(code);
}

/// Draait Paper Pilot al, dan dat venster naar voren halen.
fn laat_ander_venster_zien() {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    let klasse = win::w("PaperPilotHoofdvenster");
    unsafe {
        let h = FindWindowW(klasse.as_ptr(), std::ptr::null());
        if !h.is_null() {
            if IsIconic(h) != 0 {
                ShowWindow(h, SW_RESTORE);
            }
            SetForegroundWindow(h);
        }
    }
}
