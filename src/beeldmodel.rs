// Paper Pilot - het taalmodel dat kan kijken: handschrift lezen en beschrijven wat er op een foto of tekening staat.
//
// Alles gebeurt op de eigen computer, zonder internet (alleen de eenmalige download gebruikt internet):
// - de motor is llama.cpp (llama-server.exe, MIT-licentie), een vaste versie met Vulkan (videokaart, anders processor);
// - het model is Qwen3-VL 4B (Apache 2.0) als GGUF, met het deel voor beelden (mmproj). Welke bestanden precies,
//   vraagt Paper Pilot bij het downloaden aan Hugging Face (Q4_K_M).
// Alles staat in %LOCALAPPDATA%\Paper Pilot\Taalmodel. De motor draait als apart proces op 127.0.0.1, zit in een
// taakgroep (stopt als Paper Pilot stopt) en stopt vanzelf na 10 minuten zonder vraag.

use crate::beeld::Beeld;
use crate::dialogen::Voortgang;
use crate::teksten::t;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const MOTOR_VERSIE: &str = "b11669";
/// Mappen op Hugging Face, de eerste die werkt wordt gebruikt.
const MAPPEN: [&str; 3] = ["Qwen/Qwen3-VL-4B-Instruct-GGUF", "unsloth/Qwen3-VL-4B-Instruct-GGUF", "ggml-org/Qwen2.5-VL-3B-Instruct-GGUF"];

pub enum Soort {
    Handschrift,
    Beschrijven,
}

