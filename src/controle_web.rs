// Paper Pilot - Controleren: is deze webpagina goed te gebruiken met een schermlezer?
//
// Paper Pilot haalt de HTML van de pagina op (zonder JavaScript uit te voeren) en kijkt naar: taal en titel,
// plaatjes met alt-tekst, links en knoppen met een naam, invulvelden met een label, koppen, een hoofdgedeelte (main),
// inzoomen, iframes met een titel, dubbele id's, tabindex, automatisch afspelen en kleurcontrast in de HTML zelf.

use crate::controleren::{contrast, getal, hex_kleur, Ernst, Verslag};
use crate::teksten::{t, tf};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct Label {
    pub naam: String,
    pub sluit: bool,
    pub attrs: Vec<(String, String)>,
}

impl Label {
    pub fn attr(&self, n: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| k == n).map(|(_, v)| v.as_str())
    }
    fn heeft(&self, n: &str) -> bool {
        self.attr(n).is_some()
    }
    /// Een niet-lege attribuutwaarde.
    fn waarde(&self, n: &str) -> Option<&str> {
        self.attr(n).map(|v| v.trim()).filter(|v| !v.is_empty())
    }
}

#[derive(Debug, Clone)]
pub enum Stuk {
    Label(Label),
    Tekst(String),
}

/// HTML-tekens als &amp; en &#233; omzetten.
pub fn ontcijfer(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut uit = String::new();
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        uit.push_str(&rest[..i]);
        rest = &rest[i..];
        let eind = rest.char_indices().take(12).find(|(_, c)| *c == ';').map(|(j, _)| j);
        let Some(j) = eind else {
            uit.push('&');
            rest = &rest[1..];
            continue;
        };
        let naam = &rest[1..j];
        let c = match naam {
            "amp" => Some('&'), "lt" => Some('<'), "gt" => Some('>'), "quot" => Some('"'), "apos" => Some('\''), "nbsp" => Some(' '),
            n if n.starts_with("#x") || n.starts_with("#X") => u32::from_str_radix(&n[2..], 16).ok().and_then(char::from_u32),
            n if n.starts_with('#') => n[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match c {
            Some(c) => {
                uit.push(c);
                rest = &rest[j + 1..];
            }
            None => {
                uit.push('&');
                rest = &rest[1..];
            }
        }
    }
    uit.push_str(rest);
    uit
}

/// Knipt HTML in labels en tekst. Commentaar en de inhoud van script, style en template worden overgeslagen.
pub fn stukken(html: &str) -> Vec<Stuk> {
    let mut uit = Vec::new();
    let b = html.as_bytes();
    let mut i = 0;
    let mut tekst_begin = 0;
    while i < b.len() {
        if b[i] != b'<' {
            i += 1;
            continue;
        }
        if i > tekst_begin {
            let t = ontcijfer(&html[tekst_begin..i]);
            if !t.trim().is_empty() {
                uit.push(Stuk::Tekst(t));
            }
        }
        if html[i..].starts_with("<!--") {
            i = html[i..].find("-->").map(|j| i + j + 3).unwrap_or(b.len());
            tekst_begin = i;
            continue;
        }
        if html[i..].starts_with("<!") || html[i..].starts_with("<?") {
            i = html[i..].find('>').map(|j| i + j + 1).unwrap_or(b.len());
            tekst_begin = i;
            continue;
        }
        // Het einde van het label, met aanhalingstekens erin.
        let mut j = i + 1;
        let mut quote: Option<u8> = None;
        while j < b.len() {
            match (quote, b[j]) {
                (None, b'"') | (None, b'\'') => quote = Some(b[j]),
                (Some(q), c) if c == q => quote = None,
                (None, b'>') => break,
                _ => {}
            }
            j += 1;
        }
        let binnen = &html[i + 1..j.min(b.len())];
        let sluit = binnen.starts_with('/');
        let binnen = binnen.trim_start_matches('/');
        let naam: String = binnen.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '-').collect::<String>().to_lowercase();
        i = (j + 1).min(b.len());
        tekst_begin = i;
        if naam.is_empty() {
            continue;
        }
        let attrs = if sluit { Vec::new() } else { lees_attrs(&binnen[naam.len()..]) };
        let overslaan = !sluit && matches!(naam.as_str(), "script" | "style" | "template" | "noscript");
        uit.push(Stuk::Label(Label { naam: naam.clone(), sluit, attrs }));
        if overslaan {
            let eind = format!("</{naam}");
            let rest = html[i..].to_lowercase();
            i = rest.find(&eind).map(|k| i + k).unwrap_or(b.len());
            tekst_begin = i;
        }
    }
    if tekst_begin < b.len() {
        let t = ontcijfer(&html[tekst_begin..]);
        if !t.trim().is_empty() {
            uit.push(Stuk::Tekst(t));
        }
    }
    uit
}

