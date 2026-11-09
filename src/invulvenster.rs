// Paper Pilot - een PDF-formulier invullen, vraag voor vraag.
//
// Bovenaan "Vraag 3 van 12", daaronder de vraag (de naam van het invulvak voor de schermlezer) en het invulvak:
// een gewoon tekstvak (ook om in te dicteren), een selectievakje, of een lijst met keuzes.
// Enter = Volgende, Escape = Sluiten. Knoppen: Volgende, Vorige, Overzicht (Ctrl+L), Opslaan (Ctrl+S), Sluiten.
// Bij vragen als naam, adres of BSN staat Mijn gegevens al ingevuld (als het veld nog leeg is).

use crate::formulier_pdf::{Soort, Veld};
use crate::teksten::{t, tf};
use crate::win::*;
use crate::{dialogen, keuzevensters, logboek};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::UI::Controls::*;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const KLASSE: &str = "PaperPilotInvullen";
const ID_VOLGENDE: i32 = IDOK;
const ID_SLUITEN: i32 = IDCANCEL;
const ID_VORIGE: i32 = 201;
const ID_OVERZICHT: i32 = 202;
const ID_OPSLAAN: i32 = 203;
const ID_INVOER: i32 = 210;
const ID_PAGINA_VOLGENDE: i32 = 204;
const ID_PAGINA_VORIGE: i32 = 205;

/// Waar het formulier vandaan komt, en hoe het wordt opgeslagen.
#[derive(Clone)]
pub enum Bron {
    /// Een PDF met echte invulvelden.
    Velden(PathBuf),
    /// Een PDF met tekst maar zonder invulvelden: de plekken heeft Paper Pilot zelf gevonden.
    PlatPdf(PathBuf, Vec<crate::plat::Plek>),
    /// Een scan of foto van een formulier.
    PlatScan(Vec<crate::beeld::Beeld>, Vec<Vec<crate::herkennen::Regel>>, Vec<crate::plat::Plek>),
}

#[derive(Clone)]
struct Staat {
    hwnd: HWND,
    kop: HWND,
    voortgang: HWND,
    label: HWND,
    invoer: HWND,
    hint: HWND,
    knoppen: Vec<HWND>,
    letter: HFONT,
    kopletter: HFONT,
    kwast: HBRUSH,
    toetsen: HACCEL,
    velden: Vec<Veld>,
    /// Bij elk veld: kwam de waarde uit Mijn gegevens?
    voorgesteld: Vec<bool>,
    nr: usize,
    /// De velden die Paper Pilot vraagt (nummers in `velden`, op volgorde). Bij slim invullen niet alles.
    volgorde: Vec<usize>,
    bron: Bron,
    naam: String,
    gewijzigd: bool,
}

thread_local! {
    static STAAT: RefCell<Option<Staat>> = const { RefCell::new(None) };
}

fn lees() -> Option<Staat> {
    STAAT.with(|s| s.borrow().clone())
}

fn met<R>(f: impl FnOnce(&mut Staat) -> R) -> Option<R> {
    STAAT.with(|s| s.borrow_mut().as_mut().map(f))
}

pub fn hwnd() -> HWND {
    lees().map(|s| s.hwnd).unwrap_or(std::ptr::null_mut())
}

pub fn toetsen() -> HACCEL {
    lees().map(|s| s.toetsen).unwrap_or(std::ptr::null_mut())
}

/// Opent het invulvenster voor dit PDF-formulier.
pub fn open(eigenaar: HWND, bron: &Path, naam: &str) {
    if let Some(s) = lees() {
        unsafe { SetForegroundWindow(s.hwnd) };
        return;
    }
    let pad = bron.to_path_buf();
    let velden = dialogen::werk(eigenaar, &t("InvullenTitel"), &t("InvullenZoeken"), move |_| crate::formulier_pdf::lees_velden(&pad));
    let mut velden = match velden {
        Some(Ok(v)) if !v.is_empty() => v,
        Some(Ok(_)) => {
            dialogen::melding(eigenaar, &t("GeenFormulier"), &t("GeenFormulierTekst"), dialogen::ICOON_INFO);
            return;
        }
        Some(Err(e)) => {
            dialogen::melding(eigenaar, &t("InvullenMislukt"), &e, dialogen::ICOON_WAARSCHUWING);
            return;
        }
        None => return,
    };
    // Mijn gegevens alvast invullen in lege velden.
    let gegevens = crate::gegevens::laad();
    let mut voorgesteld = vec![false; velden.len()];
    let heb_hand = crate::handtekening::heb();
    for (i, v) in velden.iter_mut().enumerate() {
        if v.handtekening && heb_hand && v.waarde.is_empty() {
            v.waarde = crate::handtekening::merkteken();
            voorgesteld[i] = true;
            continue;
        }
        if v.soort == Soort::Tekst && v.waarde.is_empty() {
            if let Some(w) = crate::gegevens::soort_van(&v.vraag).and_then(|s| crate::gegevens::waarde(&gegevens, s)) {
                v.waarde = w;
                voorgesteld[i] = true;
            }
        }
    }
    logboek::schrijf(&format!("Formulier invullen: {} velden, {} uit Mijn gegevens.", velden.len(), voorgesteld.iter().filter(|x| **x).count()));
    maak(eigenaar, velden, voorgesteld, Bron::Velden(bron.to_path_buf()), naam);
}

