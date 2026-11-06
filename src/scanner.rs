// Paper Pilot - scanners zoeken en scannen.
//
// Twee soorten:
// 1. Scanners die Windows kent (USB, netwerk via WSD, de scanner in een printer): de scanner-onderdelen van Windows
//    (Windows.Devices.Scanners, gebouwd op WIA). Werkt met bijna elke scanner met een stuurprogramma.
// 2. Netwerkprinters met een scanner die Windows nog niet kent: eSCL (ook AirScan genoemd). Paper Pilot zoekt ze zelf
//    in het netwerk (mDNS, "_uscan._tcp") en scant via HTTP, zonder stuurprogramma.
// Paper Pilot scant op 300 dpi in grijs (goed voor tekstherkenning), van de glasplaat of de documentinvoer.

use crate::beeld::Beeld;
use std::net::{Ipv4Addr, UdpSocket};
use std::time::{Duration, Instant};
use windows::core::HSTRING;
use windows::Devices::Enumeration::DeviceInformation;
use windows::Devices::Scanners::*;
use windows::Storage::StorageFolder;

#[derive(Clone)]
pub enum Soort {
    Windows(String),
    /// Basisadres, bijvoorbeeld http://192.168.1.20:80/eSCL
    Escl(String),
}

#[derive(Clone)]
pub struct Scanner {
    pub naam: String,
    pub soort: Soort,
    pub glasplaat: bool,
    pub invoer: bool,
}

/// Zoekt alle scanners (duurt een paar seconden).
pub fn zoek() -> Vec<Scanner> {
    crate::beeld::com_start();
    let mut lijst = Vec::new();
    match windows_scanners() {
        Ok(l) => lijst.extend(l),
        Err(e) => crate::logboek::schrijf(&format!("Scanners van Windows zoeken: {e}")),
    }
    for s in escl_zoek() {
        // Dezelfde scanner niet twee keer (als Windows hem ook al kent).
        let kort = s.naam.to_lowercase();
        if !lijst.iter().any(|w: &Scanner| kort.contains(&w.naam.to_lowercase()) || w.naam.to_lowercase().contains(&kort)) {
            lijst.push(s);
        }
    }
    crate::logboek::schrijf(&format!("Scanners gevonden: {}", lijst.iter().map(|s| s.naam.clone()).collect::<Vec<_>>().join(", ")));
    lijst
}

fn windows_scanners() -> windows::core::Result<Vec<Scanner>> {
    let mut uit = Vec::new();
    let kiezer = ImageScanner::GetDeviceSelector()?;
    let apparaten = DeviceInformation::FindAllAsyncAqsFilter(&kiezer)?.join()?;
    for d in apparaten {
        let naam = d.Name()?.to_string();
        let id = d.Id()?.to_string();
        let (glas, invoer) = match ImageScanner::FromIdAsync(&HSTRING::from(&id)).and_then(|a| a.join()) {
            Ok(s) => (
                s.IsScanSourceSupported(ImageScannerScanSource::Flatbed).unwrap_or(false),
                s.IsScanSourceSupported(ImageScannerScanSource::Feeder).unwrap_or(false),
            ),
            Err(_) => (true, false),
        };
        uit.push(Scanner { naam, soort: Soort::Windows(id), glasplaat: glas || !invoer, invoer });
    }
    Ok(uit)
}

/// Scant. Met `invoer` alle pagina's uit de documentinvoer, anders één pagina van de glasplaat.
pub fn scan(s: &Scanner, invoer: bool) -> Result<Vec<Beeld>, String> {
    crate::beeld::com_start();
    let uitkomst = match &s.soort {
        Soort::Windows(id) => windows_scan(id, invoer).map_err(|e| format!("{e}")),
        Soort::Escl(basis) => escl_scan(basis, invoer),
    };
    if let Err(e) = &uitkomst {
        crate::logboek::schrijf(&format!("Scannen met {}: {e}", s.naam));
    }
    uitkomst
}

