// Paper Pilot - scannen met de klassieke scanner-onderdelen van Windows (WIA Automation, wiaaut.dll).
//
// Dit is dezelfde weg als "Windows Faxen en scannen": vrijwel elke scanner met een stuurprogramma werkt ermee
// (USB, netwerk via WSD, de scanner in een printer van bijvoorbeeld Brother, HP, Canon of Epson).
// Paper Pilot gebruikt dit als de nieuwere weg (Windows.Devices.Scanners) niet lukt, en voor scanners die alleen
// hier te zien zijn. Alles gaat via IDispatch (late binding), op een eigen draad met een STA (WIA wil dat).

use crate::beeld::Beeld;
use std::mem::ManuallyDrop;
use windows::core::{BSTR, GUID, HSTRING, PCWSTR};
use windows::Win32::System::Com::*;
use windows::Win32::System::Variant::*;

/// WIA.DeviceManager
const CLSID_DEVICE_MANAGER: GUID = GUID::from_u128(0xE1C5D730_7E97_4D8A_9E42_BBAE87C2059F);
/// wiaFormatBMP: elke scanner kan dit.
const FORMAAT_BMP: &str = "{B96B3CAB-0728-11D3-9D7B-0000F81EF32E}";
const LCID: u32 = 0x0400;

/// Een VARIANT die zichzelf opruimt.
struct Var(VARIANT);

impl Drop for Var {
    fn drop(&mut self) {
        unsafe {
            let _ = VariantClear(&mut self.0);
        }
    }
}

impl Var {
    fn leeg() -> Var {
        Var(VARIANT::default())
    }

    fn getal(v: i32) -> Var {
        let mut x = VARIANT::default();
        unsafe {
            (*x.Anonymous.Anonymous).vt = VT_I4;
            (*x.Anonymous.Anonymous).Anonymous.lVal = v;
        }
        Var(x)
    }

    fn tekst(s: &str) -> Var {
        let mut x = VARIANT::default();
        unsafe {
            (*x.Anonymous.Anonymous).vt = VT_BSTR;
            (*x.Anonymous.Anonymous).Anonymous.bstrVal = ManuallyDrop::new(BSTR::from(s));
        }
        Var(x)
    }

    fn soort(&self) -> VARENUM {
        unsafe { self.0.Anonymous.Anonymous.vt }
    }

    fn object(&self) -> Result<IDispatch, String> {
        if self.soort() == VT_DISPATCH {
            if let Some(d) = unsafe { self.0.Anonymous.Anonymous.Anonymous.pdispVal.as_ref() } {
                return Ok(d.clone());
            }
        }
        Err(format!("geen object (soort {})", self.soort().0))
    }

    fn als_tekst(&self) -> String {
        unsafe {
            match self.soort() {
                VT_BSTR => self.0.Anonymous.Anonymous.Anonymous.bstrVal.to_string(),
                VT_I4 | VT_INT => self.0.Anonymous.Anonymous.Anonymous.lVal.to_string(),
                VT_I2 => self.0.Anonymous.Anonymous.Anonymous.iVal.to_string(),
                _ => String::new(),
            }
        }
    }

    fn als_getal(&self) -> i32 {
        unsafe {
            match self.soort() {
                VT_I4 | VT_INT => self.0.Anonymous.Anonymous.Anonymous.lVal,
                VT_I2 => self.0.Anonymous.Anonymous.Anonymous.iVal as i32,
                VT_BSTR => self.als_tekst().trim().parse().unwrap_or(0),
                _ => 0,
            }
        }
    }
}

/// Roept een methode of eigenschap aan op een IDispatch-object.
fn roep(obj: &IDispatch, naam: &str, soort: DISPATCH_FLAGS, args: &[Var]) -> Result<Var, String> {
    let n = HSTRING::from(naam);
    let p = PCWSTR(n.as_ptr());
    let mut id = 0i32;
    unsafe { obj.GetIDsOfNames(&GUID::zeroed(), &p, 1, LCID, &mut id) }.map_err(|e| format!("{naam}: {}", e.message()))?;
    // De argumenten gaan in omgekeerde volgorde mee (ondiepe kopie; de Var's ruimen zelf op).
    let mut lijst: Vec<VARIANT> = args.iter().rev().map(|a| unsafe { std::ptr::read(&a.0) }).collect();
    let mut zet = windows::Win32::System::Ole::DISPID_PROPERTYPUT;
    let mut dp = DISPPARAMS { rgvarg: lijst.as_mut_ptr(), rgdispidNamedArgs: std::ptr::null_mut(), cArgs: lijst.len() as u32, cNamedArgs: 0 };
    if soort == DISPATCH_PROPERTYPUT {
        dp.rgdispidNamedArgs = &mut zet;
        dp.cNamedArgs = 1;
    }
    let mut uit = Var::leeg();
    let mut fout = EXCEPINFO::default();
    let r = unsafe { obj.Invoke(id, &GUID::zeroed(), LCID, soort, &dp, Some(&mut uit.0), Some(&mut fout), None) };
    // De kopieën niet opruimen: dat doen de originelen.
    for v in lijst {
        std::mem::forget(v);
    }
    let beschrijving = unsafe {
        let b = ManuallyDrop::take(&mut fout.bstrDescription);
        let _ = ManuallyDrop::take(&mut fout.bstrSource);
        let _ = ManuallyDrop::take(&mut fout.bstrHelpFile);
        b.to_string()
    };
    match r {
        Ok(()) => Ok(uit),
        Err(e) => {
            // Bij een fout van WIA zelf staat de echte code in scode.
            let code = if fout.scode != 0 { fout.scode as u32 } else { e.code().0 as u32 };
            Err(format!("{naam}: {code:#010x} {} {beschrijving}", e.message()).trim().to_string())
        }
    }
}

