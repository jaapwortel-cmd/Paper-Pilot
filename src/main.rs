// =====================================================================
//  Paper Pilot - brieven lezen en formulieren invullen, helemaal zelf.
//  Gemaakt door Jaap Wortel. Gratis en open source (GPL-3.0).
//
//  Werkt met NVDA, JAWS en Verteller: gewone Windows-onderdelen, alles met het
//  toetsenbord en met de muis, en een mooie opmaak die de schermlezer niet hindert.
//  Het plan staat in PLAN.md.
// =====================================================================

#![windows_subsystem = "windows"]

mod beeld;
mod controle_pdf;
mod controle_web;
mod controle_word;
mod controleren;
mod beeldmodel;
mod camera;
mod afdrukken;
mod agenda;
mod bijwerken;
mod dialogen;
mod envelop;
mod formaten;
mod formulier_pdf;
mod handtekening;
mod gegevens;
mod herkennen;
mod herinneringen;
mod hoofdvenster;
mod invulvenster;
mod http;
mod instellingen;
mod keuzevensters;
mod leesvenster;
mod lezen;
mod logboek;
mod melden;
mod pdf_maken;
mod plat;
mod post;
mod proef;
mod taken;
mod qr;
mod scanner;
mod schrijven;
mod tekstvenster;
mod versturen;
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
    /// Een document om meteen te openen ("Openen met" in de Verkenner).
    pub bestand: Option<String>,
}

