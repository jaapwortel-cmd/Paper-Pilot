// Paper Pilot - scanners zoeken en scannen.
//
// Twee soorten:
// 1. Scanners die Windows kent (USB, netwerk via WSD, de scanner in een printer): de scanner-onderdelen van Windows
//    (Windows.Devices.Scanners, gebouwd op WIA). Werkt met bijna elke scanner met een stuurprogramma.
// 2. Netwerkprinters met een scanner die Windows nog niet kent: eSCL (ook AirScan genoemd). Paper Pilot zoekt ze via de
//    zoekdienst van Windows (DNS-SD) en met een eigen mDNS-vraag op elke netwerkkaart ("_uscan._tcp"), en vraagt elke
//    printer in Windows en in het netwerk of er een eSCL-scanner in zit. Scannen gaat via HTTP, zonder stuurprogramma.
// Paper Pilot scant op 300 dpi in grijs (goed voor tekstherkenning), van de glasplaat of de documentinvoer.

use crate::beeld::Beeld;
use std::net::{Ipv4Addr, UdpSocket};
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};
use windows::core::{Interface, HSTRING};
use windows::Devices::Enumeration::{DeviceInformation, DeviceInformationKind};
use windows_collections::IIterable;
use windows::Foundation::IPropertyValue;
use windows::Devices::Scanners::*;
use windows::Storage::StorageFolder;

#[derive(Clone)]
pub enum Soort {
    Windows(String),
    /// Alleen bekend bij de klassieke WIA (DeviceID).
    Wia(String),
    /// Basisadres, bijvoorbeeld http://192.168.1.20:80/eSCL
    Escl(String),
}

#[derive(Clone)]
pub struct Scanner {
    pub naam: String,
    pub soort: Soort,
    pub glasplaat: bool,
    pub invoer: bool,
    /// Dezelfde scanner via het netwerk (eSCL), om te proberen als scannen via Windows niet lukt.
    pub reserve: Option<String>,
    /// Dezelfde scanner in de klassieke WIA (DeviceID), om te proberen als de nieuwe weg niet lukt.
    pub wia: Option<String>,
}

impl Scanner {
    /// Om de standaardscanner te bewaren in de instellingen.
    pub fn naar_tekst(&self) -> String {
        let (soort, id) = match &self.soort {
            Soort::Windows(i) => ("windows", i.clone()),
            Soort::Wia(i) => ("wia", i.clone()),
            Soort::Escl(i) => ("escl", i.clone()),
        };
        serde_json::json!({ "naam": self.naam, "soort": soort, "id": id, "glasplaat": self.glasplaat, "invoer": self.invoer,
            "reserve": self.reserve, "wia": self.wia }).to_string()
    }

    pub fn van_tekst(t: &str) -> Option<Scanner> {
        let v: serde_json::Value = serde_json::from_str(t).ok()?;
        let id = v["id"].as_str()?.to_string();
        let soort = match v["soort"].as_str()? {
            "windows" => Soort::Windows(id),
            "wia" => Soort::Wia(id),
            "escl" => Soort::Escl(id),
            _ => return None,
        };
        Some(Scanner {
            naam: v["naam"].as_str()?.to_string(),
            soort,
            glasplaat: v["glasplaat"].as_bool().unwrap_or(true),
            invoer: v["invoer"].as_bool().unwrap_or(false),
            reserve: v["reserve"].as_str().map(String::from),
            wia: v["wia"].as_str().map(String::from),
        })
    }
}

/// De standaardscanner (gekozen bij de eerste start of in Instellingen).
pub fn standaard() -> Option<Scanner> {
    crate::instellingen::get("standaard_scanner").and_then(|t| Scanner::van_tekst(&t))
}

pub fn zet_standaard(s: &Scanner) {
    crate::instellingen::zet("standaard_scanner", &s.naar_tekst());
    crate::instellingen::zet("scanner", &s.naam);
    crate::logboek::schrijf(&format!("Standaardscanner: {}", s.naam));
}

/// Wat het zoeken opleverde.
pub struct Zoekuitkomst {
    pub scanners: Vec<Scanner>,
    /// Printers en netwerkapparaten die Paper Pilot wel zag, maar waarmee hij (nog) niet kan scannen.
    pub andere: Vec<String>,
    /// Alle echte printers in Windows (geen PDF-, XPS- of faxprinters).
    pub printers: Vec<String>,
}

/// Een eSCL-scanner in het netwerk: naam, basisadres en het IP-adres.
#[derive(Clone)]
struct Escl {
    naam: String,
    basis: String,
    host: String,
}

/// Een printer of netwerkapparaat dat misschien een scanner heeft.
struct Apparaat {
    naam: String,
    ips: Vec<Ipv4Addr>,
}

/// Werk op een eigen draad, zodat alle zoektochten tegelijk lopen.
fn op_achtergrond<T: Send + 'static>(taak: impl FnOnce() -> T + Send + 'static) -> Receiver<T> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        crate::beeld::com_start();
        let _ = tx.send(taak());
    });
    rx
}

/// Wacht op werk van `op_achtergrond`, maar niet langer dan tot `tot`.
fn wacht<T>(rx: &Receiver<T>, tot: Instant, wat: &str) -> Option<T> {
    match rx.recv_timeout(tot.saturating_duration_since(Instant::now())) {
        Ok(v) => Some(v),
        Err(_) => {
            crate::logboek::schrijf(&format!("Scanners zoeken: {wat} duurde te lang."));
            None
        }
    }
}