/// Opent het invulvenster voor een formulier zonder invulvelden (plekken door Paper Pilot gevonden).
pub fn open_plat(eigenaar: HWND, bron: Bron, naam: &str) {
    if let Some(s) = lees() {
        unsafe { SetForegroundWindow(s.hwnd) };
        return;
    }
    let plekken = match &bron {
        Bron::PlatPdf(_, p) | Bron::PlatScan(_, _, p) => p.clone(),
        Bron::Velden(_) => return,
    };
    let gegevens = crate::gegevens::laad();
    let mut voorgesteld = vec![false; plekken.len()];
    let mut velden: Vec<Veld> = Vec::new();
    for (i, p) in plekken.iter().enumerate() {
        let vraag = if p.vraag.is_empty() { tf("VeldNummer", &[&t("VeldZonderNaam"), &(i + 1).to_string()]) } else { p.vraag.clone() };
        let soort = if p.soort == crate::plat::Plaats::Vinkje { Soort::Vinkje } else { Soort::Tekst };
        let mut waarde = String::new();
        let handtekening = soort == Soort::Tekst && crate::handtekening::is_vraag(&vraag);
        if handtekening && crate::handtekening::heb() {
            waarde = crate::handtekening::merkteken();
            voorgesteld[i] = true;
        } else if soort == Soort::Tekst {
            if let Some(w) = crate::gegevens::soort_van(&vraag).and_then(|s| crate::gegevens::waarde(&gegevens, s)) {
                waarde = w;
                voorgesteld[i] = true;
            }
        }
        velden.push(Veld { pagina: p.pagina as i32, annots: Vec::new(), soort, naam: String::new(), vraag, opties: Vec::new(),
            meerregelig: false, verplicht: false, waarde, links: p.x, boven: p.onder, handtekening, vak: [0.0; 4], kop: String::new() });
    }
    logboek::schrijf(&format!("Formulier zonder invulvelden: {} plekken gevonden.", velden.len()));
    maak(eigenaar, velden, voorgesteld, bron, naam);
}

/// Een tabel in het formulier: regels met dezelfde vakjes naast elkaar (zoals "naam, uren, tarief" voor 20 zorgverleners).
#[derive(Debug)]
pub struct Tabel {
    pub pagina: i32,
    /// Per regel de nummers van de velden.
    pub regels: Vec<Vec<usize>>,
    /// De kolommen, met de vraag van de eerste regel.
    pub kolommen: Vec<String>,
}

/// Een blok pagina's met precies dezelfde vragen, zoals "A. Naam zorgverlener" tot en met "F. Naam zorgverlener".
#[derive(Debug)]
pub struct Blok {
    /// De pagina's (0 = eerste), op volgorde.
    pub paginas: Vec<i32>,
    /// De eerste vraag van het blok, om te zeggen waar het om gaat.
    pub begin: String,
}

/// Zoekt pagina's die direct na elkaar komen en precies dezelfde vakjes op dezelfde plekken hebben (minstens 5 vakjes).
pub fn zoek_blokken(velden: &[Veld]) -> Vec<Blok> {
    let mut paginas: BTreeMap<i32, Vec<(i32, i32, u8)>> = BTreeMap::new();
    for v in velden {
        paginas.entry(v.pagina).or_default().push(((v.links / 4.0).round() as i32, (v.boven / 4.0).round() as i32, v.soort as u8));
    }
    for k in paginas.values_mut() {
        k.sort();
    }
    let lijst: Vec<(i32, Vec<(i32, i32, u8)>)> = paginas.into_iter().collect();
    let mut uit = Vec::new();
    let mut k = 0;
    while k < lijst.len() {
        let mut eind = k + 1;
        while eind < lijst.len() && lijst[eind].0 == lijst[eind - 1].0 + 1 && lijst[eind].1 == lijst[k].1 {
            eind += 1;
        }
        if eind - k >= 2 && lijst[k].1.len() >= 5 {
            let eerste = lijst[k].0;
            let begin = velden.iter().find(|v| v.pagina == eerste).map(|v| v.vraag.clone()).unwrap_or_default();
            uit.push(Blok { paginas: lijst[k..eind].iter().map(|(p, _)| *p).collect(), begin });
        }
        k = eind;
    }
    uit
}

/// Staat er op deze pagina al iets (niet uit Mijn gegevens)?
fn pagina_gevuld(velden: &[Veld], voorgesteld: &[bool], pagina: i32) -> bool {
    velden.iter().enumerate().any(|(i, v)| v.pagina == pagina && !v.waarde.is_empty() && !voorgesteld[i])
}

/// Alleen letters, klein: "Naam zorgverlener 3" en "Naam zorgverlener 4" worden hetzelfde.
fn kaal(s: &str) -> String {
    s.chars().filter(|c| c.is_alphabetic()).flat_map(|c| c.to_lowercase()).collect()
}

/// Zoekt tabellen: minstens drie regels onder elkaar met dezelfde vakjes op dezelfde plekken, en (bijna) dezelfde vragen.
pub fn zoek_tabellen(velden: &[Veld]) -> Vec<Tabel> {
    // 1. Regels: velden op dezelfde pagina en (bijna) dezelfde hoogte.
    let mut regels: Vec<Vec<usize>> = Vec::new();
    for (i, v) in velden.iter().enumerate() {
        match regels.last_mut() {
            Some(r) if velden[r[0]].pagina == v.pagina && (velden[r[0]].boven - v.boven).abs() < 4.0 => r.push(i),
            _ => regels.push(vec![i]),
        }
    }
    let kenmerk = |r: &Vec<usize>| -> Vec<(i32, u8)> {
        r.iter().map(|&i| ((velden[i].links / 4.0).round() as i32, velden[i].soort as u8)).collect()
    };
    // 2. Opeenvolgende regels met hetzelfde kenmerk.
    let mut uit = Vec::new();
    let mut k = 0;
    while k < regels.len() {
        let mut eind = k + 1;
        while eind < regels.len() && velden[regels[eind][0]].pagina == velden[regels[k][0]].pagina && kenmerk(&regels[eind]) == kenmerk(&regels[k]) {
            eind += 1;
        }
        let aantal = eind - k;
        if aantal >= 3 {
            // Echte tabel: de vragen herhalen zich (losse vragen als "Naam", "Adres", "Woonplaats" niet).
            let gelijk = (k + 1..eind).filter(|&r| {
                regels[r].iter().zip(&regels[r - 1]).all(|(&a, &b)| kaal(&velden[a].vraag) == kaal(&velden[b].vraag))
            }).count();
            if gelijk * 10 >= (aantal - 1) * 6 {
                let mut kolommen: Vec<String> = Vec::new();
                for &i in &regels[k] {
                    let v = velden[i].vraag.trim().to_string();
                    if !v.is_empty() && !kolommen.contains(&v) {
                        kolommen.push(v);
                    }
                }
                uit.push(Tabel { pagina: velden[regels[k][0]].pagina, regels: regels[k..eind].to_vec(), kolommen });
            }
        }
        k = eind;
    }
    uit
}

