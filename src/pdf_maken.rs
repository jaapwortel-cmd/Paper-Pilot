// Paper Pilot - een PDF maken van gescande pagina's, met de herkende tekst onzichtbaar erin.
// Zo is de PDF ook later te doorzoeken en voor te lezen (net als bij een scanner die "doorzoekbare PDF" maakt).
// Een kleine, eigen PDF-schrijver: elke pagina is een JPG-afbeelding met daarover onzichtbare tekst (Tr 3).

use crate::beeld::Beeld;
use crate::herkennen::Regel;
use std::path::Path;

/// Tekst naar WinAnsi (de standaardletters van een PDF), met \ ( en ) beschermd.
fn pdf_tekst(s: &str) -> Vec<u8> {
    let mut uit = Vec::new();
    for c in s.chars() {
        let b: u8 = match c {
            '\\' | '(' | ')' => {
                uit.push(b'\\');
                c as u8
            }
            '€' => 0x80,
            '‘' => 0x91,
            '’' => 0x92,
            '“' => 0x93,
            '”' => 0x94,
            '–' => 0x96,
            '—' => 0x97,
            c if (c as u32) < 0x20 => b' ',
            c if (c as u32) < 0x100 => c as u8,
            _ => b'?',
        };
        uit.push(b);
    }
    uit
}

/// `zichtbaar`: per pagina tekst die echt te zien moet zijn (ingevulde antwoorden): (tekst, x, onderkant, grootte) in beeldpunten.
pub fn schrijf(pad: &Path, beelden: &[Beeld], regels: &[Vec<Regel>], zichtbaar: &[Vec<(String, f32, f32, f32)>]) -> Result<(), String> {
    let tijdelijk = std::env::temp_dir().join("Paper Pilot");
    let _ = std::fs::create_dir_all(&tijdelijk);
    let mut objecten: Vec<Vec<u8>> = Vec::new(); // object n staat op plaats n-1
    // 1 = catalogus, 2 = pagina's, 3 = lettertype; daarna per pagina: pagina, afbeelding, inhoud.
    let mut kinderen = Vec::new();
    for (i, beeld) in beelden.iter().enumerate() {
        let jpg = tijdelijk.join(format!("pdf-{}-{i}.jpg", std::process::id()));
        crate::beeld::schrijf(&jpg, std::slice::from_ref(beeld))?;
        let data = std::fs::read(&jpg).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_file(&jpg);
        // Paginagrootte: A4-breedte (595 punten), de hoogte volgt de afbeelding.
        let b = 595.0f32;
        let schaal = b / beeld.breed as f32;
        let h = beeld.hoog as f32 * schaal;
        let n = 4 + i * 3;
        kinderen.push(format!("{n} 0 R"));
        objecten.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {b:.2} {h:.2}] /Resources << /XObject << /Im{i} {} 0 R >> /Font << /F1 3 0 R >> >> /Contents {} 0 R >>",
            n + 1, n + 2
        ).into_bytes());
        let mut afb = format!(
            "<< /Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode /Length {} >>\nstream\n",
            beeld.breed, beeld.hoog, data.len()
        ).into_bytes();
        afb.extend_from_slice(&data);
        afb.extend_from_slice(b"\nendstream");
        objecten.push(afb);
        let mut inhoud = format!("q {b:.2} 0 0 {h:.2} 0 0 cm /Im{i} Do Q\n").into_bytes();
        for r in regels.get(i).map(|v| v.as_slice()).unwrap_or(&[]) {
            let tekst = r.tekst.trim();
            if tekst.is_empty() {
                continue;
            }
            let grootte = (r.hoogte * schaal * 0.85).max(4.0);
            let x = r.links * schaal;
            let y = h - (r.boven + r.hoogte * 0.8) * schaal;
            // Ongeveer even breed maken als de echte regel (Helvetica: gemiddeld een halve letterhoogte per teken).
            let natuurlijk = 0.5 * grootte * tekst.chars().count() as f32;
            let rek = (100.0 * r.breedte * schaal / natuurlijk.max(1.0)).clamp(10.0, 500.0);
            let mut regel = Vec::new();
            regel.extend_from_slice(format!("BT 3 Tr /F1 {grootte:.2} Tf {rek:.1} Tz 1 0 0 1 {x:.2} {y:.2} Tm (").as_bytes());
            regel.extend_from_slice(&pdf_tekst(tekst));
            regel.extend_from_slice(b") Tj ET\n");
            inhoud.extend_from_slice(&regel);
        }
        for (tekst, x, onder, grootte) in zichtbaar.get(i).map(|v| v.as_slice()).unwrap_or(&[]) {
            let mut regel = format!("BT 0 Tr 0 g /F1 {:.2} Tf 1 0 0 1 {:.2} {:.2} Tm (", grootte * schaal, x * schaal, h - onder * schaal).into_bytes();
            regel.extend_from_slice(&pdf_tekst(tekst));
            regel.extend_from_slice(b") Tj ET\n");
            inhoud.extend_from_slice(&regel);
        }
        let mut stroom = format!("<< /Length {} >>\nstream\n", inhoud.len()).into_bytes();
        stroom.extend_from_slice(&inhoud);
        stroom.extend_from_slice(b"\nendstream");
        objecten.push(stroom);
    }
    let mut alle: Vec<Vec<u8>> = vec![
        // Taal en titel: zo is ook de PDF van Paper Pilot zelf goed voor een schermlezer.
        format!("<< /Type /Catalog /Pages 2 0 R /Lang ({}) /ViewerPreferences << /DisplayDocTitle true >> >>",
            if crate::teksten::taal() == "en" { "en-GB" } else { "nl-NL" }).into_bytes(),
        format!("<< /Type /Pages /Kids [{}] /Count {} >>", kinderen.join(" "), beelden.len()).into_bytes(),
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".to_vec(),
    ];
    alle.extend(objecten);
    // Documentinformatie met de titel (de bestandsnaam zonder .pdf), als UTF-16 met BOM.
    // Liever de eerste regel van de brief (de schermlezer noemt de titel als eerste) dan de bestandsnaam.
    let eerste = regels.first().and_then(|p| p.iter().map(|r| r.tekst.trim().to_string())
        .find(|t| t.chars().filter(|c| c.is_alphabetic()).count() >= 3)).map(|t| t.chars().take(80).collect::<String>());
    let titel = eerste.unwrap_or_else(|| pad.file_stem().map(|x| x.to_string_lossy().to_string()).unwrap_or_default());
    let mut hex = String::from("FEFF");
    for c in titel.encode_utf16() {
        hex.push_str(&format!("{c:04X}"));
    }
    alle.push(format!("<< /Title <{hex}> /Creator (Paper Pilot) /Producer (Paper Pilot) >>").into_bytes());
    let info = alle.len();
    let mut pdf = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut plaatsen = Vec::new();
    for (i, o) in alle.iter().enumerate() {
        plaatsen.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        pdf.extend_from_slice(o);
        pdf.extend_from_slice(b"\nendobj\n");
    }
    let xref = pdf.len();
    pdf.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", alle.len() + 1).as_bytes());
    for p in plaatsen {
        pdf.extend_from_slice(format!("{p:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R /Info {info} 0 R >>\nstartxref\n{xref}\n%%EOF\n", alle.len() + 1).as_bytes());
    std::fs::write(pad, pdf).map_err(|e| e.to_string())
}
