// Paper Pilot - een brief scannen met de camera van de laptop (of een losse webcam).
//
// Paper Pilot maakt steeds een foto (ongeveer elke seconde) en kijkt: is er een brief in beeld, valt er een stuk
// buiten beeld, is het te donker, is de foto scherp, staat er tekst op? De schermlezer zegt wat je moet doen
// ("schuif de brief een beetje naar links"). Is de brief twee keer achter elkaar goed in beeld, dan bewaart Paper Pilot
// die foto en herkent de tekst, net als bij een scan.
//
// Links en rechts: een camera boven het scherm kijkt naar jou. Wat aan jouw rechterkant is, staat op de foto links.
// Valt de brief op de foto links buiten beeld, dan steekt hij aan jouw rechterkant uit: schuif hem naar links.

use crate::beeld::Beeld;
use crate::teksten::t;
use windows::core::HSTRING;
use windows::Media::Capture::{MediaCapture, MediaCaptureInitializationSettings, StreamingCaptureMode};
use windows::Media::MediaProperties::ImageEncodingProperties;
use windows::Storage::Streams::{DataReader, InMemoryRandomAccessStream};

/// Wat Paper Pilot van een foto vindt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Oordeel {
    Goed,
    Donker,
    GeenBrief,
    Dichtbij,
    NaarLinks,
    NaarRechts,
    Hoger,
    Lager,
    Wazig,
    GeenTekst,
}

impl Oordeel {
    /// De sleutel van de zin voor de schermlezer.
    pub fn sleutel(self) -> &'static str {
        match self {
            Oordeel::Goed => "CameraGoed",
            Oordeel::Donker => "CameraDonker",
            Oordeel::GeenBrief => "CameraGeenBrief",
            Oordeel::Dichtbij => "CameraDichtbij",
            Oordeel::NaarLinks => "CameraNaarLinks",
            Oordeel::NaarRechts => "CameraNaarRechts",
            Oordeel::Hoger => "CameraHoger",
            Oordeel::Lager => "CameraLager",
            Oordeel::Wazig => "CameraWazig",
            Oordeel::GeenTekst => "CameraGeenTekst",
        }
    }
}