/// Een korte kolomnaam uit de kop boven een kolom: "Tijdsduur In minuten" -> "Tijdsduur (In minuten)";
/// "Zorgactiviteit Wat doet de zorgverlener en wanneer? ..." -> "Zorgactiviteit" (lange uitleg eraf).
fn kolomkop(kop: &str) -> String {
    let woorden: Vec<&str> = kop.split_whitespace().collect();
    if woorden.is_empty() {
        return String::new();
    }
    // De naam loopt tot het eerste volgende woord met een hoofdletter (niet na een "/").
    let mut eind = 1;
    while eind < woorden.len() {
        let w = woorden[eind];
        let hoofdletter = w.chars().next().is_some_and(|c| c.is_uppercase());
        if hoofdletter && woorden[eind - 1] != "/" {
            break;
        }
        eind += 1;
    }
    let naam = woorden[..eind].join(" ");
    let rest = &woorden[eind..];
    if !rest.is_empty() && rest.len() <= 3 {
        format!("{naam} ({})", rest.join(" "))
    } else {
        naam
    }
}

/// Geeft elk vakje in een tabel een duidelijke vraag: "Naam zorgverlener, regel 3" (de kolom uit de eerste regel).
pub fn benoem_tabellen(velden: &mut [Veld], tabellen: &[Tabel]) {
    for tb in tabellen {
        let kolommen: Vec<String> = tb.regels[0].iter().enumerate().map(|(c, &i)| {
            // Eerst de kop boven de kolom, anders de vraag die Paper Pilot bij het eerste vakje vond.
            let k = kolomkop(&velden[i].kop);
            let v = if !k.is_empty() { k } else { velden[i].vraag.split_whitespace().collect::<Vec<_>>().join(" ") };
            if v.is_empty() { tf("VeldNummer", &[&t("Kolom"), &(c + 1).to_string()]) } else { v }
        }).collect();
        // Liep de kop van de kolom ernaast mee ("Omschrijving Bedrag" + "Bedrag per jaar")? Dat stuk eraf.
        let mut kolommen = kolommen;
        for c in 0..kolommen.len().saturating_sub(1) {
            let hier: Vec<String> = kolommen[c].split(' ').map(String::from).collect();
            let daarna: Vec<&str> = kolommen[c + 1].split(' ').collect();
            for n in (1..hier.len()).rev() {
                if n <= daarna.len() && hier[hier.len() - n..].iter().zip(&daarna[..n]).all(|(a, b)| a == b) {
                    kolommen[c] = hier[..hier.len() - n].join(" ");
                    break;
                }
            }
        }
        for (k, r) in tb.regels.iter().enumerate() {
            for (c, &i) in r.iter().enumerate() {
                velden[i].vraag = tf("TabelVak", &[&kolommen[c], &(k + 1).to_string()]);
            }
        }
    }
}

/// Het aantal regels van een tabel waar al iets in staat (tot en met de laatste gevulde regel).
fn gevulde_regels(velden: &[Veld], voorgesteld: &[bool], tb: &Tabel) -> usize {
    tb.regels.iter().rposition(|r| r.iter().any(|&i| !velden[i].waarde.is_empty() && !voorgesteld[i])).map(|p| p + 1).unwrap_or(0)
}

/// Slim invullen: per tabel alleen de eerste `aantallen[n]` regels (en regels waar al iets in staat); losse vragen die al
/// in de PDF ingevuld zijn overslaan (behalve als Paper Pilot ze uit Mijn gegevens invulde: die check je even).
pub fn slim_volgorde(velden: &[Veld], voorgesteld: &[bool], tabellen: &[Tabel], aantallen: &[usize],
    paginas_weg: &std::collections::HashSet<i32>) -> Vec<usize> {
    let mut weg: Vec<bool> = velden.iter().map(|v| paginas_weg.contains(&v.pagina)).collect();
    let mut in_tabel = vec![false; velden.len()];
    for (tb, &aantal) in tabellen.iter().zip(aantallen) {
        let houden = aantal.max(gevulde_regels(velden, voorgesteld, tb));
        for (k, r) in tb.regels.iter().enumerate() {
            for &i in r {
                in_tabel[i] = true;
                weg[i] = weg[i] || k >= houden;
            }
        }
    }
    (0..velden.len()).filter(|&i| !weg[i] && (in_tabel[i] || velden[i].waarde.is_empty() || voorgesteld[i])).collect()
}