fn haal(obj: &IDispatch, naam: &str) -> Result<Var, String> {
    roep(obj, naam, DISPATCH_PROPERTYGET, &[])
}

fn item(lijst: &IDispatch, index: Var) -> Result<IDispatch, String> {
    roep(lijst, "Item", DISPATCH_METHOD | DISPATCH_PROPERTYGET, &[index])?.object()
}

/// Een eigenschap van WIA zetten (bijvoorbeeld "6147" = horizontale resolutie). Fouten tellen niet: niet elke scanner kent alles.
fn zet_eigenschap(obj: &IDispatch, id: &str, waarde: i32) {
    let r = haal(obj, "Properties").and_then(|p| p.object()).and_then(|p| item(&p, Var::tekst(id)))
        .and_then(|e| roep(&e, "Value", DISPATCH_PROPERTYPUT, &[Var::getal(waarde)]));
    if let Err(e) = r {
        crate::logboek::schrijf(&format!("WIA eigenschap {id} = {waarde}: {e}"));
    }
}

fn lees_eigenschap(obj: &IDispatch, id: &str) -> Option<Var> {
    haal(obj, "Properties").and_then(|p| p.object()).and_then(|p| item(&p, Var::tekst(id))).and_then(|e| haal(&e, "Value")).ok()
}

/// Voert werk uit op een eigen draad met een STA (zoals WIA wil).
fn op_sta<T: Send + 'static>(taak: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    std::thread::spawn(move || {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        }
        let r = taak();
        unsafe { CoUninitialize() };
        r
    })
    .join()
    .unwrap_or_else(|_| Err("WIA: onverwachte fout".into()))
}

fn apparaten_beheer() -> Result<IDispatch, String> {
    unsafe { CoCreateInstance(&CLSID_DEVICE_MANAGER, None, CLSCTX_ALL) }
        .map_err(|e| format!("WIA.DeviceManager: {:#010x} {}", e.code().0 as u32, e.message()).trim().to_string())
}

/// Een scanner die WIA kent.
pub struct WiaScanner {
    pub naam: String,
    pub id: String,
    pub glasplaat: bool,
    pub invoer: bool,
}

/// Alle scanners die WIA kent.
pub fn scanners() -> Result<Vec<WiaScanner>, String> {
    op_sta(|| {
        let beheer = apparaten_beheer()?;
        let infos = haal(&beheer, "DeviceInfos")?.object()?;
        let aantal = haal(&infos, "Count")?.als_getal();
        let mut uit = Vec::new();
        for i in 1..=aantal {
            let Ok(info) = item(&infos, Var::getal(i)) else { continue };
            // Type 1 = scanner (2 = camera, 3 = video).
            if haal(&info, "Type").map(|v| v.als_getal()).unwrap_or(0) != 1 {
                continue;
            }
            let id = haal(&info, "DeviceID").map(|v| v.als_tekst()).unwrap_or_default();
            let naam = lees_eigenschap(&info, "Name").map(|v| v.als_tekst()).filter(|n| !n.is_empty()).unwrap_or_else(|| id.clone());
            // WIA_DPS_DOCUMENT_HANDLING_CAPABILITIES (3086): 1 = documentinvoer, 2 = glasplaat.
            let (glas, invoer) = match roep(&info, "Connect", DISPATCH_METHOD, &[]).and_then(|v| v.object()) {
                Ok(apparaat) => match lees_eigenschap(&apparaat, "3086") {
                    Some(v) => {
                        let k = v.als_getal();
                        (k & 2 != 0 || k & 1 == 0, k & 1 != 0)
                    }
                    None => (true, false),
                },
                Err(e) => {
                    crate::logboek::schrijf(&format!("WIA {naam} verbinden: {e}"));
                    (true, false)
                }
            };
            uit.push(WiaScanner { naam, id, glasplaat: glas, invoer });
        }
        Ok(uit)
    })
}