/// Kijkt of de brief goed in beeld is.
pub fn beoordeel(beeld: &Beeld) -> Oordeel {
    // Verkleind naar ongeveer 320 punten breed, in grijs.
    let stap = (beeld.breed / 320).max(1) as usize;
    let (bb, bh) = (beeld.breed as usize / stap, beeld.hoog as usize / stap);
    if bb < 20 || bh < 20 {
        return Oordeel::GeenBrief;
    }
    let mut g = vec![0u8; bb * bh];
    for y in 0..bh {
        for x in 0..bb {
            let i = ((y * stap) * beeld.breed as usize + x * stap) * 4;
            let p = &beeld.punten[i..i + 3];
            g[y * bb + x] = ((p[0] as u32 * 29 + p[1] as u32 * 150 + p[2] as u32 * 77) >> 8) as u8;
        }
    }
    let gemiddeld = g.iter().map(|v| *v as u64).sum::<u64>() / g.len() as u64;
    if gemiddeld < 45 {
        return Oordeel::Donker;
    }
    // Drempel tussen papier (licht) en de rest (Otsu).
    let mut hist = [0u64; 256];
    for v in &g {
        hist[*v as usize] += 1;
    }
    let totaal = g.len() as f64;
    let som: f64 = hist.iter().enumerate().map(|(i, n)| i as f64 * *n as f64).sum();
    let (mut sb, mut wb, mut beste, mut drempel) = (0.0, 0.0, 0.0, 128usize);
    let mut verschil = 0.0f64;
    for (i, n) in hist.iter().enumerate() {
        wb += *n as f64;
        if wb == 0.0 {
            continue;
        }
        let wf = totaal - wb;
        if wf == 0.0 {
            break;
        }
        sb += i as f64 * *n as f64;
        let (mb, mf) = (sb / wb, (som - sb) / wf);
        let tussen = wb * wf * (mb - mf) * (mb - mf);
        if tussen > beste {
            beste = tussen;
            drempel = i;
            verschil = mf - mb;
        }
    }
    let licht = |x: usize, y: usize| g[y * bb + x] as usize > drempel;
    let papier = g.iter().filter(|v| **v as usize > drempel).count() as f64 / totaal;
    // Papier is duidelijk lichter dan de rest; is er geen groot verschil, dan is er geen brief in beeld.
    if papier < 0.05 || verschil < 50.0 {
        return Oordeel::GeenBrief;
    }
    // Hoeveel van elke rand is papier? Dan steekt de brief daar buiten beeld.
    let rand = |punten: &mut dyn Iterator<Item = (usize, usize)>| {
        let (mut l, mut n) = (0usize, 0usize);
        for (x, y) in punten {
            n += 1;
            if licht(x, y) {
                l += 1;
            }
        }
        l as f64 / n.max(1) as f64
    };
    let boven = rand(&mut (0..bb).map(|x| (x, 1)));
    let onder = rand(&mut (0..bb).map(|x| (x, bh - 2)));
    let links = rand(&mut (0..bh).map(|y| (1, y)));
    let rechts = rand(&mut (0..bh).map(|y| (bb - 2, y)));
    let grens = 0.15;
    let geraakt = [boven > grens, onder > grens, links > grens, rechts > grens];
    let aantal = geraakt.iter().filter(|x| **x).count();
    if aantal >= 3 || (links > grens && rechts > grens) || (boven > grens && onder > grens) {
        return Oordeel::Dichtbij;
    }
    // Links op de foto is jouw links (zie boven): steekt de brief daar uit, schuif hem dan naar links.
    if links > grens {
        return Oordeel::NaarLinks;
    }
    if rechts > grens {
        return Oordeel::NaarRechts;
    }
    if boven > grens {
        return Oordeel::Lager;
    }
    if onder > grens {
        return Oordeel::Hoger;
    }
    // Het papier zelf: het vak met lichte punten.
    let (mut x0, mut y0, mut x1, mut y1) = (bb, bh, 0, 0);
    for y in 0..bh {
        for x in 0..bb {
            if licht(x, y) {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
            }
        }
    }
    if x1 <= x0 + 10 || y1 <= y0 + 10 {
        return Oordeel::GeenBrief;
    }
    // Tekst: donkere puntjes op het papier, en scherpe randen (Laplace).
    let (mut donker, mut binnen, mut scherp) = (0usize, 0usize, 0f64);
    let (mut som_g, mut som_g2) = (0f64, 0f64);
    for y in (y0 + 2).max(1)..(y1.saturating_sub(2)).min(bh - 1) {
        for x in (x0 + 2).max(1)..(x1.saturating_sub(2)).min(bb - 1) {
            binnen += 1;
            let v = g[y * bb + x] as i32;
            som_g += v as f64;
            som_g2 += (v * v) as f64;
            if (v as usize) < drempel.saturating_sub(25) {
                donker += 1;
            }
            let lap = 4 * v - g[y * bb + x - 1] as i32 - g[y * bb + x + 1] as i32 - g[(y - 1) * bb + x] as i32 - g[(y + 1) * bb + x] as i32;
            scherp += (lap * lap) as f64;
        }
    }
    if binnen == 0 {
        return Oordeel::GeenBrief;
    }
    let tekst = donker as f64 / binnen as f64;
    let gem = som_g / binnen as f64;
    let spreiding = (som_g2 / binnen as f64 - gem * gem).max(0.0).sqrt();
    if tekst < 0.004 {
        // Geen duidelijke letters: wazig (wel grijze vlekken) of een lege kant (egaal).
        return if spreiding > 6.0 { Oordeel::Wazig } else { Oordeel::GeenTekst };
    }
    if scherp / binnen as f64 / tekst.max(0.01) < 900.0 {
        return Oordeel::Wazig;
    }
    Oordeel::Goed
}

/// De camera klaarzetten (de eerste die Windows kent).
fn start() -> Result<MediaCapture, String> {
    let fout = |e: windows::core::Error| format!("{}", e.message());
    let camera = MediaCapture::new().map_err(fout)?;
    let instellingen = MediaCaptureInitializationSettings::new().map_err(fout)?;
    instellingen.SetStreamingCaptureMode(StreamingCaptureMode::Video).map_err(fout)?;
    // Liever de camera aan de achterkant (tablet); anders de eerste.
    if let Ok(lijst) = windows::Devices::Enumeration::DeviceInformation::FindAllAsyncDeviceClass(windows::Devices::Enumeration::DeviceClass::VideoCapture)
        .and_then(|a| a.join())
    {
        let mut gekozen: Option<HSTRING> = None;
        for i in 0..lijst.Size().unwrap_or(0) {
            if let Ok(d) = lijst.GetAt(i) {
                let achter = d.EnclosureLocation().ok().and_then(|p| p.Panel().ok())
                    == Some(windows::Devices::Enumeration::Panel::Back);
                if gekozen.is_none() || achter {
                    gekozen = d.Id().ok();
                }
            }
        }
        match gekozen {
            Some(id) => instellingen.SetVideoDeviceId(&id).map_err(fout)?,
            None => return Err(t("CameraGeen")),
        }
    }
    camera.InitializeWithSettingsAsync(&instellingen).map_err(fout)?.join().map_err(|_| t("CameraGeen"))?;
    Ok(camera)
}