/// Welke velden vragen? Bij een klein formulier zonder tabellen: alles. Anders mag de gebruiker kiezen:
/// slim (per tabel alleen de regels die je nodig hebt, al ingevulde vragen overslaan), alleen verplicht, of alles.
/// None = de gebruiker annuleert.
fn kies_volgorde(eigenaar: HWND, velden: &mut [Veld], voorgesteld: &[bool]) -> Option<Vec<usize>> {
    let alles: Vec<usize> = (0..velden.len()).collect();
    let tabellen = zoek_tabellen(velden);
    benoem_tabellen(velden, &tabellen);
    let blokken = zoek_blokken(velden);
    let velden = &*velden;
    if velden.len() <= 25 && tabellen.is_empty() && blokken.is_empty() {
        return Some(alles);
    }
    const SLIM: i32 = 3401;
    const VERPLICHT: i32 = 3402;
    const ALLES: i32 = 3403;
    let in_tabel: usize = tabellen.iter().map(|t| t.regels.iter().map(|r| r.len()).sum::<usize>()).sum();
    let verplicht: Vec<usize> = (0..velden.len()).filter(|&i| velden[i].verplicht || velden[i].handtekening).collect();
    let titel = t("InvullenTitel");
    let kop = tf("SlimKop", &[&velden.len().to_string()]);
    let mut tekst = if tabellen.is_empty() { t("SlimTekstZonderTabel") } else {
        tf("SlimTekst", &[&in_tabel.to_string(), &tabellen.len().to_string()])
    };
    if !blokken.is_empty() {
        let paginas: usize = blokken.iter().map(|b| b.paginas.len()).sum();
        tekst = format!("{}\n\n{tekst}", tf("SlimBlokken", &[&paginas.to_string()]));
    }
    let mut v = dialogen::Vraag::nieuw(&titel, &kop, &tekst);
    v.icoon = dialogen::ICOON_INFO;
    v.opdrachtlinks = true;
    v.knoppen.push(dialogen::Knop { id: SLIM, tekst: t("KnopSlim") });
    if !verplicht.is_empty() {
        v.knoppen.push(dialogen::Knop { id: VERPLICHT, tekst: tf("KnopAlleenVerplicht", &[&verplicht.len().to_string()]) });
    }
    v.knoppen.push(dialogen::Knop { id: ALLES, tekst: tf("KnopAlleVragen", &[&velden.len().to_string()]) });
    v.gewone_knoppen = TDCBF_CANCEL_BUTTON;
    v.standaard = SLIM;
    let keuze = v.toon(eigenaar);
    let volgorde = match keuze {
        ALLES => alles,
        VERPLICHT => verplicht,
        SLIM => {
            // Eerst de herhaalde blokken (bijvoorbeeld één pagina per zorgverlener): hoeveel heb je er nodig?
            let mut paginas_weg = std::collections::HashSet::new();
            for (n, b) in blokken.iter().enumerate() {
                let gevuld = b.paginas.iter().rposition(|&p| pagina_gevuld(velden, voorgesteld, p)).map(|x| x + 1).unwrap_or(0);
                let mut keuzes = Vec::new();
                for k in 1..=b.paginas.len() {
                    keuzes.push(if k == 1 { tf("BlokEen", &[&(b.paginas[0] + 1).to_string()]) } else {
                        tf("BlokMeer", &[&k.to_string(), &(b.paginas[0] + 1).to_string(), &(b.paginas[k - 1] + 1).to_string()]) });
                }
                let label = tf("BlokVraag", &[&(b.paginas[0] + 1).to_string(), &(b.paginas.last().unwrap() + 1).to_string(),
                    &b.paginas.len().to_string(), &b.begin]);
                let titel = tf("BlokVan", &[&(n + 1).to_string(), &blokken.len().to_string()]);
                let k = keuzevensters::kies_uit_lijst(eigenaar, &titel, &label, &keuzes, gevuld.max(1) - 1)? + 1;
                for &p in &b.paginas[k.max(gevuld)..] {
                    paginas_weg.insert(p);
                }
            }
            // Per tabel: hoeveel regels heb je nodig? (Regels met al iets erin tellen altijd mee.)
            let mut aantallen = Vec::new();
            for (n, tb) in tabellen.iter().enumerate() {
                if paginas_weg.contains(&tb.pagina) {
                    aantallen.push(0);
                    continue;
                }
                let gevuld = gevulde_regels(velden, voorgesteld, tb);
                let mut keuzes = vec![t("RegelsGeen")];
                for k in 1..=tb.regels.len() {
                    keuzes.push(if k == 1 { t("RegelsEen") } else { tf("RegelsMeer", &[&k.to_string()]) });
                }
                let kolommen: String = tb.kolommen.iter().take(6).cloned().collect::<Vec<_>>().join(", ");
                let label = tf("RegelsVraag", &[&(tb.pagina + 1).to_string(), &tb.regels.len().to_string(), &kolommen]);
                let titel = tf("TabelVan", &[&(n + 1).to_string(), &tabellen.len().to_string()]);
                aantallen.push(keuzevensters::kies_uit_lijst(eigenaar, &titel, &label, &keuzes, gevuld.max(1).min(tb.regels.len()))?);
            }
            slim_volgorde(velden, voorgesteld, &tabellen, &aantallen, &paginas_weg)
        }
        _ => return None,
    };
    logboek::schrijf(&format!("Formulier: {} velden, {} tabellen ({} vakjes), {} herhaalde blokken, keuze {keuze}, {} vragen.",
        velden.len(), tabellen.len(), in_tabel, blokken.len(), volgorde.len()));
    Some(volgorde)
}