pub fn map() -> PathBuf {
    if let Some(m) = std::env::var_os("PAPERPILOT_TAALMODEL") {
        return PathBuf::from(m);
    }
    let basis = crate::win::omgeving("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    basis.join("Paper Pilot").join("Taalmodel")
}

fn motor_exe() -> Option<PathBuf> {
    fn zoek(map: &Path) -> Option<PathBuf> {
        for e in fs::read_dir(map).ok()?.flatten() {
            let p = e.path();
            if p.is_dir() {
                if let Some(v) = zoek(&p) {
                    return Some(v);
                }
            } else if p.file_name().map(|n| n.eq_ignore_ascii_case("llama-server.exe")).unwrap_or(false) {
                return Some(p);
            }
        }
        None
    }
    zoek(&map().join("motor"))
}

pub fn geinstalleerd() -> bool {
    motor_exe().is_some() && map().join("model.gguf").is_file() && map().join("mmproj.gguf").is_file()
}

/// Grootte op schijf in GB (voor de melding bij verwijderen).
pub fn grootte_gb() -> f64 {
    fn tel(p: &Path) -> u64 {
        fs::read_dir(p).map(|l| l.flatten().map(|e| {
            let p = e.path();
            if p.is_dir() { tel(&p) } else { e.metadata().map(|m| m.len()).unwrap_or(0) }
        }).sum()).unwrap_or(0)
    }
    tel(&map()) as f64 / 1e9
}

// ------------------------------------------------------------------ downloaden

fn kies_bestanden(map: &str) -> Result<(String, String), String> {
    let json = crate::http::haal_tekst(&format!("https://huggingface.co/api/models/{map}"))?;
    let v: serde_json::Value = serde_json::from_str(&json).map_err(|e| e.to_string())?;
    let namen: Vec<String> = v["siblings"].as_array().map(|a| a.iter().filter_map(|s| s["rfilename"].as_str().map(String::from)).collect()).unwrap_or_default();
    let gguf: Vec<&String> = namen.iter().filter(|n| n.to_lowercase().ends_with(".gguf")).collect();
    let mut model = None;
    for soort in ["Q4_K_M", "Q4_K_S", "Q4_0", "Q5_K_M", "Q8_0"] {
        model = gguf.iter().find(|n| !n.to_lowercase().contains("mmproj") && n.to_uppercase().contains(soort) && !n.contains("-of-")).map(|n| n.to_string());
        if model.is_some() {
            break;
        }
    }
    let mut mmproj = None;
    for soort in ["F16", "Q8_0", "BF16", ""] {
        mmproj = gguf.iter().find(|n| n.to_lowercase().contains("mmproj") && n.to_uppercase().contains(soort)).map(|n| n.to_string());
        if mmproj.is_some() {
            break;
        }
    }
    match (model, mmproj) {
        (Some(m), Some(p)) => Ok((m, p)),
        _ => Err(format!("geen geschikte bestanden in {map}")),
    }
}

fn haal(adres: &str, doel: &Path, wat: &str, m: &Voortgang) -> Result<(), String> {
    let mut laatst = Instant::now() - Duration::from_secs(1);
    crate::http::download(adres, doel, true, &mut |binnen, totaal| {
        if laatst.elapsed() >= Duration::from_millis(400) {
            laatst = Instant::now();
            let mb = binnen as f64 / 1e6;
            let (tekst, pct) = match totaal {
                Some(t) if t > 0 => (crate::teksten::tf("ModelDownloadVan", &[wat, &format!("{:.0}", mb), &format!("{:.0}", t as f64 / 1e6)]), (binnen * 100 / t) as u32),
                _ => (crate::teksten::tf("ModelDownload", &[wat, &format!("{:.0}", mb)]), 0),
            };
            m.meld(tekst, pct);
        }
    }, &m.gestopt)
}

/// Downloadt de motor en het model (als ze er nog niet zijn). Kan onderbroken worden; dan later verder.
pub fn installeer(m: &Voortgang) -> Result<(), String> {
    let doelmap = map();
    fs::create_dir_all(&doelmap).map_err(|e| e.to_string())?;
    if motor_exe().is_none() {
        let zip = doelmap.join("motor.zip");
        let adres = format!("https://github.com/ggml-org/llama.cpp/releases/download/{MOTOR_VERSIE}/llama-{MOTOR_VERSIE}-bin-win-vulkan-x64.zip");
        haal(&adres, &zip, &t("ModelDeelMotor"), m)?;
        m.meld(t("ModelUitpakken"), 100);
        let bestand = fs::File::open(&zip).map_err(|e| e.to_string())?;
        let mut archief = zip::ZipArchive::new(bestand).map_err(|e| e.to_string())?;
        archief.extract(doelmap.join("motor")).map_err(|e| e.to_string())?;
        let _ = fs::remove_file(&zip);
        if motor_exe().is_none() {
            return Err("geen llama-server.exe in de zip".into());
        }
    }
    if !(doelmap.join("model.gguf").is_file() && doelmap.join("mmproj.gguf").is_file()) {
        let eigen = std::env::var("PAPERPILOT_HFMAP").ok().or_else(|| crate::instellingen::get("beeldmodel_map"));
        let mappen: Vec<String> = match eigen {
            Some(eigen) if !eigen.is_empty() => vec![eigen],
            _ => MAPPEN.iter().map(|s| s.to_string()).collect(),
        };
        let mut gekozen = None;
        let mut laatste_fout = String::new();
        for map in &mappen {
            match kies_bestanden(map) {
                Ok(b) => {
                    gekozen = Some((map.clone(), b));
                    break;
                }
                Err(e) => laatste_fout = e,
            }
        }
        let (hfmap, (model, mmproj)) = gekozen.ok_or(laatste_fout)?;
        crate::logboek::schrijf(&format!("Taalmodel: {hfmap} / {model} / {mmproj}"));
        let adres = |b: &str| format!("https://huggingface.co/{hfmap}/resolve/main/{}", b.replace(' ', "%20"));
        haal(&adres(&mmproj), &doelmap.join("mmproj.gguf"), &t("ModelDeelBeeld"), m)?;
        haal(&adres(&model), &doelmap.join("model.gguf"), &t("ModelDeelModel"), m)?;
        let _ = fs::write(doelmap.join("model.txt"), format!("{hfmap}\r\n{model}\r\n{mmproj}\r\n"));
    }
    Ok(())
}

pub fn verwijder() -> Result<(), String> {
    stop();
    fs::remove_dir_all(map()).map_err(|e| e.to_string())
}

// ------------------------------------------------------------------ de motor

struct Motor {
    kind: Child,
    poort: u16,
    laatst: Instant,
}

static MOTOR: Mutex<Option<Motor>> = Mutex::new(None);
static BEWAKER: AtomicBool = AtomicBool::new(false);

fn taakgroep(kind: &Child) {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::System::JobObjects::*;
    static GROEP: Mutex<usize> = Mutex::new(0);
    unsafe {
        let mut g = GROEP.lock().unwrap();
        if *g == 0 {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if !job.is_null() {
                let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                SetInformationJobObject(job, JobObjectExtendedLimitInformation, &info as *const _ as *const _,
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32);
                *g = job as usize;
            }
        }
        if *g != 0 {
            AssignProcessToJobObject(*g as _, kind.as_raw_handle() as _);
        }
    }
}

fn vrije_poort() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0").and_then(|l| l.local_addr()).map(|a| a.port()).unwrap_or(38917)
}