fn lees_attrs(s: &str) -> Vec<(String, String)> {
    let mut uit = Vec::new();
    let c: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < c.len() {
        while i < c.len() && (c[i].is_whitespace() || c[i] == '/') {
            i += 1;
        }
        let begin = i;
        while i < c.len() && !c[i].is_whitespace() && c[i] != '=' && c[i] != '/' && c[i] != '>' {
            i += 1;
        }
        if i == begin {
            i += 1;
            continue;
        }
        let naam: String = c[begin..i].iter().collect::<String>().to_lowercase();
        while i < c.len() && c[i].is_whitespace() {
            i += 1;
        }
        let mut waarde = String::new();
        if i < c.len() && c[i] == '=' {
            i += 1;
            while i < c.len() && c[i].is_whitespace() {
                i += 1;
            }
            if i < c.len() && (c[i] == '"' || c[i] == '\'') {
                let q = c[i];
                i += 1;
                let b = i;
                while i < c.len() && c[i] != q {
                    i += 1;
                }
                waarde = c[b..i.min(c.len())].iter().collect();
                i += 1;
            } else {
                let b = i;
                while i < c.len() && !c[i].is_whitespace() && c[i] != '>' {
                    i += 1;
                }
                waarde = c[b..i].iter().collect();
            }
        }
        uit.push((naam, ontcijfer(&waarde)));
    }
    uit
}

/// Een taalcode zoals "nl", "nl-NL" of "en-GB".
fn geldige_taal(s: &str) -> bool {
    let mut delen = s.split('-');
    let eerste = delen.next().unwrap_or("");
    (2..=3).contains(&eerste.len()) && eerste.chars().all(|c| c.is_ascii_alphabetic())
        && delen.all(|d| (2..=8).contains(&d.len()) && d.chars().all(|c| c.is_ascii_alphanumeric()))
}

/// Een kleur uit CSS: #abc, #aabbcc, rgb(1,2,3) of een paar bekende namen.
fn css_kleur(s: &str) -> Option<(u8, u8, u8)> {
    let s = s.trim().to_lowercase();
    let s = s.trim_end_matches("!important").trim();
    if s.starts_with('#') {
        return hex_kleur(s);
    }
    if let Some(r) = s.strip_prefix("rgb(").or_else(|| s.strip_prefix("rgba(")) {
        let d: Vec<u8> = r.trim_end_matches(')').split(',').take(3).filter_map(|x| x.trim().parse::<f32>().ok()).map(|x| x as u8).collect();
        if d.len() == 3 {
            return Some((d[0], d[1], d[2]));
        }
    }
    Some(match s {
        "white" => (255, 255, 255), "black" => (0, 0, 0), "red" => (255, 0, 0), "gray" | "grey" => (128, 128, 128),
        "silver" => (192, 192, 192), "yellow" => (255, 255, 0), "orange" => (255, 165, 0), "lightgray" | "lightgrey" => (211, 211, 211),
        "blue" => (0, 0, 255), "green" => (0, 128, 0), "navy" => (0, 0, 128),
        _ => return None,
    })
}

fn stijl(l: &Label, eigenschap: &str) -> Option<String> {
    l.attr("style")?.split(';').find_map(|d| {
        let (k, v) = d.split_once(':')?;
        if k.trim().eq_ignore_ascii_case(eigenschap) { Some(v.trim().to_string()) } else { None }
    })
}

fn kort(s: &str) -> String {
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if s.chars().count() > 60 { format!("{}…", s.chars().take(60).collect::<String>()) } else { s }
}