/// Zoekt alle scanners: aan de computer (USB), in Windows bekend, en in het netwerk (duurt hooguit zo'n 15 seconden).
pub fn zoek() -> Zoekuitkomst {
    crate::beeld::com_start();
    let begin = Instant::now();
    // 1. Scanners die Windows kent (met een stuurprogramma, USB of netwerk).
    let wia = op_achtergrond(|| windows_scanners().map_err(|e| e.to_string()));
    // 1b. De klassieke WIA (zoals Windows Faxen en scannen): ziet soms scanners die de nieuwe weg mist.
    let klassiek = op_achtergrond(crate::wia::scanners);
    // 2. Netwerkscanners via de zoekdienst van Windows zelf (mDNS/Bonjour) en via een eigen mDNS-vraag op elke netwerkkaart.
    let diensten: Vec<_> = ["_uscan._tcp", "_uscans._tcp", "_ipp._tcp", "_ipps._tcp", "_pdl-datastream._tcp", "_printer._tcp"]
        .iter().map(|d| (d.to_string(), op_achtergrond(move || dnssd_windows(d)))).collect();
    let eigen = op_achtergrond(mdns_zoek);
    // 3. De printers die in Windows staan (daar zit vaak een scanner in).
    let printers = op_achtergrond(printers);

    let mut escl: Vec<Escl> = Vec::new();
    let mut apparaten: Vec<Apparaat> = Vec::new();
    let netwerk_klaar = begin + Duration::from_secs(8);
    for (soort, rx) in &diensten {
        let Some(lijst) = wacht(rx, netwerk_klaar, soort) else { continue };
        for d in lijst {
            if soort.starts_with("_uscan") {
                if let Some(ip) = d.ips.first() {
                    let rs = d.rs.trim_matches('/');
                    let rs = if rs.is_empty() { "eSCL" } else { rs };
                    let veilig = soort == "_uscans._tcp";
                    let poort = if d.poort != 0 { d.poort } else if veilig { 443 } else { 80 };
                    escl.push(Escl { naam: d.naam.clone(), basis: format!("{}://{ip}:{poort}/{rs}", if veilig { "https" } else { "http" }), host: ip.to_string() });
                }
            } else {
                apparaten.push(Apparaat { naam: d.naam, ips: d.ips });
            }
        }
    }
    if let Some(l) = wacht(&eigen, begin + Duration::from_secs(6), "mDNS") {
        escl.extend(l);
    }
    let mut printernamen = Vec::new();
    if let Some(l) = wacht(&printers, begin + Duration::from_secs(8), "printers") {
        for p in l {
            printernamen.push(p.naam.clone());
            apparaten.push(p);
        }
    }
    // Per apparaat één eSCL-adres; gewone http heeft voorrang boven https.
    escl.sort_by_key(|e| e.basis.starts_with("https"));
    let mut gezien = std::collections::HashSet::new();
    escl.retain(|e| gezien.insert(e.host.clone()));

    // 4. Printers en apparaten met een IP-adres zonder bekende eSCL-scanner: zelf vragen of er een scanner in zit.
    let mut proeven = Vec::new();
    for a in &apparaten {
        for ip in &a.ips {
            if gezien.insert(ip.to_string()) {
                let (ip, naam) = (*ip, a.naam.clone());
                proeven.push(op_achtergrond(move || escl_proef(ip).map(|basis| Escl { naam, basis, host: ip.to_string() })));
            }
        }
    }
    let proef_klaar = Instant::now() + Duration::from_secs(8);
    for rx in &proeven {
        if let Some(Some(e)) = wacht(rx, proef_klaar, "eSCL-proef") {
            escl.push(e);
        }
    }

    // 5. Alles samen. Wat Windows kent eerst; dezelfde scanner niet twee keer.
    let mut lijst = match wacht(&wia, begin + Duration::from_secs(20), "scanners van Windows") {
        Some(Ok(l)) => l,
        Some(Err(e)) => {
            crate::logboek::schrijf(&format!("Scanners van Windows zoeken: {e}"));
            Vec::new()
        }
        None => Vec::new(),
    };
    match wacht(&klassiek, begin + Duration::from_secs(25), "WIA") {
        Some(Ok(l)) => {
            for w in l {
                let kort = w.naam.to_lowercase();
                if let Some(s) = lijst.iter_mut().find(|s| zelfde(&s.naam, &kort)) {
                    s.wia = Some(w.id);
                } else {
                    lijst.push(Scanner { naam: w.naam, soort: Soort::Wia(w.id), glasplaat: w.glasplaat, invoer: w.invoer, reserve: None, wia: None });
                }
            }
        }
        Some(Err(e)) => crate::logboek::schrijf(&format!("Scanners zoeken via WIA: {e}")),
        None => {}
    }
    let mogelijkheden: Vec<_> = escl.iter().map(|e| { let b = e.basis.clone(); op_achtergrond(move || escl_mogelijkheden(&b)) }).collect();
    let klaar = Instant::now() + Duration::from_secs(6);
    for (e, rx) in escl.iter().zip(&mogelijkheden) {
        let (glas, invoer) = wacht(rx, klaar, "ScannerCapabilities").unwrap_or((true, false));
        let kort = e.naam.to_lowercase();
        if let Some(w) = lijst.iter_mut().find(|w| matches!(w.soort, Soort::Windows(_) | Soort::Wia(_)) && zelfde(&w.naam, &kort)) {
            // Windows kent hem al: het netwerkadres bewaren als reserve.
            if w.reserve.is_none() {
                w.reserve = Some(e.basis.clone());
            }
            continue;
        }
        if lijst.iter().any(|w: &Scanner| zelfde(&w.naam, &kort)) {
            continue;
        }
        lijst.push(Scanner { naam: format!("{} ({})", e.naam, crate::teksten::t("ScannerNetwerk")), soort: Soort::Escl(e.basis.clone()), glasplaat: glas, invoer, reserve: None, wia: None });
    }

    // Printers zonder scanner die Paper Pilot kan gebruiken: die noemt Paper Pilot, zodat je weet wat hij wel zag.
    let mut andere: Vec<String> = Vec::new();
    for a in &apparaten {
        let kort = a.naam.to_lowercase();
        if virtueel(&kort) || a.naam.is_empty() || lijst.iter().any(|s| zelfde(&s.naam, &kort)) || andere.iter().any(|x| zelfde(x, &kort)) {
            continue;
        }
        andere.push(a.naam.clone());
    }
    crate::logboek::schrijf(&format!("Scanners gevonden: {} | printers in Windows: {} | eSCL in het netwerk: {} | zonder scanner: {} ({} ms)",
        lijst.iter().map(|s| s.naam.clone()).collect::<Vec<_>>().join(", "), printernamen.join(", "),
        escl.iter().map(|e| e.basis.clone()).collect::<Vec<_>>().join(", "), andere.join(", "), begin.elapsed().as_millis()));
    let printers: Vec<String> = printernamen.iter().filter(|n| !virtueel(n)).cloned().collect();
    Zoekuitkomst { scanners: lijst, andere, printers }
}

