// Paper Pilot - afbeeldingen: lezen en opslaan met WIC (Windows Imaging Component), en draaien.
//
// WIC kent JPG, PNG, BMP, GIF, TIFF (ook met meer pagina's), JPEG XR, en HEIC en WebP als de gratis extensie
// van Windows er is. Een foto van een telefoon die "op zijn kant" is opgeslagen (EXIF-stand) wordt eerst goed gezet.
// Intern is een afbeelding altijd BGRA, 8 bits per kleur, van boven naar beneden.

use std::path::Path;
use windows::core::HSTRING;
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows::Win32::Foundation::GENERIC_READ;
use windows::Win32::Graphics::Imaging::*;
use windows::Win32::System::Com::*;

#[derive(Clone)]
pub struct Beeld {
    pub breed: u32,
    pub hoog: u32,
    /// BGRA, rij voor rij.
    pub punten: Vec<u8>,
}

pub const EXTENSIES: [&str; 13] = ["jpg", "jpeg", "png", "bmp", "gif", "tif", "tiff", "webp", "heic", "heif", "jxr", "wdp", "jfif"];

pub fn is_afbeelding(pad: &Path) -> bool {
    let ext = pad.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    EXTENSIES.contains(&ext.as_str())
}

/// COM klaarzetten op deze draad (nodig voor WIC en de tekstherkenning).
pub fn com_start() {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
}

fn fabriek() -> Result<IWICImagingFactory, String> {
    com_start();
    unsafe { CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER) }.map_err(|e| format!("WIC: {e}"))
}

impl Beeld {
    pub fn nieuw(breed: u32, hoog: u32) -> Beeld {
        Beeld { breed, hoog, punten: vec![255; (breed * hoog * 4) as usize] }
    }