/// Eén foto als beeld.
fn foto(camera: &MediaCapture) -> Result<Beeld, String> {
    let fout = |e: windows::core::Error| format!("{}", e.message());
    let stroom = InMemoryRandomAccessStream::new().map_err(fout)?;
    camera.CapturePhotoToStreamAsync(&ImageEncodingProperties::CreateJpeg().map_err(fout)?, &stroom).map_err(fout)?.join().map_err(fout)?;
    let grootte = stroom.Size().map_err(fout)? as u32;
    let lezer = DataReader::CreateDataReader(&stroom.GetInputStreamAt(0).map_err(fout)?).map_err(fout)?;
    lezer.LoadAsync(grootte).map_err(fout)?.join().map_err(fout)?;
    let mut data = vec![0u8; grootte as usize];
    lezer.ReadBytes(&mut data).map_err(fout)?;
    let pad = std::env::temp_dir().join(format!("paperpilot-camera-{}.jpg", std::process::id()));
    std::fs::write(&pad, &data).map_err(|e| e.to_string())?;
    let b = crate::beeld::lees(&pad);
    let _ = std::fs::remove_file(&pad);
    b?.into_iter().next().ok_or_else(|| t("CameraGeen"))
}

/// Hoe het fotograferen afliep.
pub enum Uitkomst {
    /// De brief was goed in beeld: dit is de foto.
    Foto(Beeld),
    /// De gebruiker koos Annuleren.
    Gestopt,
    /// Al een tijdje geen brief (of geen tekst) in beeld; met de laatste foto, om eventueel toch te lezen.
    GeenBrief(Beeld),
}

/// Hoe lang er geen brief in beeld mag zijn voordat Paper Pilot het vraagt.
const GEEN_BRIEF_SECONDEN: u64 = 10;

/// Foto's maken tot de brief goed in beeld is (hooguit twee minuten). Zegt via de schermlezer wat je moet doen.
/// Is er een tijdje geen brief te zien (bijvoorbeeld omdat je iets anders voor de camera houdt), dan stopt hij met GeenBrief.
pub fn neem(v: &crate::dialogen::Voortgang) -> Result<Uitkomst, String> {
    use std::sync::atomic::Ordering;
    let camera = start()?;
    let zeg = |tekst: &str| {
        v.meld(tekst.to_string(), 0);
        let h = v.venster.load(Ordering::Relaxed) as windows_sys::Win32::Foundation::HWND;
        crate::melden::zeg(h, tekst);
    };
    zeg(&t("CameraBegin"));
    let begin = std::time::Instant::now();
    let mut vorige: Option<Oordeel> = None;
    let mut laatst_gezegd = std::time::Instant::now() - std::time::Duration::from_secs(10);
    let mut goed_achter_elkaar = 0;
    let mut geen_brief_sinds: Option<std::time::Instant> = None;
    while begin.elapsed().as_secs() < 120 {
        if v.gestopt.load(Ordering::Relaxed) {
            return Ok(Uitkomst::Gestopt);
        }
        let beeld = foto(&camera)?;
        let oordeel = beoordeel(&beeld);
        if matches!(oordeel, Oordeel::GeenBrief | Oordeel::GeenTekst) {
            let sinds = *geen_brief_sinds.get_or_insert_with(std::time::Instant::now);
            if sinds.elapsed().as_secs() >= GEEN_BRIEF_SECONDEN {
                crate::logboek::schrijf(&format!("Camera: {GEEN_BRIEF_SECONDEN} s geen brief in beeld ({:?}).", oordeel));
                return Ok(Uitkomst::GeenBrief(beeld));
            }
        } else {
            geen_brief_sinds = None;
        }
        if oordeel == Oordeel::Goed {
            goed_achter_elkaar += 1;
            if goed_achter_elkaar >= 2 {
                zeg(&t("CameraGenomen"));
                crate::logboek::schrijf(&format!("Camera: foto genomen na {} s.", begin.elapsed().as_secs()));
                return Ok(Uitkomst::Foto(beeld));
            }
        } else {
            goed_achter_elkaar = 0;
        }
        // Alleen iets zeggen als het anders is dan net, of na drie seconden opnieuw.
        if Some(oordeel) != vorige || laatst_gezegd.elapsed().as_secs() >= 3 {
            zeg(&t(oordeel.sleutel()));
            laatst_gezegd = std::time::Instant::now();
            vorige = Some(oordeel);
        }
    }
    Err(t("CameraTeLang"))
}