/// Een printer die alleen een bestand maakt (PDF, XPS, OneNote) of faxt.
fn virtueel(naam: &str) -> bool {
    let kort = naam.to_lowercase();
    ["pdf", "xps", "fax", "onenote"].iter().any(|v| kort.contains(v))
}

/// Lijken twee namen op hetzelfde apparaat? ("Brother MFC-L2710DW series" en "Brother MFC-L2710DW")
fn zelfde(a: &str, kort_b: &str) -> bool {
    let a = a.to_lowercase();
    let schoon = |s: &str| s.replace(" series", "").replace(" (netwerk)", "").replace(" (network)", "").trim().to_string();
    let (a, b) = (schoon(&a), schoon(kort_b));
    !a.is_empty() && !b.is_empty() && (a.contains(&b) || b.contains(&a))
}

fn windows_scanners() -> windows::core::Result<Vec<Scanner>> {
    let mut uit = Vec::new();
    let kiezer = ImageScanner::GetDeviceSelector()?;
    let op = DeviceInformation::FindAllAsyncAqsFilter(&kiezer)?;
    klaar(&op, Duration::from_secs(15))?;
    let apparaten = op.GetResults()?;
    for d in apparaten {
        let naam = d.Name()?.to_string();
        let id = d.Id()?.to_string();
        let scanner = ImageScanner::FromIdAsync(&HSTRING::from(&id)).and_then(|op| klaar(&op, Duration::from_secs(15)).and_then(|_| op.GetResults()));
        if let Err(e) = &scanner {
            crate::logboek::schrijf(&format!("Scanner {naam} openen: {:#010x} {}", e.code().0 as u32, e.message()));
        }
        let (glas, invoer) = match scanner {
            Ok(s) => (
                s.IsScanSourceSupported(ImageScannerScanSource::Flatbed).unwrap_or(false),
                s.IsScanSourceSupported(ImageScannerScanSource::Feeder).unwrap_or(false),
            ),
            Err(_) => (true, false),
        };
        uit.push(Scanner { naam, soort: Soort::Windows(id), glasplaat: glas || !invoer, invoer, reserve: None, wia: None });
    }
    Ok(uit)
}

/// Wacht tot een opdracht van Windows (async) klaar is, en geeft bij een fout de ECHTE fout van het apparaat.
/// (`join()` van de windows-crate vraagt bij een fout toch de uitkomst op; Windows antwoordt dan alleen
/// "asynchrone bewerking niet correct gestart" (0x80000019) en de echte fout van de scanner gaat verloren.)
fn klaar<I: Interface>(op: &I, max: Duration) -> windows::core::Result<()> {
    use windows_future::{AsyncStatus, IAsyncInfo};
    let info: IAsyncInfo = op.cast()?;
    let begin = Instant::now();
    loop {
        let status = info.Status()?;
        if status == AsyncStatus::Started {
            if begin.elapsed() > max {
                let _ = info.Cancel();
                // ERROR_TIMEOUT
                return Err(windows::core::Error::from_hresult(windows::core::HRESULT(0x800705B4u32 as i32)));
            }
            std::thread::sleep(Duration::from_millis(100));
        } else if status == AsyncStatus::Completed {
            return Ok(());
        } else if status == AsyncStatus::Canceled {
            // ERROR_CANCELLED
            return Err(windows::core::Error::from_hresult(windows::core::HRESULT(0x800704C7u32 as i32)));
        } else {
            let code = info.ErrorCode()?;
            return Err(windows::core::Error::from_hresult(code));
        }
    }
}