    fn punt(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * self.breed + x) * 4) as usize;
        [self.punten[i], self.punten[i + 1], self.punten[i + 2], self.punten[i + 3]]
    }

    /// Een kwart, half of driekwart slag met de klok mee.
    pub fn kwartslag(&self, kwarten: u32) -> Beeld {
        let k = kwarten % 4;
        if k == 0 {
            return self.clone();
        }
        let (b, h) = if k == 2 { (self.breed, self.hoog) } else { (self.hoog, self.breed) };
        let mut uit = Beeld::nieuw(b, h);
        for y in 0..self.hoog {
            for x in 0..self.breed {
                let (nx, ny) = match k {
                    1 => (self.hoog - 1 - y, x),
                    2 => (self.breed - 1 - x, self.hoog - 1 - y),
                    _ => (y, self.breed - 1 - x),
                };
                let i = ((ny * b + nx) * 4) as usize;
                uit.punten[i..i + 4].copy_from_slice(&self.punt(x, y));
            }
        }
        uit
    }

    /// Draaien over een kleine hoek (graden, met de klok mee), met een witte rand.
    pub fn draai(&self, graden: f64) -> Beeld {
        if graden.abs() < 0.05 {
            return self.clone();
        }
        let (s, c) = graden.to_radians().sin_cos();
        let (b, h) = (self.breed as f64, self.hoog as f64);
        let (cx, cy) = (b / 2.0, h / 2.0);
        let mut uit = Beeld::nieuw(self.breed, self.hoog);
        for y in 0..self.hoog {
            for x in 0..self.breed {
                // Terug naar de plek in het origineel.
                let dx = x as f64 - cx;
                let dy = y as f64 - cy;
                let ox = c * dx + s * dy + cx;
                let oy = -s * dx + c * dy + cy;
                if ox >= 0.0 && oy >= 0.0 && ox < b - 1.0 && oy < h - 1.0 {
                    let (x0, y0) = (ox as u32, oy as u32);
                    let (fx, fy) = (ox - x0 as f64, oy - y0 as f64);
                    let p = [self.punt(x0, y0), self.punt(x0 + 1, y0), self.punt(x0, y0 + 1), self.punt(x0 + 1, y0 + 1)];
                    let i = ((y * self.breed + x) * 4) as usize;
                    for k in 0..4 {
                        let v = p[0][k] as f64 * (1.0 - fx) * (1.0 - fy) + p[1][k] as f64 * fx * (1.0 - fy)
                            + p[2][k] as f64 * (1.0 - fx) * fy + p[3][k] as f64 * fx * fy;
                        uit.punten[i + k] = v.round() as u8;
                    }
                }
            }
        }
        uit
    }

    /// Twee keer zo groot (voor kleine letters), met zachte overgangen.
    pub fn vergroot2(&self) -> Beeld {
        let (b, h) = (self.breed * 2, self.hoog * 2);
        let mut uit = Beeld::nieuw(b, h);
        for y in 0..h {
            for x in 0..b {
                let (sx, sy) = (x as f32 / 2.0, y as f32 / 2.0);
                let (x0, y0) = ((sx as u32).min(self.breed - 1), (sy as u32).min(self.hoog - 1));
                let (x1, y1) = ((x0 + 1).min(self.breed - 1), (y0 + 1).min(self.hoog - 1));
                let (fx, fy) = (sx - x0 as f32, sy - y0 as f32);
                let p = [self.punt(x0, y0), self.punt(x1, y0), self.punt(x0, y1), self.punt(x1, y1)];
                let i = ((y * b + x) * 4) as usize;
                for k in 0..4 {
                    let v = p[0][k] as f32 * (1.0 - fx) * (1.0 - fy) + p[1][k] as f32 * fx * (1.0 - fy)
                        + p[2][k] as f32 * (1.0 - fx) * fy + p[3][k] as f32 * fx * fy;
                    uit.punten[i + k] = v.round() as u8;
                }
            }
        }
        uit
    }

    /// Grijs en met meer contrast: het donkerste deel wordt zwart, het lichtste wit.
    /// Helpt bij gekleurde of glanzende etiketten en bij een envelopvenster.
    pub fn contrast(&self) -> Beeld {
        let grijs: Vec<u8> = self.punten.chunks_exact(4)
            .map(|p| ((p[0] as u32 * 29 + p[1] as u32 * 150 + p[2] as u32 * 77) >> 8) as u8).collect();
        let mut telling = [0usize; 256];
        for g in &grijs {
            telling[*g as usize] += 1;
        }
        let totaal = grijs.len().max(1);
        let grens = |deel: usize| {
            let mut som = 0;
            for (i, n) in telling.iter().enumerate() {
                som += n;
                if som * 100 >= totaal * deel {
                    return i as f32;
                }
            }
            255.0
        };
        let (laag, hoog) = (grens(2), grens(60).max(grens(2) + 1.0));
        let mut uit = Beeld::nieuw(self.breed, self.hoog);
        for (i, g) in grijs.iter().enumerate() {
            let v = (((*g as f32 - laag) / (hoog - laag)).clamp(0.0, 1.0) * 255.0) as u8;
            uit.punten[i * 4..i * 4 + 3].copy_from_slice(&[v, v, v]);
        }
        uit
    }

    /// Kleiner maken zodat de langste kant hooguit `max` beeldpunten is.
    pub fn verklein(&self, max: u32) -> Beeld {
        let langst = self.breed.max(self.hoog);
        if langst <= max {
            return self.clone();
        }
        let f = max as f64 / langst as f64;
        let (b, h) = (((self.breed as f64 * f) as u32).max(1), ((self.hoog as f64 * f) as u32).max(1));
        let mut uit = Beeld::nieuw(b, h);
        for y in 0..h {
            for x in 0..b {
                let p = self.punt(((x as f64 / f) as u32).min(self.breed - 1), ((y as f64 / f) as u32).min(self.hoog - 1));
                let i = ((y * b + x) * 4) as usize;
                uit.punten[i..i + 4].copy_from_slice(&p);
            }
        }
        uit
    }
}

/// De EXIF-stand van een foto (1 = goed, 3 = op zijn kop, 6 = kwartslag rechtsom, 8 = linksom).
fn exif_stand(frame: &IWICBitmapFrameDecode) -> u16 {
    unsafe {
        let Ok(lezer) = frame.GetMetadataQueryReader() else { return 1 };
        for naam in ["/app1/ifd/{ushort=274}", "/ifd/{ushort=274}"] {
            let mut waarde = PROPVARIANT::default();
            if lezer.GetMetadataByName(&HSTRING::from(naam), &mut waarde).is_ok() {
                let binnen = &waarde.Anonymous.Anonymous;
                // VT_UI2 = 18: de stand staat als 16-bits getal in de EXIF-gegevens.
                if binnen.vt.0 == 18 {
                    return binnen.Anonymous.uiVal;
                }
            }
        }
        1
    }
}