pub fn controleer(adres: &str) -> Result<Verslag, String> {
    let mut data = Vec::new();
    let (status, _) = crate::http::verzoek("GET", adres, "Accept: text/html\r\nAccept-Language: nl, en\r\n", None, 30000, &mut |b, _, _| {
        if data.len() < 8_000_000 {
            data.extend_from_slice(b);
        }
        Ok(())
    }).map_err(|e| tf("ControleWebNiet", &[&e]))?;
    if status >= 400 {
        return Err(tf("ControleWebStatus", &[&status.to_string()]));
    }
    let html = String::from_utf8_lossy(&data).to_string();
    Ok(controleer_html(adres, &html))
}

/// De controle zelf (los van internet, zodat de proef hem met een bestand kan doen).
pub fn controleer_html(adres: &str, html: &str) -> Verslag {
    let mut v = Verslag::nieuw(adres);
    let s = stukken(html);
    let labels: Vec<&Label> = s.iter().filter_map(|x| if let Stuk::Label(l) = x { Some(l) } else { None }).collect();
    fn open<'a>(labels: &'a [&'a Label], n: &'a str) -> impl Iterator<Item = &'a Label> + 'a {
        labels.iter().filter(move |l| !l.sluit && l.naam == n).copied()
    }

    // Taal en titel.
    match open(&labels, "html").next().and_then(|h| h.waarde("lang").map(|s| s.to_string())) {
        Some(taal) if !geldige_taal(&taal) => v.punt(Ernst::Probleem, "CWebFouteTaal", &[&taal], Vec::new()),
        Some(taal) => v.goed("CWebTaal", &[&taal]),
        None => v.punt(Ernst::Probleem, "CWebGeenTaal", &[], Vec::new()),
    }
    let mut titel = String::new();
    let mut in_titel = false;
    // Tekst per element: voor links, knoppen, labels en koppen.
    let mut teksten_tot: Vec<String> = Vec::new();
    let mut lege_links = Vec::new();
    let mut vage_links = Vec::new();
    let mut links = 0usize;
    let mut lege_knoppen = Vec::new();
    let mut knoppen = 0usize;
    let mut koppen: Vec<(u8, String)> = Vec::new();
    let mut label_voor: HashSet<String> = HashSet::new();
    let mut in_label = 0usize;
    let mut velden: Vec<(Label, bool)> = Vec::new(); // (veld, binnen een label)
    let mut ids: HashMap<String, usize> = HashMap::new();
    let mut plaatjes = 0usize;
    let mut plaatjes_zonder = Vec::new();
    let mut iframes_zonder = Vec::new();
    let mut tabindex = 0usize;
    let mut autoplay = 0usize;
    let mut laag_contrast = Vec::new();
    let mut lichaam_tekst = 0usize;
    let mut scripts = 0usize;
    // Stapel voor a/button/hN/label: (soort, begin-index in teksten_tot, label)
    let mut stapel: Vec<(String, usize, Label)> = Vec::new();
    let mut verborgen_diepte = 0usize;
    // Extra controles uit WCAG 2.2.
    let mut tabellen: Vec<bool> = Vec::new(); // open tabellen: heeft al een kopcel?
    let mut tabellen_totaal = 0usize;
    let mut tabellen_zonder_kop = 0usize;
    let mut fieldset_diepte = 0usize;
    let mut rondjes: HashMap<String, bool> = HashMap::new(); // naam -> in een groep (fieldset)
    let mut zonder_autocomplete = Vec::new();
    let mut video_open: Option<bool> = None; // Some(heeft ondertiteling)
    let mut video_zonder_ondertiteling = 0usize;
    let mut bewegend = 0usize;
    let mut verversen = false;
    let mut klik_zonder_toetsenbord = Vec::new();
    let mut verborgen_maar_focus = Vec::new();
    let mut svg_zonder_naam = 0usize;

    for stuk in &s {
        match stuk {
            Stuk::Tekst(tekst) => {
                if in_titel {
                    titel.push_str(tekst);
                }
                if verborgen_diepte == 0 {
                    lichaam_tekst += tekst.trim().len();
                }
                teksten_tot.push(tekst.clone());
            }
            Stuk::Label(l) => {
                if !l.sluit {
                    if let Some(id) = l.waarde("id") {
                        *ids.entry(id.to_string()).or_insert(0) += 1;
                    }
                    if let Some(ti) = l.waarde("tabindex").and_then(|x| x.parse::<i32>().ok()) {
                        if ti > 0 {
                            tabindex += 1;
                        }
                    }
                    // Klikbaar gemaakt met JavaScript, maar niet met het toetsenbord te bereiken.
                    if (l.naam == "div" || l.naam == "span" || l.naam == "li" || l.naam == "td") && l.heeft("onclick")
                        && l.attr("role").is_none() && l.attr("tabindex").is_none()
                    {
                        klik_zonder_toetsenbord.push(format!("<{}{}>", l.naam, l.waarde("class").map(|c| format!(" class=\"{}\"", kort(c))).unwrap_or_default()));
                    }
                    // Verborgen voor de schermlezer, maar wel te bereiken met Tab.
                    let focusbaar = (l.naam == "a" && l.heeft("href")) || matches!(l.naam.as_str(), "button" | "input" | "select" | "textarea")
                        || l.waarde("tabindex").and_then(|x| x.parse::<i32>().ok()).map(|x| x >= 0).unwrap_or(false);
                    if focusbaar && l.attr("aria-hidden") == Some("true") && l.attr("type") != Some("hidden") {
                        verborgen_maar_focus.push(format!("<{}>", l.naam));
                    }
                    // Kleurcontrast in de HTML zelf (style="color: ...; background: ...").
                    if let (Some(k), Some(a)) = (stijl(l, "color").and_then(|c| css_kleur(&c)),
                        stijl(l, "background-color").or_else(|| stijl(l, "background")).and_then(|c| css_kleur(&c)))
                    {
                        let c = contrast(k, a);
                        if c < 4.5 {
                            laag_contrast.push(format!("<{}> ({} {})", l.naam, t("ControleContrast"), getal(c)));
                        }
                    }
                }
                match (l.naam.as_str(), l.sluit) {
                    ("title", false) => in_titel = true,
                    ("title", true) => in_titel = false,
                    ("script", false) => scripts += 1,
                    ("head", false) => verborgen_diepte += 1,
                    ("head", true) => verborgen_diepte = verborgen_diepte.saturating_sub(1),
                    ("img", false) => {
                        plaatjes += 1;
                        let decoratief = matches!(l.attr("role"), Some("presentation") | Some("none")) || l.attr("aria-hidden") == Some("true");
                        if !l.heeft("alt") && !decoratief && l.waarde("aria-label").is_none() && l.waarde("aria-labelledby").is_none() {
                            plaatjes_zonder.push(l.waarde("src").map(|x| x.rsplit('/').next().unwrap_or(x).to_string()).unwrap_or_default());
                        }
                        // Een plaatje in een link of knop geeft die link zijn naam.
                        if let Some(alt) = l.waarde("alt") {
                            teksten_tot.push(alt.to_string());
                        }
                    }
                    ("svg", false) => {
                        if l.attr("role") == Some("img") && l.waarde("aria-label").is_none() && l.waarde("aria-labelledby").is_none() {
                            svg_zonder_naam += 1;
                        }
                        if let Some(n) = l.waarde("aria-label") {
                            teksten_tot.push(n.to_string());
                        }
                    }
                    ("table", false) => {
                        let opmaak = matches!(l.attr("role"), Some("presentation") | Some("none"));
                        tabellen.push(opmaak);
                        if !opmaak {
                            tabellen_totaal += 1;
                        }
                    }
                    ("th", false) => {
                        if let Some(t) = tabellen.last_mut() {
                            *t = true;
                        }
                    }
                    ("td", false) => {
                        if matches!(l.attr("scope"), Some("row") | Some("col")) {
                            if let Some(t) = tabellen.last_mut() {
                                *t = true;
                            }
                        }
                    }
                    ("table", true) => {
                        if tabellen.pop() == Some(false) {
                            tabellen_zonder_kop += 1;
                        }
                    }
                    ("fieldset", false) => fieldset_diepte += 1,
                    ("fieldset", true) => fieldset_diepte = fieldset_diepte.saturating_sub(1),
                    ("marquee", false) | ("blink", false) => bewegend += 1,
                    ("meta", false) => {
                        if l.attr("http-equiv").map(|h| h.eq_ignore_ascii_case("refresh")).unwrap_or(false)
                            && l.attr("content").and_then(|c| c.split(';').next()).and_then(|n| n.trim().parse::<f32>().ok()).map(|n| n > 0.0).unwrap_or(false)
                        {
                            verversen = true;
                        }
                    }
                    ("track", false) => {
                        if matches!(l.attr("kind"), Some("captions") | Some("subtitles")) {
                            if let Some(v) = video_open.as_mut() {
                                *v = true;
                            }
                        }
                    }
                    ("video", true) => {
                        if video_open.take() == Some(false) {
                            video_zonder_ondertiteling += 1;
                        }
                    }
                    ("iframe", false) => {
                        if l.waarde("title").is_none() && l.waarde("aria-label").is_none() && l.attr("aria-hidden") != Some("true") {
                            iframes_zonder.push(l.waarde("src").map(kort).unwrap_or_default());
                        }
                    }
                    ("video", false) | ("audio", false) => {
                        if l.naam == "video" {
                            video_open = Some(false);
                        }
                        if l.heeft("autoplay") && !l.heeft("muted") {
                            autoplay += 1;
                        }
                    }
                    ("label", false) => {
                        in_label += 1;
                        if let Some(f) = l.waarde("for") {
                            label_voor.insert(f.to_string());
                        }
                    }
                    ("label", true) => in_label = in_label.saturating_sub(1),
                    ("input", false) | ("select", false) | ("textarea", false) => {
                        let soort = l.attr("type").unwrap_or("text").to_lowercase();
                        match soort.as_str() {
                            "hidden" => {}
                            "submit" | "button" | "reset" | "image" => {
                                knoppen += 1;
                                if l.waarde("value").is_none() && l.waarde("alt").is_none() && l.waarde("aria-label").is_none() && l.waarde("title").is_none()
                                    && soort != "submit" && soort != "reset"
                                {
                                    lege_knoppen.push(format!("<input type=\"{soort}\">"));
                                }
                            }
                            _ => {
                                if soort == "radio" {
                                    if let Some(n) = l.waarde("name") {
                                        let in_groep = fieldset_diepte > 0;
                                        let e = rondjes.entry(n.to_string()).or_insert(in_groep);
                                        *e &= in_groep;
                                    }
                                }
                                // Persoonlijke gegevens: met autocomplete kan de browser ze invullen (WCAG 1.3.5).
                                let sleutel = format!("{} {} {}", l.attr("name").unwrap_or(""), l.attr("id").unwrap_or(""), soort).to_lowercase();
                                let persoonlijk = ["email", "e-mail", "tel", "phone", "telefoon", "naam", "name", "postcode", "zip", "adres", "address", "straat", "street", "woonplaats", "city"]
                                    .iter().any(|w| sleutel.contains(w)) && !sleutel.contains("search") && !sleutel.contains("zoek");
                                if persoonlijk && l.attr("autocomplete").is_none() && l.naam == "input" {
                                    zonder_autocomplete.push(format!("<input {}>", kort(l.waarde("name").or(l.waarde("id")).unwrap_or(&soort))));
                                }
                                velden.push(((*l).clone(), in_label > 0))
                            }
                        }
                    }
                    ("a", false) if l.heeft("href") => stapel.push(("a".into(), teksten_tot.len(), (*l).clone())),
                    ("button", false) => stapel.push(("button".into(), teksten_tot.len(), (*l).clone())),
                    (h, false) if h.len() == 2 && h.starts_with('h') && h.as_bytes()[1].is_ascii_digit() => {
                        stapel.push((h.to_string(), teksten_tot.len(), (*l).clone()));
                    }
                    (n, true) if n == "a" || n == "button" || (n.len() == 2 && n.starts_with('h') && n.as_bytes()[1].is_ascii_digit()) => {
                        if let Some(pos) = stapel.iter().rposition(|(s, _, _)| s == n) {
                            let (soort, begin, lab) = stapel.remove(pos);
                            let tekst = teksten_tot[begin.min(teksten_tot.len())..].join(" ").split_whitespace().collect::<Vec<_>>().join(" ");
                            let naam = lab.waarde("aria-label").map(|s| s.to_string()).or(lab.waarde("title").map(|s| s.to_string()))
                                .or(if lab.waarde("aria-labelledby").is_some() { Some("…".into()) } else { None }).unwrap_or(tekst.clone());
                            match soort.as_str() {
                                "a" => {
                                    links += 1;
                                    if naam.trim().is_empty() {
                                        lege_links.push(lab.waarde("href").map(kort).unwrap_or_default());
                                    } else if ["klik hier", "hier", "click here", "here", "lees meer", "meer", "read more", "more", "meer info", "link"]
                                        .contains(&naam.trim().to_lowercase().as_str())
                                    {
                                        vage_links.push(format!("\"{}\" → {}", kort(&naam), lab.waarde("href").map(kort).unwrap_or_default()));
                                    }
                                }
                                "button" => {
                                    knoppen += 1;
                                    if naam.trim().is_empty() {
                                        lege_knoppen.push(lab.waarde("class").map(|c| format!("<button class=\"{}\">", kort(c))).unwrap_or_else(|| "<button>".into()));
                                    }
                                }
                                h => koppen.push((h.as_bytes()[1] - b'0', tekst)),
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    let titel = titel.split_whitespace().collect::<Vec<_>>().join(" ");
    if titel.is_empty() {
        v.punt(Ernst::Probleem, "CWebGeenTitel", &[], Vec::new());
    } else {
        v.goed("CWebTitel", &[&titel]);
    }
    // Plaatjes.
    if !plaatjes_zonder.is_empty() {
        v.punt(Ernst::Probleem, "CWebPlaatjesZonder", &[&plaatjes_zonder.len().to_string(), &plaatjes.to_string()], plaatjes_zonder);
    } else if plaatjes > 0 {
        v.goed("CWebPlaatjesGoed", &[&plaatjes.to_string()]);
    }
    // Links en knoppen.
    if !lege_links.is_empty() {
        v.punt(Ernst::Probleem, "CWebLegeLinks", &[&lege_links.len().to_string()], lege_links);
    } else if links > 0 {
        v.goed("CWebLinksGoed", &[&links.to_string()]);
    }
    if !vage_links.is_empty() {
        v.punt(Ernst::LetOp, "CWebVageLinks", &[&vage_links.len().to_string()], vage_links);
    }
    if !lege_knoppen.is_empty() {
        v.punt(Ernst::Probleem, "CWebLegeKnoppen", &[&lege_knoppen.len().to_string()], lege_knoppen);
    } else if knoppen > 0 {
        v.goed("CWebKnoppenGoed", &[&knoppen.to_string()]);
    }
    // Invulvelden met een label.
    let mut zonder_label = Vec::new();
    let mut alleen_placeholder = 0;
    for (l, binnen) in &velden {
        let heeft = *binnen || l.waarde("aria-label").is_some() || l.waarde("aria-labelledby").is_some() || l.waarde("title").is_some()
            || l.waarde("id").map(|id| label_voor.contains(id)).unwrap_or(false);
        if !heeft {
            if l.waarde("placeholder").is_some() {
                alleen_placeholder += 1;
            }
            let wat = l.waarde("name").or(l.waarde("id")).or(l.waarde("placeholder")).unwrap_or("");
            zonder_label.push(format!("<{} {}>", l.naam, kort(wat)));
        }
    }
    if !zonder_label.is_empty() {
        v.punt(Ernst::Probleem, "CWebVeldenZonder", &[&zonder_label.len().to_string(), &velden.len().to_string()], zonder_label);
        if alleen_placeholder > 0 {
            v.punt(Ernst::LetOp, "CWebPlaceholder", &[&alleen_placeholder.to_string()], Vec::new());
        }
    } else if !velden.is_empty() {
        v.goed("CWebVeldenGoed", &[&velden.len().to_string()]);
    }
    // Koppen.
    if koppen.is_empty() {
        v.punt(Ernst::Probleem, "CWebGeenKoppen", &[], Vec::new());
    } else {
        let h1 = koppen.iter().filter(|(n, _)| *n == 1).count();
        if h1 == 0 {
            v.punt(Ernst::Probleem, "CWebGeenH1", &[], Vec::new());
        } else {
            v.goed("CWebKoppen", &[&koppen.len().to_string(), &h1.to_string()]);
        }
        let mut gaten = Vec::new();
        let mut vorig = 0u8;
        for (n, tekst) in &koppen {
            if vorig > 0 && *n > vorig + 1 {
                gaten.push(format!("h{n}: {}", kort(tekst)));
            }
            vorig = *n;
        }
        if !gaten.is_empty() {
            v.punt(Ernst::LetOp, "CWebKopNiveaus", &[], gaten);
        }
        let leeg: Vec<String> = koppen.iter().filter(|(_, t)| t.trim().is_empty()).map(|(n, _)| format!("h{n}")).collect();
        if !leeg.is_empty() {
            v.punt(Ernst::Probleem, "CWebLegeKoppen", &[&leeg.len().to_string()], leeg);
        }
    }
    // Tabellen met kopcellen.
    if tabellen_zonder_kop > 0 {
        v.punt(Ernst::Probleem, "CWebTabelZonderKop", &[&tabellen_zonder_kop.to_string(), &tabellen_totaal.to_string()], Vec::new());
    } else if tabellen_totaal > 0 {
        v.goed("CWebTabellenGoed", &[&tabellen_totaal.to_string()]);
    }
    let los: Vec<String> = rondjes.iter().filter(|(_, g)| !**g).map(|(n, _)| format!("name=\"{n}\"")).collect();
    if !los.is_empty() {
        v.punt(Ernst::LetOp, "CWebRondjesGroep", &[&los.len().to_string()], los);
    }
    if !zonder_autocomplete.is_empty() {
        v.punt(Ernst::LetOp, "CWebAutocomplete", &[&zonder_autocomplete.len().to_string()], zonder_autocomplete);
    }
    if video_zonder_ondertiteling > 0 {
        v.punt(Ernst::LetOp, "CWebOndertiteling", &[&video_zonder_ondertiteling.to_string()], Vec::new());
    }
    if bewegend > 0 {
        v.punt(Ernst::Probleem, "CWebBewegend", &[], Vec::new());
    }
    if verversen {
        v.punt(Ernst::Probleem, "CWebVerversen", &[], Vec::new());
    }
    if !klik_zonder_toetsenbord.is_empty() {
        v.punt(Ernst::Probleem, "CWebKlikZonderToetsenbord", &[&klik_zonder_toetsenbord.len().to_string()], klik_zonder_toetsenbord);
    }
    if !verborgen_maar_focus.is_empty() {
        v.punt(Ernst::Probleem, "CWebVerborgenFocus", &[&verborgen_maar_focus.len().to_string()], verborgen_maar_focus);
    }
    if svg_zonder_naam > 0 {
        v.punt(Ernst::Probleem, "CWebSvgZonderNaam", &[&svg_zonder_naam.to_string()], Vec::new());
    }
    // Hoofdgedeelte en andere herkenningspunten.
    let heeft_main = labels.iter().any(|l| !l.sluit && (l.naam == "main" || l.attr("role") == Some("main")));
    if heeft_main {
        v.goed("CWebMain", &[]);
    } else {
        v.punt(Ernst::LetOp, "CWebGeenMain", &[], Vec::new());
    }
    // Inzoomen.
    let zoom_uit = open(&labels, "meta").any(|m| m.attr("name").map(|n| n.eq_ignore_ascii_case("viewport")).unwrap_or(false) && {
        let c = m.attr("content").unwrap_or("").to_lowercase().replace(' ', "");
        c.contains("user-scalable=no") || c.contains("user-scalable=0") || c.contains("maximum-scale=1,") || c.ends_with("maximum-scale=1")
            || c.contains("maximum-scale=1.0")
    });
    if zoom_uit {
        v.punt(Ernst::Probleem, "CWebZoom", &[], Vec::new());
    }
    if !iframes_zonder.is_empty() {
        v.punt(Ernst::Probleem, "CWebIframes", &[&iframes_zonder.len().to_string()], iframes_zonder);
    }
    let dubbel: Vec<String> = ids.iter().filter(|(_, n)| **n > 1).map(|(id, n)| format!("id=\"{id}\" ({n}x)")).collect();
    if !dubbel.is_empty() {
        v.punt(Ernst::LetOp, "CWebDubbeleIds", &[&dubbel.len().to_string()], dubbel);
    }
    if tabindex > 0 {
        v.punt(Ernst::LetOp, "CWebTabindex", &[&tabindex.to_string()], Vec::new());
    }
    if autoplay > 0 {
        v.punt(Ernst::Probleem, "CWebAutoplay", &[], Vec::new());
    }
    if !laag_contrast.is_empty() {
        v.punt(Ernst::Probleem, "CWebContrast", &[&laag_contrast.len().to_string()], laag_contrast);
    }
    // Wordt de pagina vooral met JavaScript gemaakt?
    if lichaam_tekst < 200 && scripts >= 3 {
        v.opmerkingen.push(t("CWebJavaScript"));
    }
    v.opmerkingen.push(t("CWebAlleenHtml"));
    v
}