fn main() {
    logboek::vang_fouten();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let waarde = |naam: &str| args.iter().position(|a| a == naam).and_then(|i| args.get(i + 1)).cloned();
    instellingen::laad();

    if args.iter().any(|a| a == "--herinneringen") {
        // Vanuit de geplande taak van Windows: alleen een venster als er vandaag iets te melden is.
        crate::teksten::zet_taal(&instellingen::get("taal").unwrap_or_else(|| "nl".into()));
        std::process::exit(herinneringen::vanuit_taak());
    }
    if let Some(verslag) = waarde("--proefstart") {
        std::process::exit(proef::proefstart(&verslag));
    }
    if let Some(bestand) = waarde("--proef-lezen") {
        let verslag = args.iter().position(|a| a == "--proef-lezen").and_then(|i| args.get(i + 2)).cloned().unwrap_or_else(|| "lezen.txt".into());
        std::process::exit(proef::proef_lezen(&bestand, &verslag));
    }
    if let Some(hfmap) = waarde("--proef-beeldmodel") {
        // --proef-beeldmodel <map op Hugging Face> <afbeelding> <verslag>
        let i = args.iter().position(|a| a == "--proef-beeldmodel").unwrap_or(0);
        let beeld = args.get(i + 2).cloned().unwrap_or_default();
        let verslag = args.get(i + 3).cloned().unwrap_or_else(|| "beeldmodel.txt".into());
        std::process::exit(proef::proef_beeldmodel(&hfmap, &beeld, &verslag));
    }
    if let Some(bron) = waarde("--proef-invullen") {
        let i = args.iter().position(|a| a == "--proef-invullen").unwrap_or(0);
        let doel = args.get(i + 2).cloned().unwrap_or_else(|| "ingevuld.pdf".into());
        let verslag = args.get(i + 3).cloned().unwrap_or_else(|| "invullen.txt".into());
        std::process::exit(proef::proef_invullen(&bron, &doel, &verslag));
    }
    if let Some(bestand) = waarde("--proef-plat") {
        let i = args.iter().position(|a| a == "--proef-plat").unwrap_or(0);
        let verslag = args.get(i + 2).cloned().unwrap_or_else(|| "plat.txt".into());
        std::process::exit(proef::proef_plat(&bestand, &verslag));
    }
    if let Some(bestand) = waarde("--proef-controleren") {
        // --proef-controleren <pdf, docx of html> <verslag>
        let i = args.iter().position(|a| a == "--proef-controleren").unwrap_or(0);
        let verslag = args.get(i + 2).cloned().unwrap_or_else(|| "controle.txt".into());
        std::process::exit(proef::proef_controleren(&bestand, &verslag));
    }
    if let Some(map) = waarde("--proef-camera") {
        let i = args.iter().position(|a| a == "--proef-camera").unwrap_or(0);
        let verslag = args.get(i + 2).cloned().unwrap_or_else(|| "camera.txt".into());
        std::process::exit(proef::proef_camera(&map, &verslag));
    }
    if let Some(bestand) = waarde("--proef-afspraken") {
        let i = args.iter().position(|a| a == "--proef-afspraken").unwrap_or(0);
        let verslag = args.get(i + 2).cloned().unwrap_or_else(|| "afspraken.txt".into());
        std::process::exit(proef::proef_afspraken(&bestand, &verslag));
    }
    if let Some(verslag) = waarde("--proef-herinneringen") {
        std::process::exit(proef::proef_herinneringen(&verslag));
    }
    if let Some(scan) = waarde("--proef-handtekening") {
        // --proef-handtekening <scan> <pdf> <doel> <verslag>
        let i = args.iter().position(|a| a == "--proef-handtekening").unwrap_or(0);
        let pdf = args.get(i + 2).cloned().unwrap_or_default();
        let doel = args.get(i + 3).cloned().unwrap_or_else(|| "getekend.pdf".into());
        let verslag = args.get(i + 4).cloned().unwrap_or_else(|| "handtekening.txt".into());
        std::process::exit(proef::proef_handtekening(&scan, &pdf, &doel, &verslag));
    }
    if let Some(verslag) = waarde("--proef-scanners") {
        std::process::exit(proef::proef_scanners(&verslag));
    }
    if let Some(verslag) = waarde("--proef-updates") {
        std::process::exit(proef::proef_updates(&verslag));
    }

    let opties = Opties {
        demo: args.iter().any(|a| a == "--demo"),
        taal: waarde("--taal"),
        thema: waarde("--thema"),
        schermafbeelding: waarde("--schermafbeelding"),
        bestand: args.iter().find(|a| !a.starts_with("--") && std::path::Path::new(a).is_file()).cloned(),
    };
    let proef = opties.demo || opties.schermafbeelding.is_some();

    // Maar één Paper Pilot tegelijk; het installatieprogramma kijkt ook naar deze naam.
    let naam = win::w("PaperPilotDraait");
    let mutex = unsafe { windows_sys::Win32::System::Threading::CreateMutexW(std::ptr::null(), 0, naam.as_ptr()) };
    if !proef && unsafe { windows_sys::Win32::Foundation::GetLastError() } == windows_sys::Win32::Foundation::ERROR_ALREADY_EXISTS {
        laat_ander_venster_zien(opties.bestand.as_deref());
        return;
    }
    if !proef {
        bijwerken::ruim_op();
    }
    let code = hoofdvenster::start(opties);
    let _ = mutex;
    std::process::exit(code);
}

/// Draait Paper Pilot al, dan dat venster naar voren halen (en het document daar openen).
fn laat_ander_venster_zien(bestand: Option<&str>) {
    use windows_sys::Win32::System::DataExchange::COPYDATASTRUCT;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    let klasse = win::w("PaperPilotHoofdvenster");
    unsafe {
        let h = FindWindowW(klasse.as_ptr(), std::ptr::null());
        if !h.is_null() {
            if IsIconic(h) != 0 {
                ShowWindow(h, SW_RESTORE);
            }
            SetForegroundWindow(h);
            if let Some(pad) = bestand {
                let tekst: Vec<u16> = pad.encode_utf16().collect();
                let cds = COPYDATASTRUCT { dwData: 1, cbData: (tekst.len() * 2) as u32, lpData: tekst.as_ptr() as *mut _ };
                SendMessageW(h, WM_COPYDATA, 0, &cds as *const _ as isize);
            }
        }
    }
}