/// Leest alle pagina's (frames) van een afbeelding.
pub fn lees(pad: &Path) -> Result<Vec<Beeld>, String> {
    let f = fabriek()?;
    unsafe {
        let decoder = f
            .CreateDecoderFromFilename(&HSTRING::from(pad.as_os_str()), None, GENERIC_READ, WICDecodeMetadataCacheOnDemand)
            .map_err(|e| {
                crate::logboek::schrijf(&format!("Afbeelding openen ({}): {e}", pad.display()));
                crate::teksten::t("BeeldKanNiet")
            })?;
        let aantal = decoder.GetFrameCount().map_err(|e| e.to_string())?;
        let mut uit = Vec::new();
        for i in 0..aantal.min(200) {
            let frame = decoder.GetFrame(i).map_err(|e| e.to_string())?;
            let (mut b, mut h) = (0u32, 0u32);
            frame.GetSize(&mut b, &mut h).map_err(|e| e.to_string())?;
            let omzetter = f.CreateFormatConverter().map_err(|e| e.to_string())?;
            omzetter
                .Initialize(&frame, &GUID_WICPixelFormat32bppBGRA, WICBitmapDitherTypeNone, None, 0.0, WICBitmapPaletteTypeCustom)
                .map_err(|e| e.to_string())?;
            let mut punten = vec![0u8; (b * h * 4) as usize];
            omzetter.CopyPixels(std::ptr::null(), b * 4, &mut punten).map_err(|e| e.to_string())?;
            // Doorzichtige delen worden wit (anders worden ze zwart voor de tekstherkenning).
            for p in punten.chunks_exact_mut(4) {
                let a = p[3] as u32;
                if a < 255 {
                    for k in 0..3 {
                        p[k] = ((p[k] as u32 * a + 255 * (255 - a)) / 255) as u8;
                    }
                    p[3] = 255;
                }
            }
            let beeld = Beeld { breed: b, hoog: h, punten };
            let beeld = match exif_stand(&frame) {
                3 => beeld.kwartslag(2),
                6 => beeld.kwartslag(1),
                8 => beeld.kwartslag(3),
                _ => beeld,
            };
            uit.push(beeld);
        }
        Ok(uit)
    }
}

/// Slaat afbeeldingen op; de soort volgt uit de extensie. Een TIFF krijgt alle pagina's, andere soorten de eerste.
pub fn schrijf(pad: &Path, beelden: &[Beeld]) -> Result<(), String> {
    let f = fabriek()?;
    let ext = pad.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let (soort, meer) = match ext.as_str() {
        "png" => (GUID_ContainerFormatPng, false),
        "jpg" | "jpeg" | "jfif" => (GUID_ContainerFormatJpeg, false),
        "bmp" => (GUID_ContainerFormatBmp, false),
        "gif" => (GUID_ContainerFormatGif, false),
        "tif" | "tiff" => (GUID_ContainerFormatTiff, true),
        "jxr" | "wdp" => (GUID_ContainerFormatWmp, false),
        _ => return Err(crate::teksten::t("BeeldSoortOnbekend")),
    };
    unsafe {
        let stroom = f.CreateStream().map_err(|e| e.to_string())?;
        stroom.InitializeFromFilename(&HSTRING::from(pad.as_os_str()), 0x4000_0000).map_err(|e| e.to_string())?;
        let encoder = f.CreateEncoder(&soort, std::ptr::null()).map_err(|e| e.to_string())?;
        encoder.Initialize(&stroom, WICBitmapEncoderNoCache).map_err(|e| e.to_string())?;
        let aantal = if meer { beelden.len() } else { beelden.len().min(1) };
        for beeld in &beelden[..aantal] {
            let mut frame = None;
            let mut eigenschappen = None;
            encoder.CreateNewFrame(&mut frame, &mut eigenschappen).map_err(|e| e.to_string())?;
            let frame = frame.ok_or("geen frame")?;
            frame.Initialize(eigenschappen.as_ref()).map_err(|e| e.to_string())?;
            frame.SetSize(beeld.breed, beeld.hoog).map_err(|e| e.to_string())?;
            let bron = f
                .CreateBitmapFromMemory(beeld.breed, beeld.hoog, &GUID_WICPixelFormat32bppBGRA, beeld.breed * 4, &beeld.punten)
                .map_err(|e| e.to_string())?;
            frame.WriteSource(&bron, std::ptr::null()).map_err(|e| e.to_string())?;
            frame.Commit().map_err(|e| e.to_string())?;
        }
        encoder.Commit().map_err(|e| e.to_string())?;
    }
    Ok(())
}