fn maak(eigenaar: HWND, mut velden: Vec<Veld>, voorgesteld: Vec<bool>, bron: Bron, naam: &str) {
    // Groot formulier (zoals een PGB-budgetplan)? Dan eerst kiezen hoe: slim, alleen verplicht, of alles.
    let Some(volgorde) = kies_volgorde(eigenaar, &mut velden, &voorgesteld) else { return };
    if volgorde.is_empty() {
        dialogen::melding(eigenaar, &t("GeenFormulier"), &t("SlimNietsTekst"), dialogen::ICOON_INFO);
        return;
    }
    let klasse = w(KLASSE);
    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(venster_proc),
        hInstance: instantie(),
        hIcon: unsafe { LoadIconW(instantie(), 1 as _) },
        hCursor: unsafe { LoadCursorW(std::ptr::null_mut(), IDC_ARROW) },
        lpszClassName: klasse.as_ptr(),
        ..unsafe { std::mem::zeroed() }
    };
    unsafe { RegisterClassExW(&wc) };
    let titel = w(&format!("{} - {naam} - Paper Pilot", t("InvullenTitel")));
    let hwnd = unsafe {
        CreateWindowExW(0, klasse.as_ptr(), titel.as_ptr(), WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
            CW_USEDEFAULT, CW_USEDEFAULT, 900, 620, eigenaar, std::ptr::null_mut(), instantie(), std::ptr::null())
    };
    if hwnd.is_null() {
        return;
    }
    let d = dpi(hwnd);
    let kleuren = crate::hoofdvenster::kleuren();
    crate::thema::titelbalk(hwnd, kleuren.donker);
    let letter = lettertype(13, false, d, "Segoe UI");
    let kopletter = lettertype(16, true, d, "Segoe UI");
    let maak = |klasse: &str, tekst: &str, stijl: u32, id: i32| unsafe {
        let k = w(klasse);
        let tt = w(tekst);
        let c = CreateWindowExW(0, k.as_ptr(), tt.as_ptr(), WS_CHILD | WS_VISIBLE | stijl, 0, 0, 10, 10, hwnd, id as isize as _,
            instantie(), std::ptr::null());
        zet_lettertype(c, letter);
        c
    };
    let kop = maak("STATIC", &tf("InvullenKop", &[naam]), 0x80 | 0x4000, -1);
    zet_lettertype(kop, kopletter);
    let voortgang = maak("STATIC", "", 0x80, -1);
    let hint = maak("STATIC", "", 0x80, -1);
    let mut knoppen = Vec::new();
    for (id, sleutel, stijl) in [
        (ID_VOLGENDE, "KnopVolgendeVraag", BS_DEFPUSHBUTTON),
        (ID_VORIGE, "KnopVorigeVraag", BS_PUSHBUTTON),
        (ID_OVERZICHT, "KnopOverzicht", BS_PUSHBUTTON),
        (ID_OPSLAAN, "KnopFormulierOpslaan", BS_PUSHBUTTON),
        (ID_SLUITEN, "KnopSluitenEsc", BS_PUSHBUTTON),
    ] {
        knoppen.push(maak("BUTTON", &t(sleutel), WS_TABSTOP | stijl as u32, id));
    }
    let toetsen = unsafe {
        let ctrl = (FVIRTKEY | FCONTROL) as u8;
        let a = [
            ACCEL { fVirt: ctrl, key: b'S' as u16, cmd: ID_OPSLAAN as u16 },
            ACCEL { fVirt: ctrl, key: b'L' as u16, cmd: ID_OVERZICHT as u16 },
            ACCEL { fVirt: (FVIRTKEY | FALT) as u8, key: VK_LEFT, cmd: ID_VORIGE as u16 },
            ACCEL { fVirt: (FVIRTKEY | FALT) as u8, key: VK_RIGHT, cmd: ID_VOLGENDE as u16 },
            ACCEL { fVirt: ctrl, key: VK_NEXT, cmd: ID_PAGINA_VOLGENDE as u16 },
            ACCEL { fVirt: ctrl, key: VK_PRIOR, cmd: ID_PAGINA_VORIGE as u16 },
        ];
        CreateAcceleratorTableW(a.as_ptr(), a.len() as i32)
    };
    let kwast = unsafe { CreateSolidBrush(kleuren.vak) };
    STAAT.with(|s| *s.borrow_mut() = Some(Staat { hwnd, kop, voortgang, label: std::ptr::null_mut(), invoer: std::ptr::null_mut(), hint,
        knoppen, letter, kopletter, kwast, toetsen, velden, voorgesteld, nr: 0, volgorde: volgorde.clone(), bron, naam: naam.to_string(), gewijzigd: false }));
    unsafe { ShowWindow(hwnd, SW_SHOWNORMAL) };
    toon_vraag(volgorde[0]);
}

/// De waarde uit het invulvak terug in het veld zetten.
fn bewaar_huidige() {
    let Some(s) = lees() else { return };
    if s.invoer.is_null() {
        return;
    }
    let v = &s.velden[s.nr];
    let nieuw = match v.soort {
        Soort::Tekst => lees_tekst(s.invoer),
        Soort::Vinkje => {
            let aan = unsafe { SendMessageW(s.invoer, BM_GETCHECK, 0, 0) } == BST_CHECKED as isize;
            if aan { "1".into() } else { String::new() }
        }
        Soort::Rondje | Soort::Lijst => {
            let i = unsafe { SendMessageW(s.invoer, LB_GETCURSEL, 0, 0) };
            // De eerste regel is "(niets gekozen)".
            if i <= 0 { String::new() } else { (i - 1).to_string() }
        }
    };
    met(|st| {
        if st.velden[st.nr].waarde != nieuw {
            st.velden[st.nr].waarde = nieuw;
            st.voorgesteld[st.nr] = false;
            st.gewijzigd = true;
        }
    });
}