fn windows_scan(id: &str, invoer: bool) -> windows::core::Result<Vec<Beeld>> {
    let scanner = ImageScanner::FromIdAsync(&HSTRING::from(id))?.join()?;
    let dpi = ImageScannerResolution { DpiX: 300.0, DpiY: 300.0 };
    let bron = if invoer {
        let c = scanner.FeederConfiguration()?;
        let _ = c.SetDesiredResolution(dpi);
        if c.IsColorModeSupported(ImageScannerColorMode::Grayscale).unwrap_or(false) {
            let _ = c.SetColorMode(ImageScannerColorMode::Grayscale);
        }
        if c.IsFormatSupported(ImageScannerFormat::Png).unwrap_or(false) {
            let _ = c.SetFormat(ImageScannerFormat::Png);
        }
        let _ = c.SetMaxNumberOfPages(0);
        ImageScannerScanSource::Feeder
    } else if scanner.IsScanSourceSupported(ImageScannerScanSource::Flatbed).unwrap_or(false) {
        let c = scanner.FlatbedConfiguration()?;
        let _ = c.SetDesiredResolution(dpi);
        if c.IsColorModeSupported(ImageScannerColorMode::Grayscale).unwrap_or(false) {
            let _ = c.SetColorMode(ImageScannerColorMode::Grayscale);
        }
        if c.IsFormatSupported(ImageScannerFormat::Png).unwrap_or(false) {
            let _ = c.SetFormat(ImageScannerFormat::Png);
        }
        ImageScannerScanSource::Flatbed
    } else {
        ImageScannerScanSource::Default
    };
    let map = std::env::temp_dir().join("Paper Pilot").join(format!("scan-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&map);
    let folder = StorageFolder::GetFolderFromPathAsync(&HSTRING::from(map.as_os_str()))?.join()?;
    let uitkomst = scanner.ScanFilesToFolderAsync(bron, &folder)?.join()?;
    let mut beelden = Vec::new();
    for bestand in uitkomst.ScannedFiles()? {
        let pad = std::path::PathBuf::from(bestand.Path()?.to_string());
        if let Ok(b) = crate::beeld::lees(&pad) {
            beelden.extend(b);
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

/// Zoekt eSCL-scanners in het netwerk met mDNS (2,5 seconden).
fn escl_zoek() -> Vec<Scanner> {
    let mut diensten: std::collections::HashMap<String, Dienst> = std::collections::HashMap::new();
    let mut adressen: std::collections::HashMap<String, Ipv4Addr> = std::collections::HashMap::new();
    let Ok(sok) = UdpSocket::bind("0.0.0.0:0") else { return Vec::new() };
    let _ = sok.set_read_timeout(Some(Duration::from_millis(300)));
    let _ = sok.set_multicast_ttl_v4(255);
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
        let (glas, invoer) = escl_mogelijkheden(&basis);
        uit.push(Scanner { naam: format!("{naam} ({})", crate::teksten::t("ScannerNetwerk")), soort: Soort::Escl(basis), glasplaat: glas, invoer });
    }
    uit
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

fn escl_scan(basis: &str, invoer: bool) -> Result<Vec<Beeld>, String> {
    let bron = if invoer { "Feeder" } else { "Platen" };
    // A4 in 1/300 inch.
    let instellingen = format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<scan:ScanSettings xmlns:scan="http://schemas.hp.com/imaging/escl/2011/05/03" xmlns:pwg="http://www.pwg.org/schemas/2010/12/sm">
<pwg:Version>2.0</pwg:Version>
<scan:Intent>Document</scan:Intent>
<pwg:ScanRegions><pwg:ScanRegion><pwg:ContentRegionUnits>escl:ThreeHundredthsOfInches</pwg:ContentRegionUnits><pwg:XOffset>0</pwg:XOffset><pwg:YOffset>0</pwg:YOffset><pwg:Width>2480</pwg:Width><pwg:Height>3508</pwg:Height></pwg:ScanRegion></pwg:ScanRegions>
<pwg:InputSource>{bron}</pwg:InputSource>
<scan:ColorMode>Grayscale8</scan:ColorMode>
<scan:XResolution>300</scan:XResolution>
<scan:YResolution>300</scan:YResolution>
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