/// Een fout van de scanner in gewone taal (met de code erachter, voor het logboek en de maker).
fn uitleg(e: &windows::core::Error) -> String {
    let code = e.code().0 as u32;
    let sleutel = match code {
        0x80210002 => "ScanFoutVastgelopen",
        0x80210003 => "ScanFoutGeenPapier",
        0x80210004 | 0x80210065 => "ScanFoutPapier",
        0x80210005 | 0x80210015 | 0x8021000A => "ScanFoutNietBereikbaar",
        0x80210006 | 0x8021000D => "ScanFoutBezig",
        0x80210007 | 0x80210016 => "ScanFoutOpwarmen",
        0x80210008 => "ScanFoutWacht",
        0x80210064 => "ScanFoutKlep",
        0x800705B4 => "ScanFoutDuurtTeLang",
        0x800704C7 => "ScanFoutGeannuleerd",
        _ => "ScanFoutAlgemeen",
    };
    format!("{} (code {code:#010x}: {})", crate::teksten::t(sleutel), e.message())
}

/// Een WIA-fout (tekst met "0x8021....") in gewone taal.
fn wia_uitleg(e: &str) -> String {
    let code = e.split_whitespace().find_map(|w| w.trim_start_matches(|c: char| !c.is_ascii_digit()).strip_prefix("0x")
        .and_then(|h| u32::from_str_radix(&h[..h.len().min(8)], 16).ok())).unwrap_or(0x80004005);
    let fout = windows::core::Error::from_hresult(windows::core::HRESULT(code as i32));
    let mut u = uitleg(&fout);
    if code == 0x80004005 {
        u = format!("{} ({e})", crate::teksten::t("ScanFoutAlgemeen"));
    }
    u
}

/// Scant. Met `invoer` alle pagina's uit de documentinvoer, anders één pagina van de glasplaat.
/// Lukt het niet, dan probeert Paper Pilot het nog eens met de standaardinstellingen van de scanner,
/// en daarna via het netwerk (eSCL) als dezelfde scanner daar ook te vinden is.
pub fn scan(s: &Scanner, invoer: bool, dpi: u32) -> Result<Vec<Beeld>, String> {
    crate::beeld::com_start();
    let uitkomst = match &s.soort {
        Soort::Windows(id) => {
            let mut r = windows_scan(id, invoer, dpi, true);
            if let Err(e) = &r {
                crate::logboek::schrijf(&format!("Scannen met {} (eigen instellingen) lukte niet: {:#010x} {}", s.naam, e.code().0 as u32, e.message()));
                // Geen papier, klep open of zelf geannuleerd: nog eens proberen heeft geen zin.
                let opnieuw = !matches!(e.code().0 as u32, 0x80210003 | 0x80210064 | 0x800704C7 | 0x80210002);
                // Eerst de klassieke WIA (werkt met vrijwel elk stuurprogramma).
                if opnieuw {
                    if let Some(wid) = &s.wia {
                        match crate::wia::scan(wid, invoer, dpi) {
                            Ok(b) => {
                                crate::logboek::schrijf(&format!("Gescand met {} via de klassieke WIA.", s.naam));
                                return Ok(b);
                            }
                            Err(e) => crate::logboek::schrijf(&format!("Scannen met {} via de klassieke WIA lukte niet: {e}", s.naam)),
                        }
                    }
                }
                if opnieuw {
                    r = windows_scan(id, invoer, dpi, false);
                    if let Err(e2) = &r {
                        crate::logboek::schrijf(&format!("Scannen met {} (standaardinstellingen) lukte niet: {:#010x} {}", s.naam, e2.code().0 as u32, e2.message()));
                    }
                }
            }
            match (r, &s.reserve) {
                (Ok(b), _) => Ok(b),
                (Err(e), Some(basis)) => {
                    crate::logboek::schrijf(&format!("Scannen met {}: nu via het netwerk ({basis}).", s.naam));
                    escl_scan(basis, invoer, dpi).map_err(|e2| {
                        crate::logboek::schrijf(&format!("Scannen via het netwerk lukte ook niet: {e2}"));
                        uitleg(&e)
                    })
                }
                (Err(e), None) => Err(uitleg(&e)),
            }
        }
        Soort::Wia(id) => match crate::wia::scan(id, invoer, dpi) {
            Ok(b) => Ok(b),
            Err(e) => {
                crate::logboek::schrijf(&format!("Scannen met {} via WIA lukte niet: {e}", s.naam));
                match &s.reserve {
                    Some(basis) => escl_scan(basis, invoer, dpi).map_err(|_| wia_uitleg(&e)),
                    None => Err(wia_uitleg(&e)),
                }
            }
        },
        Soort::Escl(basis) => escl_scan(basis, invoer, dpi).map_err(|e| format!("{} ({e})", crate::teksten::t("ScanFoutAlgemeen"))),
    };
    match &uitkomst {
        Ok(b) => crate::logboek::schrijf(&format!("Gescand met {}: {} pagina('s), {dpi} dpi, {}.", s.naam, b.len(), if invoer { "documentinvoer" } else { "glasplaat" })),
        Err(e) => crate::logboek::schrijf(&format!("Scannen met {}: {e}", s.naam)),
    }
    uitkomst
}