fn toon_vraag(nr: usize) {
    let Some(s) = lees() else { return };
    let nr = nr.min(s.velden.len().saturating_sub(1));
    unsafe {
        if !s.label.is_null() {
            DestroyWindow(s.label);
        }
        if !s.invoer.is_null() {
            DestroyWindow(s.invoer);
        }
    }
    let v = s.velden[nr].clone();
    let vraagtekst = if v.verplicht { tf("VraagVerplicht", &[&v.vraag]) } else { v.vraag.clone() };
    let maak = |klasse: &str, tekst: &str, stijl: u32, ex: u32| unsafe {
        let k = w(klasse);
        let tt = w(tekst);
        let c = CreateWindowExW(ex, k.as_ptr(), tt.as_ptr(), WS_CHILD | WS_VISIBLE | stijl, 0, 0, 10, 10, s.hwnd, ID_INVOER as isize as _,
            instantie(), std::ptr::null());
        zet_lettertype(c, s.letter);
        c
    };
    // Eerst de vraag, dan het invulvak: zo noemt de schermlezer de vraag als naam van het invulvak.
    let label = if v.soort == Soort::Vinkje { std::ptr::null_mut() } else { maak("STATIC", &vraagtekst, 0x80, 0) };
    let invoer = match v.soort {
        Soort::Tekst => {
            let stijl = WS_TABSTOP | if v.meerregelig { (ES_MULTILINE | ES_AUTOVSCROLL | ES_WANTRETURN) as u32 | WS_VSCROLL } else { ES_AUTOHSCROLL as u32 };
            let e = maak("EDIT", &regels(&v.waarde), stijl, WS_EX_CLIENTEDGE);
            unsafe { SendMessageW(e, EM_SETSEL, 0, -1) };
            e
        }
        Soort::Vinkje => {
            let c = maak("BUTTON", &vraagtekst, WS_TABSTOP | BS_AUTOCHECKBOX as u32 | BS_MULTILINE as u32, 0);
            unsafe { SendMessageW(c, BM_SETCHECK, if v.waarde.is_empty() { BST_UNCHECKED } else { BST_CHECKED } as usize, 0) };
            c
        }
        Soort::Rondje | Soort::Lijst => {
            let l = maak("LISTBOX", "", WS_TABSTOP | WS_VSCROLL | WS_BORDER | (LBS_NOTIFY | LBS_NOINTEGRALHEIGHT) as u32, 0);
            let mut opties = vec![t("NietsGekozen")];
            opties.extend(v.opties.iter().cloned());
            for o in &opties {
                let tt = w(o);
                unsafe { SendMessageW(l, LB_ADDSTRING, 0, tt.as_ptr() as LPARAM) };
            }
            let keuze = v.waarde.parse::<usize>().map(|k| k + 1).unwrap_or(0);
            unsafe { SendMessageW(l, LB_SETCURSEL, keuze, 0) };
            l
        }
    };
    // Vooraan in de Tab-volgorde.
    unsafe {
        if !label.is_null() {
            SetWindowPos(label, HWND_TOP, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
            SetWindowPos(invoer, label, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
        } else {
            SetWindowPos(invoer, HWND_TOP, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
        }
    }
    let plek = s.volgorde.iter().position(|&i| i == nr).map(|p| p + 1).unwrap_or(0);
    let paginas = s.velden.iter().map(|v| v.pagina).max().unwrap_or(0) + 1;
    zet_tekst(s.voortgang, &tf("VraagVanPagina", &[&plek.to_string(), &s.volgorde.len().to_string(), &(v.pagina + 1).to_string(), &paginas.to_string()]));
    let hint = if v.handtekening && crate::handtekening::is_merkteken(&v.waarde) { t("HintHandtekening") }
        else if v.handtekening && !crate::handtekening::heb() { t("HintGeenHandtekening") }
        else if s.voorgesteld[nr] { t("HintMijnGegevens") } else if v.soort == Soort::Lijst || v.soort == Soort::Rondje {
        t("HintKiezen") } else if v.soort == Soort::Vinkje { t("HintVinkje") } else { t("HintTypen") };
    zet_tekst(s.hint, &hint);
    met(|st| {
        st.nr = nr;
        st.label = label;
        st.invoer = invoer;
    });
    schik();
    unsafe { SetFocus(invoer) };
}

fn schik() {
    let Some(s) = lees() else { return };
    let mut rc: RECT = unsafe { std::mem::zeroed() };
    unsafe { GetClientRect(s.hwnd, &mut rc) };
    let d = dpi(s.hwnd);
    let m = px(20, d);
    let b = rc.right - 2 * m;
    let v = s.velden.get(s.nr);
    let groot = v.map(|v| v.meerregelig || v.soort == Soort::Lijst || v.soort == Soort::Rondje).unwrap_or(false);
    unsafe {
        MoveWindow(s.kop, m, m, b, px(34, d), 1);
        MoveWindow(s.voortgang, m, m + px(40, d), b, px(26, d), 1);
        let mut y = m + px(84, d);
        if !s.label.is_null() {
            MoveWindow(s.label, m, y, b, px(56, d), 1);
            y += px(60, d);
        }
        let h = if groot { px(180, d) } else if v.map(|v| v.soort == Soort::Vinkje).unwrap_or(false) { px(56, d) } else { px(38, d) };
        MoveWindow(s.invoer, m, y, b, h, 1);
        y += h + px(10, d);
        MoveWindow(s.hint, m, y, b, px(48, d), 1);
        let kh = px(44, d);
        let mut x = m;
        let ky = rc.bottom - m - kh;
        for k in &s.knoppen {
            let kb = px(160, d);
            MoveWindow(*k, x, ky, kb, kh, 1);
            x += kb + px(10, d);
        }
    }
}

fn volgende() {
    bewaar_huidige();
    let Some(s) = lees() else { return };
    if let Some(&i) = s.volgorde.iter().find(|&&i| i > s.nr) {
        toon_vraag(i);
    } else {
        dialogen::melding(s.hwnd, &t("LaatsteVraag"), &t("LaatsteVraagTekst"), dialogen::ICOON_INFO);
        unsafe { SetFocus(s.knoppen[3]) };
    }
}

fn vorige() {
    bewaar_huidige();
    let Some(s) = lees() else { return };
    if let Some(&i) = s.volgorde.iter().rev().find(|&&i| i < s.nr) {
        toon_vraag(i);
    } else {
        unsafe { SetFocus(s.invoer) };
    }
}

/// Ctrl+Page Down: naar de eerste vraag op de volgende pagina (de rest van deze pagina overslaan).
fn volgende_pagina() {
    bewaar_huidige();
    let Some(s) = lees() else { return };
    let pagina = s.velden[s.nr].pagina;
    if let Some(&i) = s.volgorde.iter().find(|&&i| s.velden[i].pagina > pagina) {
        toon_vraag(i);
    } else {
        dialogen::melding(s.hwnd, &t("LaatstePagina"), &t("LaatsteVraagTekst"), dialogen::ICOON_INFO);
        unsafe { SetFocus(s.invoer) };
    }
}

/// Ctrl+Page Up: naar de eerste vraag van deze pagina, of (als je daar al staat) van de pagina ervoor.
fn vorige_pagina() {
    bewaar_huidige();
    let Some(s) = lees() else { return };
    let pagina = s.velden[s.nr].pagina;
    let eerste_hier = s.volgorde.iter().copied().find(|&i| s.velden[i].pagina == pagina);
    let doel = if eerste_hier.is_some_and(|i| i < s.nr) {
        eerste_hier
    } else {
        let vorige = s.volgorde.iter().rev().map(|&i| s.velden[i].pagina).find(|&p| p < pagina);
        vorige.and_then(|p| s.volgorde.iter().copied().find(|&i| s.velden[i].pagina == p))
    };
    match doel {
        Some(i) => toon_vraag(i),
        None => unsafe {
            SetFocus(s.invoer);
        },
    }
}

fn waarde_tekst(v: &Veld) -> String {
    match v.soort {
        Soort::Tekst => if v.waarde.is_empty() { t("Leeg") } else { v.waarde.replace(['\r', '\n'], " ") },
        Soort::Vinkje => if v.waarde.is_empty() { t("NietAangevinkt") } else { t("Aangevinkt") },
        _ => v.waarde.parse::<usize>().ok().and_then(|k| v.opties.get(k).cloned()).unwrap_or_else(|| t("Leeg")),
    }
}

fn overzicht() {
    bewaar_huidige();
    let Some(s) = lees() else { return };
    let lijst: Vec<String> = s.volgorde.iter().enumerate().map(|(k, &i)| format!("{}. {}: {}", k + 1, s.velden[i].vraag, waarde_tekst(&s.velden[i]))).collect();
    let plek = s.volgorde.iter().position(|&i| i == s.nr).unwrap_or(0);
    if let Some(k) = keuzevensters::kies_uit_lijst(s.hwnd, &t("OverzichtTitel"), &t("OverzichtLabel"), &lijst, plek) {
        toon_vraag(s.volgorde[k]);
    } else {
        unsafe { SetFocus(s.invoer) };
    }
}

fn opslaan() {
    bewaar_huidige();
    let Some(s) = lees() else { return };
    let leeg: Vec<&Veld> = s.velden.iter().filter(|v| v.verplicht && v.waarde.is_empty()).collect();
    if !leeg.is_empty()
        && !dialogen::ja_nee(s.hwnd, &tf("VerplichtLeeg", &[&leeg.len().to_string()]), &t("VerplichtLeegTekst"), false)
    {
        let eerste = s.velden.iter().position(|v| v.verplicht && v.waarde.is_empty()).unwrap_or(0);
        toon_vraag(eerste);
        return;
    }
    let stam = Path::new(&s.naam).file_stem().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
    let voorstel = format!("{stam} {}.pdf", t("Ingevuld"));
    let Some(doel) = crate::leesvenster::opslaan_als_pdf(s.hwnd, &voorstel) else {
        unsafe { SetFocus(s.invoer) };
        return;
    };
    let (bron, velden, d2) = (s.bron.clone(), s.velden.clone(), doel.clone());
    let uitkomst = dialogen::werk(s.hwnd, &t("InvullenTitel"), &t("FormulierBewaren"), move |_| {
        // Waar de handtekening moet komen ("[mijn handtekening]" in het antwoord); dat antwoord zelf wordt geen tekst.
        let hand = if velden.iter().any(|v| crate::handtekening::is_merkteken(&v.waarde)) { crate::handtekening::laad() } else { None };
        let met_hand: Vec<bool> = velden.iter().map(|v| hand.is_some() && crate::handtekening::is_merkteken(&v.waarde)).collect();
        let waarden: Vec<String> = velden.iter().map(|v| if crate::handtekening::is_merkteken(&v.waarde) { String::new() } else { v.waarde.clone() }).collect();
        match bron {
            Bron::Velden(pad) => {
                let mut zonder = velden.clone();
                for (v, w) in zonder.iter_mut().zip(&waarden) {
                    v.waarde = w.clone();
                }
                crate::formulier_pdf::vul_in(&pad, &zonder, &d2)?;
                let vakken: Vec<_> = velden.iter().zip(&met_hand).filter(|(_, m)| **m).map(|(v, _)| crate::handtekening::Vak {
                    pagina: v.pagina, links: v.vak[0], onder: v.vak[1], breedte: v.vak[2] - v.vak[0], hoogte: v.vak[3] - v.vak[1],
                    veld: v.annots.first().copied() }).collect();
                match &hand { Some(h) => crate::handtekening::op_pdf(&d2, &vakken, h), None => Ok(()) }
            }
            Bron::PlatPdf(pad, plekken) => {
                let paren: Vec<_> = plekken.iter().cloned().zip(waarden).collect();
                crate::plat::vul_pdf_in(&pad, &paren, &d2)?;
                let Some(h) = &hand else { return Ok(()) };
                let hoogtes = crate::plat::paginahoogtes(&d2)?;
                let schaal = crate::plat::PDF_SCHAAL;
                let vakken: Vec<_> = plekken.iter().zip(&met_hand).filter(|(_, m)| **m).map(|(p, _)| crate::handtekening::Vak {
                    pagina: p.pagina as i32, links: p.x / schaal,
                    onder: hoogtes.get(p.pagina).copied().unwrap_or(842.0) - p.onder / schaal,
                    breedte: p.breedte / schaal, hoogte: p.hoogte / schaal, veld: None }).collect();
                crate::handtekening::op_pdf(&d2, &vakken, h)
            }
            Bron::PlatScan(mut beelden, regels, plekken) => {
                if let Some(h) = &hand {
                    for (p, _) in plekken.iter().zip(&met_hand).filter(|(_, m)| **m) {
                        if let Some(b) = beelden.get_mut(p.pagina) {
                            crate::handtekening::op_beeld(b, p.x, p.onder, p.breedte, p.hoogte, h);
                        }
                    }
                }
                let paren: Vec<_> = plekken.into_iter().zip(waarden).collect();
                let zichtbaar = crate::plat::zichtbaar(&paren, beelden.len());
                crate::pdf_maken::schrijf(&d2, &beelden, &regels, &zichtbaar)
            }
        }
    });
    match uitkomst {
        Some(Ok(())) => {
            met(|st| st.gewijzigd = false);
            logboek::schrijf("Ingevuld formulier opgeslagen.");
            dialogen::melding(s.hwnd, &t("FormulierOpgeslagen"), &tf("FormulierOpgeslagenTekst", &[&doel.to_string_lossy()]), dialogen::ICOON_INFO);
            bied_gegevens_aan(&s.velden);
            if dialogen::ja_nee(s.hwnd, &t("FormulierMailenVraag"), &t("FormulierMailenTekst"), false) {
                crate::versturen::mail_met_uitleg(s.hwnd, &doel, &stam);
            }
        }
        Some(Err(e)) => dialogen::melding(s.hwnd, &t("OpslaanMislukt"), &e, dialogen::ICOON_WAARSCHUWING),
        None => {}
    }
    unsafe { SetFocus(s.invoer) };
}

/// Na het opslaan: nieuwe of andere persoonlijke gegevens bewaren in Mijn gegevens?
fn bied_gegevens_aan(velden: &[Veld]) {
    let Some(s) = lees() else { return };
    let mut gegevens = crate::gegevens::laad();
    let mut nieuw: BTreeMap<String, String> = BTreeMap::new();
    for v in velden.iter().filter(|v| v.soort == Soort::Tekst && !v.waarde.trim().is_empty()) {
        if let Some(soort) = crate::gegevens::soort_van(&v.vraag) {
            if crate::gegevens::SOORTEN.contains(&soort) && gegevens.get(soort).map(|g| g != v.waarde.trim()).unwrap_or(true) {
                nieuw.insert(soort.to_string(), v.waarde.trim().to_string());
            }
        }
    }
    if nieuw.is_empty() {
        return;
    }
    let lijst: Vec<String> = nieuw.iter().map(|(k, v)| format!("{}: {v}", t(&format!("Gegeven_{k}")))).collect();
    if dialogen::ja_nee(s.hwnd, &t("GegevensBewarenVraag"), &tf("GegevensBewarenTekst", &[&lijst.join("\n")]), false) {
        gegevens.extend(nieuw);
        if let Err(e) = crate::gegevens::bewaar(&gegevens) {
            dialogen::melding(s.hwnd, &t("OpslaanMislukt"), &e, dialogen::ICOON_WAARSCHUWING);
        }
    }
}

fn sluiten(hwnd: HWND) {
    bewaar_huidige();
    let gewijzigd = lees().map(|s| s.gewijzigd).unwrap_or(false);
    if gewijzigd && !dialogen::ja_nee(hwnd, &t("NietOpgeslagen"), &t("NietOpgeslagenTekst"), false) {
        if let Some(s) = lees() {
            unsafe { SetFocus(s.invoer) };
        }
        return;
    }
    unsafe { DestroyWindow(hwnd) };
}

unsafe extern "system" fn venster_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_SIZE => {
            schik();
            0
        }
        WM_COMMAND => {
            let id = (wp & 0xFFFF) as i32;
            let melding = ((wp >> 16) & 0xFFFF) as u32;
            match id {
                ID_VOLGENDE => volgende(),
                ID_VORIGE => vorige(),
                ID_OVERZICHT => overzicht(),
                ID_PAGINA_VOLGENDE => volgende_pagina(),
                ID_PAGINA_VORIGE => vorige_pagina(),
                ID_OPSLAAN => opslaan(),
                ID_SLUITEN => sluiten(hwnd),
                ID_INVOER if melding == 2 => volgende(), // dubbelklik in de lijst
                _ => {}
            }
            0
        }
        WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX | WM_CTLCOLORBTN => {
            let k = crate::hoofdvenster::kleuren();
            let Some(s) = lees() else { return DefWindowProcW(hwnd, msg, wp, lp) };
            SetTextColor(wp as HDC, if lp as HWND == s.hint || lp as HWND == s.voortgang { k.zacht } else { k.tekst });
            if msg == WM_CTLCOLOREDIT || msg == WM_CTLCOLORLISTBOX {
                SetBkColor(wp as HDC, k.vak);
                return s.kwast as LRESULT;
            }
            SetBkColor(wp as HDC, k.achter);
            SetDCBrushColor(wp as HDC, k.achter);
            GetStockObject(DC_BRUSH) as LRESULT
        }
        WM_ERASEBKGND => {
            let mut rc: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut rc);
            crate::thema::vul(wp as HDC, &rc, crate::hoofdvenster::kleuren().achter);
            1
        }
        WM_ACTIVATE => {
            if (wp & 0xFFFF) as u32 != WA_INACTIVE {
                if let Some(s) = lees() {
                    let f = GetFocus();
                    if f.is_null() || IsChild(hwnd, f) == 0 {
                        SetFocus(s.invoer);
                    }
                }
            }
            0
        }
        WM_CLOSE => {
            sluiten(hwnd);
            0
        }
        WM_DESTROY => {
            if let Some(s) = lees() {
                weg(s.letter as _);
                weg(s.kopletter as _);
                weg(s.kwast as _);
                DestroyAcceleratorTable(s.toetsen);
            }
            STAAT.with(|s| *s.borrow_mut() = None);
            let lees_hwnd = crate::leesvenster::hwnd();
            if !lees_hwnd.is_null() {
                SetForegroundWindow(lees_hwnd);
            }
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}