/// Scant met WIA. `invoer`: alle pagina's uit de documentinvoer, anders één pagina van de glasplaat.
pub fn scan(id: &str, invoer: bool, dpi: u32) -> Result<Vec<Beeld>, String> {
    let id = id.to_string();
    op_sta(move || {
        let beheer = apparaten_beheer()?;
        let infos = haal(&beheer, "DeviceInfos")?.object()?;
        let aantal = haal(&infos, "Count")?.als_getal();
        let mut gevonden = None;
        for i in 1..=aantal {
            if let Ok(info) = item(&infos, Var::getal(i)) {
                if haal(&info, "DeviceID").map(|v| v.als_tekst()).unwrap_or_default() == id {
                    gevonden = Some(info);
                    break;
                }
            }
        }
        let info = gevonden.ok_or_else(|| "WIA: scanner niet gevonden (0x80210005)".to_string())?;
        let apparaat = roep(&info, "Connect", DISPATCH_METHOD, &[])?.object()?;
        // WIA_DPS_DOCUMENT_HANDLING_SELECT (3088): 1 = documentinvoer, 2 = glasplaat. WIA_DPS_PAGES (3096): 0 = alle.
        zet_eigenschap(&apparaat, "3088", if invoer { 1 } else { 2 });
        if invoer {
            zet_eigenschap(&apparaat, "3096", 1);
        }
        let items = haal(&apparaat, "Items")?.object()?;
        let onderdeel = item(&items, Var::getal(1))?;
        // 6146 = soort beeld (2 = grijs), 6147/6148 = resolutie.
        zet_eigenschap(&onderdeel, "6146", 2);
        zet_eigenschap(&onderdeel, "6147", dpi as i32);
        zet_eigenschap(&onderdeel, "6148", dpi as i32);
        let map = std::env::temp_dir().join("Paper Pilot");
        let _ = std::fs::create_dir_all(&map);
        let mut beelden = Vec::new();
        for pagina in 0..200 {
            let beeld = match roep(&onderdeel, "Transfer", DISPATCH_METHOD, &[Var::tekst(FORMAAT_BMP)]).and_then(|v| v.object()) {
                Ok(b) => b,
                Err(e) => {
                    // Bij de documentinvoer betekent een fout na de eerste pagina meestal "geen papier meer": klaar.
                    if pagina > 0 && invoer {
                        crate::logboek::schrijf(&format!("WIA documentinvoer klaar na {pagina} pagina('s): {e}"));
                        break;
                    }
                    return Err(e);
                }
            };
            let pad = map.join(format!("wia-{}-{pagina}.bmp", std::process::id()));
            let _ = std::fs::remove_file(&pad);
            roep(&beeld, "SaveFile", DISPATCH_METHOD, &[Var::tekst(&pad.to_string_lossy())])?;
            match crate::beeld::lees(&pad) {
                Ok(b) => beelden.extend(b),
                Err(e) => crate::logboek::schrijf(&format!("WIA-pagina lezen: {e}")),
            }
            let _ = std::fs::remove_file(&pad);
            if !invoer {
                break;
            }
        }
        Ok(beelden)
    })
}

/// Proef zonder scanner: de IDispatch-hulpjes met Scripting.Dictionary (zit in elke Windows), en daarna kijken wat WIA ziet.
pub fn proef() -> Result<String, String> {
    let mut uit = op_sta(|| {
        let clsid = unsafe { CLSIDFromProgID(windows::core::w!("Scripting.Dictionary")) }.map_err(|e| format!("Scripting.Dictionary: {e}"))?;
        let d: IDispatch = unsafe { CoCreateInstance(&clsid, None, CLSCTX_ALL) }.map_err(|e| format!("Dictionary maken: {e}"))?;
        roep(&d, "Add", DISPATCH_METHOD, &[Var::tekst("brief"), Var::getal(42)])?;
        roep(&d, "Add", DISPATCH_METHOD, &[Var::tekst("naam"), Var::tekst("Paper Pilot")])?;
        let aantal = haal(&d, "Count")?.als_getal();
        let getal = roep(&d, "Item", DISPATCH_METHOD | DISPATCH_PROPERTYGET, &[Var::tekst("brief")])?.als_getal();
        let naam = roep(&d, "Item", DISPATCH_METHOD | DISPATCH_PROPERTYGET, &[Var::tekst("naam")])?.als_tekst();
        roep(&d, "Item", DISPATCH_PROPERTYPUT, &[Var::tekst("brief"), Var::getal(7)])?;
        let nieuw = roep(&d, "Item", DISPATCH_METHOD | DISPATCH_PROPERTYGET, &[Var::tekst("brief")])?.als_getal();
        if aantal != 2 || getal != 42 || naam != "Paper Pilot" || nieuw != 7 {
            return Err(format!("IDispatch geeft verkeerde waarden: {aantal} {getal} {naam} {nieuw}"));
        }
        Ok(format!("IDispatch-hulpjes: goed (Count {aantal}, Item {getal}, {naam}, na zetten {nieuw})\r\n"))
    })?;
    match scanners() {
        Ok(l) => uit.push_str(&format!("WIA ziet {} scanner(s): {}\r\n", l.len(), l.iter().map(|s| s.naam.clone()).collect::<Vec<_>>().join(", "))),
        Err(e) => uit.push_str(&format!("WIA niet beschikbaar op deze computer (ter informatie): {e}\r\n")),
    }
    Ok(uit)
}