/// Scant via Windows. `eigen`: Paper Pilot kiest resolutie, grijs en PNG; anders de standaard van de scanner.
fn windows_scan(id: &str, invoer: bool, dpi: u32, eigen: bool) -> windows::core::Result<Vec<Beeld>> {
    let op = ImageScanner::FromIdAsync(&HSTRING::from(id))?;
    klaar(&op, Duration::from_secs(60))?;
    let scanner = op.GetResults()?;
    let dpi = ImageScannerResolution { DpiX: dpi as f32, DpiY: dpi as f32 };
    let glas = scanner.IsScanSourceSupported(ImageScannerScanSource::Flatbed).unwrap_or(false);
    let bron = if invoer {
        if eigen {
            let c = scanner.FeederConfiguration()?;
            let _ = c.SetDesiredResolution(dpi);
            if c.IsColorModeSupported(ImageScannerColorMode::Grayscale).unwrap_or(false) {
                let _ = c.SetColorMode(ImageScannerColorMode::Grayscale);
            }
            if c.IsFormatSupported(ImageScannerFormat::Png).unwrap_or(false) {
                let _ = c.SetFormat(ImageScannerFormat::Png);
            }
            let _ = c.SetMaxNumberOfPages(0);
        }
        ImageScannerScanSource::Feeder
    } else if glas {
        if eigen {
            let c = scanner.FlatbedConfiguration()?;
            let _ = c.SetDesiredResolution(dpi);
            if c.IsColorModeSupported(ImageScannerColorMode::Grayscale).unwrap_or(false) {
                let _ = c.SetColorMode(ImageScannerColorMode::Grayscale);
            }
            if c.IsFormatSupported(ImageScannerFormat::Png).unwrap_or(false) {
                let _ = c.SetFormat(ImageScannerFormat::Png);
            }
        }
        ImageScannerScanSource::Flatbed
    } else {
        ImageScannerScanSource::Default
    };
    let bron = if !eigen && !invoer { ImageScannerScanSource::Default } else { bron };
    crate::logboek::schrijf(&format!("Scannen via Windows: bron {}, {}.", bron.0, if eigen { "eigen instellingen" } else { "standaardinstellingen" }));
    let map = std::env::temp_dir().join("Paper Pilot").join(format!("scan-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&map);
    let _ = std::fs::create_dir_all(&map);
    let op = StorageFolder::GetFolderFromPathAsync(&HSTRING::from(map.as_os_str()))?;
    klaar(&op, Duration::from_secs(30))?;
    let folder = op.GetResults()?;
    let op = scanner.ScanFilesToFolderAsync(bron, &folder)?;
    klaar(&op, Duration::from_secs(600))?;
    let uitkomst = op.GetResults()?;
    let mut beelden = Vec::new();
    for bestand in uitkomst.ScannedFiles()? {
        let pad = std::path::PathBuf::from(bestand.Path()?.to_string());
        match crate::beeld::lees(&pad) {
            Ok(b) => beelden.extend(b),
            Err(e) => crate::logboek::schrijf(&format!("Gescande pagina {} kon niet gelezen worden: {e}", pad.display())),
        }
        let _ = std::fs::remove_file(&pad);
    }
    let _ = std::fs::remove_dir_all(&map);
    Ok(beelden)
}

// ------------------------------------------------------------------ eSCL (AirScan)

/// Een DNS-naam lezen (met verwijzingen), vanaf `i`. Geeft de naam en de plek erna.
fn dns_naam(p: &[u8], mut i: usize) -> Option<(String, usize)> {
    let mut delen = Vec::new();
    let mut eind = None;
    let mut sprongen = 0;
    loop {
        let len = *p.get(i)? as usize;
        if len == 0 {
            i += 1;
            break;
        }
        if len & 0xC0 == 0xC0 {
            let doel = ((len & 0x3F) << 8) | *p.get(i + 1)? as usize;
            if eind.is_none() {
                eind = Some(i + 2);
            }
            sprongen += 1;
            if sprongen > 20 {
                return None;
            }
            i = doel;
            continue;
        }
        delen.push(String::from_utf8_lossy(p.get(i + 1..i + 1 + len)?).to_string());
        i += 1 + len;
    }
    Some((delen.join("."), eind.unwrap_or(i)))
}

#[derive(Default, Clone)]
struct Dienst {
    poort: u16,
    doel: String,
    rs: String,
    ty: String,
    ip: Option<Ipv4Addr>,
    https: bool,
}

/// Een dienst die de zoekdienst van Windows vond (DNS-SD).
#[derive(Default)]
struct DnsDienst {
    naam: String,
    poort: u16,
    rs: String,
    ips: Vec<Ipv4Addr>,
}

/// Zoekt met de zoekdienst van Windows naar een soort dienst in het netwerk (bijvoorbeeld "_uscan._tcp").
/// Dit gaat via Windows zelf, dus de firewall houdt de antwoorden niet tegen.
fn dnssd_windows(dienst: &str) -> Vec<DnsDienst> {
    let mut uit = Vec::new();
    let r = (|| -> windows::core::Result<()> {
        let filter = HSTRING::from(format!(
            "System.Devices.AepService.ProtocolId:=\"{{4526e8c1-8aac-4153-9b16-55e86ada0e54}}\" AND System.Devices.Dnssd.Domain:=\"local\" AND System.Devices.Dnssd.ServiceName:=\"{dienst}\""));
        let namen = ["System.Devices.Dnssd.InstanceName", "System.Devices.Dnssd.PortNumber", "System.Devices.Dnssd.TextAttributes", "System.Devices.IpAddress"];
        let eigenschappen: IIterable<HSTRING> = IIterable::from(namen.iter().map(|n| HSTRING::from(*n)).collect::<Vec<_>>());
        let op = DeviceInformation::FindAllAsyncWithKindAqsFilterAndAdditionalProperties(&filter, &eigenschappen, DeviceInformationKind::AssociationEndpointService)?;
        klaar(&op, Duration::from_secs(8))?;
        let lijst = op.GetResults()?;
        for d in lijst {
            let p = d.Properties()?;
            let tekst = |n: &str| -> Vec<String> {
                let Ok(v) = p.Lookup(&HSTRING::from(n)) else { return Vec::new() };
                let Ok(v) = v.cast::<IPropertyValue>() else { return Vec::new() };
                if let Ok(s) = v.GetString() {
                    return vec![s.to_string()];
                }
                let mut a = windows::core::Array::<HSTRING>::new();
                if v.GetStringArray(&mut a).is_ok() {
                    return a.iter().map(|s| s.to_string()).collect();
                }
                Vec::new()
            };
            let mut dd = DnsDienst { naam: d.Name().map(|n| n.to_string()).unwrap_or_default(), ..Default::default() };
            if let Some(n) = tekst("System.Devices.Dnssd.InstanceName").into_iter().next() {
                if !n.is_empty() {
                    dd.naam = n;
                }
            }
            if let Ok(v) = p.Lookup(&HSTRING::from("System.Devices.Dnssd.PortNumber")).and_then(|v| v.cast::<IPropertyValue>()) {
                dd.poort = v.GetUInt16().or_else(|_| v.GetUInt32().map(|x| x as u16)).unwrap_or(0);
            }
            for t in tekst("System.Devices.Dnssd.TextAttributes") {
                if let Some(v) = t.strip_prefix("rs=") {
                    dd.rs = v.to_string();
                } else if let Some(v) = t.strip_prefix("ty=") {
                    if !v.is_empty() {
                        dd.naam = v.to_string();
                    }
                }
            }
            dd.ips = tekst("System.Devices.IpAddress").iter().filter_map(|s| s.parse().ok()).collect();
            uit.push(dd);
        }
        Ok(())
    })();
    if let Err(e) = r {
        crate::logboek::schrijf(&format!("Zoeken naar {dienst} via Windows: {e}"));
    }
    uit
}

/// De IPv4-adressen van de eigen netwerkkaarten (die aan staan).
fn eigen_adressen() -> Vec<Ipv4Addr> {
    use windows_sys::Win32::NetworkManagement::IpHelper::*;
    use windows_sys::Win32::Networking::WinSock::{AF_INET, SOCKADDR_IN};
    let mut uit = Vec::new();
    let mut grootte = 32 * 1024u32;
    let mut buf: Vec<u64> = vec![0; grootte as usize / 8];
    unsafe {
        let mut r = GetAdaptersAddresses(AF_INET as u32, GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER,
            std::ptr::null(), buf.as_mut_ptr() as *mut _, &mut grootte);
        if r == 111 {
            // ERROR_BUFFER_OVERFLOW: groter maken en nog eens.
            buf = vec![0; grootte as usize / 8 + 1];
            r = GetAdaptersAddresses(AF_INET as u32, GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER,
                std::ptr::null(), buf.as_mut_ptr() as *mut _, &mut grootte);
        }
        if r != 0 {
            return uit;
        }
        let mut a = buf.as_ptr() as *const IP_ADAPTER_ADDRESSES_LH;
        while !a.is_null() {
            // IfOperStatusUp = 1
            if (*a).OperStatus == 1 {
                let mut u = (*a).FirstUnicastAddress;
                while !u.is_null() {
                    let sa = (*u).Address.lpSockaddr;
                    if !sa.is_null() && (*sa).sa_family == AF_INET {
                        let s = &*(sa as *const SOCKADDR_IN);
                        let ip = Ipv4Addr::from(u32::from_be(s.sin_addr.S_un.S_addr));
                        if !ip.is_loopback() && !ip.is_link_local() {
                            uit.push(ip);
                        }
                    }
                    u = (*u).Next;
                }
            }
            a = (*a).Next;
        }
    }
    uit
}

/// Zoekt eSCL-scanners met een eigen mDNS-vraag, op elke netwerkkaart (2,5 seconden).
fn mdns_zoek() -> Vec<Escl> {
    let mut kaarten: Vec<Option<Ipv4Addr>> = eigen_adressen().into_iter().map(Some).collect();
    if kaarten.is_empty() {
        kaarten.push(None);
    }
    let draden: Vec<_> = kaarten.into_iter().map(|k| std::thread::spawn(move || mdns_op(k))).collect();
    let mut uit: Vec<Escl> = Vec::new();
    for d in draden {
        if let Ok(l) = d.join() {
            uit.extend(l);
        }
    }
    uit
}

/// Eén mDNS-vraag op één netwerkkaart (of de standaardkaart bij None).
fn mdns_op(kaart: Option<Ipv4Addr>) -> Vec<Escl> {
    let mut diensten: std::collections::HashMap<String, Dienst> = std::collections::HashMap::new();
    let mut adressen: std::collections::HashMap<String, Ipv4Addr> = std::collections::HashMap::new();
    let Ok(sok) = UdpSocket::bind((kaart.unwrap_or(Ipv4Addr::UNSPECIFIED), 0)) else { return Vec::new() };
    let _ = sok.set_read_timeout(Some(Duration::from_millis(300)));
    let _ = sok.set_multicast_ttl_v4(255);
    if let Some(ip) = kaart {
        // De vraag via deze netwerkkaart versturen (IP_MULTICAST_IF).
        use std::os::windows::io::AsRawSocket;
        use windows_sys::Win32::Networking::WinSock::{setsockopt, IPPROTO_IP, IP_MULTICAST_IF};
        let waarde = u32::from(ip).to_be();
        unsafe {
            setsockopt(sok.as_raw_socket() as usize, IPPROTO_IP, IP_MULTICAST_IF, &waarde as *const u32 as *const u8, 4);
        }
    }
    for soort in ["_uscan._tcp.local", "_uscans._tcp.local"] {
        let mut v = vec![0u8, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0];
        for deel in soort.split('.') {
            v.push(deel.len() as u8);
            v.extend_from_slice(deel.as_bytes());
        }
        v.extend_from_slice(&[0, 0, 12, 0x80, 1]); // PTR, klasse IN met "antwoord rechtstreeks"
        let _ = sok.send_to(&v, "224.0.0.251:5353");
    }
    let begin = Instant::now();
    let mut buf = [0u8; 9000];
    while begin.elapsed() < Duration::from_millis(2500) {
        let Ok((n, _)) = sok.recv_from(&mut buf) else { continue };
        let p = &buf[..n];
        if n < 12 {
            continue;
        }
        let tel = |i: usize| u16::from_be_bytes([p[i], p[i + 1]]) as usize;
        let (vragen, totaal) = (tel(4), tel(6) + tel(8) + tel(10));
        let mut i = 12;
        for _ in 0..vragen {
            let Some((_, j)) = dns_naam(p, i) else { break };
            i = j + 4;
        }
        for _ in 0..totaal {
            let Some((naam, j)) = dns_naam(p, i) else { break };
            if j + 10 > n {
                break;
            }
            let soort = u16::from_be_bytes([p[j], p[j + 1]]);
            let lengte = u16::from_be_bytes([p[j + 8], p[j + 9]]) as usize;
            let data = j + 10;
            if data + lengte > n {
                break;
            }
            let https = naam.contains("_uscans.");
            match soort {
                12 => {
                    if let Some((inst, _)) = dns_naam(p, data) {
                        diensten.entry(inst).or_insert_with(|| Dienst { https, ..Default::default() });
                    }
                }
                33 => {
                    let d = diensten.entry(naam.clone()).or_insert_with(|| Dienst { https, ..Default::default() });
                    d.poort = u16::from_be_bytes([p[data + 4], p[data + 5]]);
                    if let Some((doel, _)) = dns_naam(p, data + 6) {
                        d.doel = doel;
                    }
                }
                16 => {
                    let d = diensten.entry(naam.clone()).or_insert_with(|| Dienst { https, ..Default::default() });
                    let mut k = data;
                    while k < data + lengte {
                        let l = p[k] as usize;
                        let s = String::from_utf8_lossy(&p[k + 1..(k + 1 + l).min(data + lengte)]).to_string();
                        if let Some(v) = s.strip_prefix("rs=") {
                            d.rs = v.to_string();
                        } else if let Some(v) = s.strip_prefix("ty=") {
                            d.ty = v.to_string();
                        }
                        k += 1 + l;
                    }
                }
                1 if lengte == 4 => {
                    adressen.insert(naam.clone(), Ipv4Addr::new(p[data], p[data + 1], p[data + 2], p[data + 3]));
                }
                _ => {}
            }
            i = data + lengte;
        }
    }
    // Per apparaat één keer; gewone http heeft voorrang boven https (eigen certificaten van printers).
    let mut gekozen: Vec<(String, Dienst, Ipv4Addr)> = Vec::new();
    for (inst, d) in diensten {
        let Some(ip) = d.ip.or_else(|| adressen.get(&d.doel).copied()) else { continue };
        if d.poort == 0 {
            continue;
        }
        let naam = if d.ty.is_empty() { inst.split("._").next().unwrap_or(&inst).to_string() } else { d.ty.clone() };
        match gekozen.iter_mut().find(|(n, _, _)| *n == naam) {
            Some(g) if g.1.https && !d.https => *g = (naam, d, ip),
            Some(_) => {}
            None => gekozen.push((naam, d, ip)),
        }
    }
    let mut uit = Vec::new();
    for (naam, d, ip) in gekozen {
        let rs = d.rs.trim_matches('/');
        let rs = if rs.is_empty() { "eSCL" } else { rs };
        let basis = format!("{}://{ip}:{}/{rs}", if d.https { "https" } else { "http" }, d.poort);
        uit.push(Escl { naam, basis, host: ip.to_string() });
    }
    uit
}

/// Vraagt een apparaat in het netwerk of er een eSCL-scanner in zit. Geeft het basisadres.
fn escl_proef(ip: Ipv4Addr) -> Option<String> {
    for basis in [format!("http://{ip}:80/eSCL"), format!("https://{ip}:443/eSCL")] {
        let mut data = Vec::new();
        let r = crate::http::verzoek("GET", &format!("{basis}/ScannerCapabilities"), "", None, 3000, &mut |b, _, _| {
            if data.len() < 200_000 {
                data.extend_from_slice(b);
            }
            Ok(())
        });
        if let Ok((200, _)) = r {
            if String::from_utf8_lossy(&data).contains("ScannerCapabilities") {
                return Some(basis);
            }
        }
    }
    None
}

/// De printers die in Windows staan, met het IP-adres als dat in de naam van de poort staat (zoals "IP_192.168.1.20").
fn printers() -> Vec<Apparaat> {
    use windows_sys::Win32::Graphics::Printing::*;
    let mut uit = Vec::new();
    unsafe {
        let vlaggen = PRINTER_ENUM_LOCAL | PRINTER_ENUM_CONNECTIONS;
        let (mut nodig, mut aantal) = (0u32, 0u32);
        EnumPrintersW(vlaggen, std::ptr::null(), 2, std::ptr::null_mut(), 0, &mut nodig, &mut aantal);
        if nodig == 0 {
            return uit;
        }
        let mut buf: Vec<u64> = vec![0; nodig as usize / 8 + 1];
        if EnumPrintersW(vlaggen, std::ptr::null(), 2, buf.as_mut_ptr() as *mut u8, nodig, &mut nodig, &mut aantal) == 0 {
            return uit;
        }
        let lijst = std::slice::from_raw_parts(buf.as_ptr() as *const PRINTER_INFO_2W, aantal as usize);
        let tekst = |p: *const u16| -> String {
            if p.is_null() {
                return String::new();
            }
            let mut n = 0;
            while *p.add(n) != 0 {
                n += 1;
            }
            String::from_utf16_lossy(std::slice::from_raw_parts(p, n))
        };
        for p in lijst {
            let naam = tekst(p.pPrinterName);
            let poort = tekst(p.pPortName);
            uit.push(Apparaat { naam, ips: ip_in(&poort).into_iter().collect() });
        }
    }
    uit
}

/// Zoekt een IPv4-adres in een tekst ("IP_192.168.1.20", "192.168.1.20_1").
fn ip_in(tekst: &str) -> Option<Ipv4Addr> {
    tekst.split(|c: char| !(c.is_ascii_digit() || c == '.')).filter_map(|d| d.parse::<Ipv4Addr>().ok()).find(|ip| crate::http::lokaal(&ip.to_string()))
}

fn escl_mogelijkheden(basis: &str) -> (bool, bool) {
    let mut data = Vec::new();
    let ok = crate::http::verzoek("GET", &format!("{basis}/ScannerCapabilities"), "", None, 5000, &mut |b, _, _| {
        data.extend_from_slice(b);
        Ok(())
    });
    let tekst = String::from_utf8_lossy(&data);
    match ok {
        Ok((200, _)) => (tekst.contains("Platen") || !tekst.contains("Adf"), tekst.contains("Adf")),
        _ => (true, false),
    }
}

fn escl_scan(basis: &str, invoer: bool, dpi: u32) -> Result<Vec<Beeld>, String> {
    let bron = if invoer { "Feeder" } else { "Platen" };
    // A4 in 1/300 inch.
    let instellingen = format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<scan:ScanSettings xmlns:scan="http://schemas.hp.com/imaging/escl/2011/05/03" xmlns:pwg="http://www.pwg.org/schemas/2010/12/sm">
<pwg:Version>2.0</pwg:Version>
<scan:Intent>Document</scan:Intent>
<pwg:ScanRegions><pwg:ScanRegion><pwg:ContentRegionUnits>escl:ThreeHundredthsOfInches</pwg:ContentRegionUnits><pwg:XOffset>0</pwg:XOffset><pwg:YOffset>0</pwg:YOffset><pwg:Width>2480</pwg:Width><pwg:Height>3508</pwg:Height></pwg:ScanRegion></pwg:ScanRegions>
<pwg:InputSource>{bron}</pwg:InputSource>
<scan:ColorMode>Grayscale8</scan:ColorMode>
<scan:XResolution>{dpi}</scan:XResolution>
<scan:YResolution>{dpi}</scan:YResolution>
<pwg:DocumentFormat>image/jpeg</pwg:DocumentFormat>
<scan:DocumentFormatExt>image/jpeg</scan:DocumentFormatExt>
</scan:ScanSettings>"#);
    let (status, _, locatie) = crate::http::verzoek_met_locatie("POST", &format!("{basis}/ScanJobs"), "Content-Type: text/xml\r\n",
        Some(instellingen.as_bytes()), 30000, &mut |_, _, _| Ok(()))?;
    if status != 201 {
        return Err(format!("de scanner antwoordde {status}"));
    }
    let locatie = locatie.ok_or("geen opdracht van de scanner")?;
    let opdracht = if locatie.starts_with("http") {
        locatie.trim_end_matches('/').to_string()
    } else {
        // Alleen een pad: schema en host van het basisadres ervoor zetten.
        let na = basis.find("://").map(|i| i + 3).unwrap_or(0);
        let host_eind = basis[na..].find('/').map(|i| na + i).unwrap_or(basis.len());
        format!("{}{}", &basis[..host_eind], locatie.trim_end_matches('/'))
    };
    let tijdelijk = std::env::temp_dir().join("Paper Pilot");
    let _ = std::fs::create_dir_all(&tijdelijk);
    let mut beelden = Vec::new();
    for pagina in 0..200 {
        let mut data = Vec::new();
        let mut status = 503;
        // De scanner kan even bezig zijn: dan opnieuw proberen.
        for _ in 0..60 {
            data.clear();
            status = crate::http::verzoek("GET", &format!("{opdracht}/NextDocument"), "", None, 120000, &mut |b, _, _| {
                data.extend_from_slice(b);
                Ok(())
            })?.0;
            if status != 503 {
                break;
            }
            std::thread::sleep(Duration::from_secs(1));
        }
        if status != 200 {
            if pagina == 0 {
                return Err(format!("de scanner antwoordde {status}"));
            }
            break;
        }
        let jpg = tijdelijk.join(format!("escl-{}-{pagina}.jpg", std::process::id()));
        std::fs::write(&jpg, &data).map_err(|e| e.to_string())?;
        if let Ok(b) = crate::beeld::lees(&jpg) {
            beelden.extend(b);
        }
        let _ = std::fs::remove_file(&jpg);
        if !invoer {
            break;
        }
    }
    Ok(beelden)
}