fn start_motor(lagen: &str) -> Result<Motor, String> {
    use std::os::windows::process::CommandExt;
    let exe = motor_exe().ok_or_else(|| t("ModelNietGeinstalleerd"))?;
    let poort = vrije_poort();
    let log = fs::File::create(map().join("motor.log")).map_err(|e| e.to_string())?;
    let log2 = log.try_clone().map_err(|e| e.to_string())?;
    let kind = Command::new(&exe)
        .args(["-m", &map().join("model.gguf").to_string_lossy(), "--mmproj", &map().join("mmproj.gguf").to_string_lossy(),
            "--host", "127.0.0.1", "--port", &poort.to_string(), "-c", "8192", "-ngl", lagen, "--no-webui"])
        .current_dir(exe.parent().unwrap_or(Path::new(".")))
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(log2))
        .creation_flags(0x0800_0000) // geen venster
        .spawn()
        .map_err(|e| format!("motor starten: {e}"))?;
    taakgroep(&kind);
    let mut motor = Motor { kind, poort, laatst: Instant::now() };
    // Wachten tot het model geladen is (hooguit 4 minuten).
    let begin = Instant::now();
    while begin.elapsed() < Duration::from_secs(240) {
        if let Ok(Some(_)) = motor.kind.try_wait() {
            return Err("de motor stopte".into());
        }
        let mut ok = false;
        let _ = crate::http::verzoek("GET", &format!("http://127.0.0.1:{poort}/health"), "", None, 5000, &mut |_, s, _| {
            ok = s == 200;
            Ok(())
        });
        if ok {
            return Ok(motor);
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    let _ = motor.kind.kill();
    Err("de motor werd niet op tijd klaar".into())
}

/// Start de motor als hij nog niet draait; eerst met de videokaart, anders alleen de processor.
fn zorg_voor_motor() -> Result<u16, String> {
    let mut slot = MOTOR.lock().unwrap();
    if let Some(m) = slot.as_mut() {
        if matches!(m.kind.try_wait(), Ok(None)) {
            m.laatst = Instant::now();
            return Ok(m.poort);
        }
    }
    let motor = match start_motor("99") {
        Ok(m) => m,
        Err(e) => {
            crate::logboek::schrijf(&format!("Motor met videokaart: {e}; nu met alleen de processor."));
            start_motor("0").map_err(|e| {
                crate::logboek::schrijf(&format!("Motor: {e}"));
                t("ModelStartNiet")
            })?
        }
    };
    let poort = motor.poort;
    *slot = Some(motor);
    if !BEWAKER.swap(true, Ordering::SeqCst) {
        std::thread::spawn(|| loop {
            std::thread::sleep(Duration::from_secs(30));
            let mut slot = MOTOR.lock().unwrap();
            if slot.as_ref().map(|m| m.laatst.elapsed() > Duration::from_secs(600)).unwrap_or(false) {
                if let Some(mut m) = slot.take() {
                    let _ = m.kind.kill();
                    crate::logboek::schrijf("Taalmodel gestopt na 10 minuten zonder vraag.");
                }
            }
        });
    }
    Ok(poort)
}

pub fn stop() {
    if let Some(mut m) = MOTOR.lock().unwrap().take() {
        let _ = m.kind.kill();
    }
}

fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut uit = String::with_capacity(data.len() * 4 / 3 + 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        uit.push(T[(n >> 18) as usize & 63] as char);
        uit.push(T[(n >> 12) as usize & 63] as char);
        uit.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        uit.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    uit
}

fn opdracht(soort: &Soort) -> String {
    match soort {
        Soort::Handschrift => t("ModelVraagHandschrift"),
        Soort::Beschrijven => t("ModelVraagBeschrijven"),
    }
}

/// Laat het model naar de afbeelding kijken. Kan lang duren (op de processor soms minuten).
pub fn vraag(beeld: &Beeld, soort: Soort) -> Result<String, String> {
    if !geinstalleerd() {
        return Err(t("ModelNietGeinstalleerd"));
    }
    let poort = zorg_voor_motor()?;
    // Als JPG naar het model sturen (kleiner), hooguit 1280 beeldpunten aan de lange kant.
    let tijdelijk = std::env::temp_dir().join("Paper Pilot");
    let _ = fs::create_dir_all(&tijdelijk);
    let jpg = tijdelijk.join(format!("vraag-{}.jpg", std::process::id()));
    crate::beeld::schrijf(&jpg, &[beeld.verklein(1280)])?;
    let data = fs::read(&jpg).map_err(|e| e.to_string())?;
    let _ = fs::remove_file(&jpg);
    let lichaam = serde_json::json!({
        "messages": [{
            "role": "user",
            "content": [
                { "type": "image_url", "image_url": { "url": format!("data:image/jpeg;base64,{}", base64(&data)) } },
                { "type": "text", "text": opdracht(&soort) }
            ]
        }],
        "temperature": 0.1,
        "max_tokens": 1500
    })
    .to_string();
    let mut antwoord = Vec::new();
    let (status, _) = crate::http::verzoek("POST", &format!("http://127.0.0.1:{poort}/v1/chat/completions"),
        "Content-Type: application/json\r\n", Some(lichaam.as_bytes()), 900_000, &mut |b, _, _| {
            antwoord.extend_from_slice(b);
            Ok(())
        })?;
    if let Some(m) = MOTOR.lock().unwrap().as_mut() {
        m.laatst = Instant::now();
    }
    let tekst = String::from_utf8_lossy(&antwoord).to_string();
    if status != 200 {
        crate::logboek::schrijf(&format!("Taalmodel antwoordde {status}: {}", tekst.chars().take(300).collect::<String>()));
        return Err(t("ModelFout"));
    }
    let v: serde_json::Value = serde_json::from_str(&tekst).map_err(|e| e.to_string())?;
    let inhoud = v["choices"][0]["message"]["content"].as_str().unwrap_or("").trim().to_string();
    if inhoud.is_empty() {
        return Err(t("ModelLeeg"));
    }
    Ok(inhoud)
}
