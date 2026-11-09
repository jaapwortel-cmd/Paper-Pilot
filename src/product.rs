// Paper Pilot - een product opzoeken aan de hand van de streepjescode (EAN), bij Open Food Facts.
//
// Open Food Facts (en de zusters Open Beauty Facts, Open Pet Food Facts en Open Products Facts) zijn vrije
// databanken, ingevuld door vrijwilligers. Paper Pilot stuurt ALLEEN het nummer van de streepjescode, geen foto.
// De eerste keer vraagt Paper Pilot of dat mag (instelling product_internet).

use crate::teksten::{t, tf};

#[derive(Clone, Debug, Default)]
pub struct Product {
    pub code: String,
    pub naam: String,
    pub merk: String,
    pub hoeveelheid: String,
    pub ingredienten: String,
    pub allergenen: Vec<String>,
    pub sporen: Vec<String>,
    pub nutriscore: String,
    /// "Open Food Facts" enz.
    pub bron: String,
}

/// Databanken, de eerste die het product kent wint.
const BRONNEN: [(&str, &str); 4] = [
    ("world.openfoodfacts.org", "Open Food Facts"),
    ("world.openbeautyfacts.org", "Open Beauty Facts"),
    ("world.openpetfoodfacts.org", "Open Pet Food Facts"),
    ("world.openproductsfacts.org", "Open Products Facts"),
];

/// Is dit het nummer van een productcode (EAN-8, UPC-A, EAN-13 of GTIN-14), met een kloppend controlecijfer?
pub fn is_productcode(tekst: &str) -> bool {
    let tekst = tekst.trim();
    if !matches!(tekst.len(), 8 | 12 | 13 | 14) || !tekst.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    let cijfers: Vec<u32> = tekst.chars().map(|c| c as u32 - '0' as u32).collect();
    let (laatste, rest) = cijfers.split_last().unwrap();
    // Van rechts naar links: om en om x3 en x1.
    let som: u32 = rest.iter().rev().enumerate().map(|(i, c)| if i % 2 == 0 { c * 3 } else { *c }).sum();
    (10 - som % 10) % 10 == *laatste
}

/// Zoekt het product op. Ok(None) = geen enkele databank kent het.
pub fn zoek(code: &str) -> Result<Option<Product>, String> {
    let code = code.trim();
    let taal = crate::teksten::taal();
    let velden = format!(
        "product_name,product_name_{taal},generic_name_{taal},brands,quantity,ingredients_text,ingredients_text_{taal},allergens_tags,traces_tags,nutriscore_grade"
    );
    let mut laatste_fout = None;
    let mut bereikt = false;
    for (host, bron) in BRONNEN {
        let adres = format!("https://{host}/api/v2/product/{code}.json?fields={velden}");
        let mut data = Vec::new();
        let antwoord = crate::http::verzoek("GET", &adres, "", None, 20000, &mut |b, _, _| {
            data.extend_from_slice(b);
            Ok(())
        });
        match antwoord {
            Ok((200, _)) => {
                bereikt = true;
                let tekst = String::from_utf8_lossy(&data);
                let v: serde_json::Value = match serde_json::from_str(&tekst) {
                    Ok(v) => v,
                    Err(e) => {
                        laatste_fout = Some(e.to_string());
                        continue;
                    }
                };
                // status 0 = "product not found": dan de volgende databank.
                if v["status"].as_i64() == Some(1) {
                    return Ok(Some(lees(code, &v["product"], bron, taal)));
                }
            }
            Ok((404, _)) => bereikt = true,
            Ok((s, _)) => laatste_fout = Some(format!("{bron}: {s}")),
            Err(e) => laatste_fout = Some(e),
        }
    }
    if let Some(e) = &laatste_fout {
        crate::logboek::schrijf(&format!("Product {code}: {e}"));
    }
    // Geen enkele databank bereikt: dat is iets anders dan "niet gevonden".
    if !bereikt {
        return Err(t("ProductGeenInternet"));
    }
    Ok(None)
}

fn lees(code: &str, p: &serde_json::Value, bron: &str, taal: &str) -> Product {
    let tekst = |sleutels: &[String]| -> String {
        sleutels.iter().filter_map(|k| p[k.as_str()].as_str()).map(|s| s.trim()).find(|s| !s.is_empty()).unwrap_or("").to_string()
    };
    let lijst = |k: &str| -> Vec<String> {
        let mut uit: Vec<String> = p[k].as_array().map(|a| a.iter().filter_map(|x| x.as_str()).map(allergeen).collect()).unwrap_or_default();
        uit.dedup();
        uit
    };
    let naam = tekst(&[format!("product_name_{taal}"), "product_name".into(), format!("generic_name_{taal}")]);
    Product {
        code: code.to_string(),
        naam,
        merk: p["brands"].as_str().unwrap_or("").replace(',', ", ").replace("  ", " ").trim().to_string(),
        hoeveelheid: p["quantity"].as_str().unwrap_or("").trim().to_string(),
        ingredienten: tekst(&[format!("ingredients_text_{taal}"), "ingredients_text".into()]).replace('_', ""),
        allergenen: lijst("allergens_tags"),
        sporen: lijst("traces_tags"),
        nutriscore: p["nutriscore_grade"].as_str().filter(|s| s.len() == 1).map(|s| s.to_uppercase()).unwrap_or_default(),
        bron: bron.to_string(),
    }
}

/// "en:milk" wordt "melk" (de 14 allergenen die op elke verpakking moeten staan); anders het woord zelf.
fn allergeen(tag: &str) -> String {
    let woord = tag.split_once(':').map(|x| x.1).unwrap_or(tag);
    let sleutel = match woord {
        "gluten" => "AllergeenGluten",
        "milk" => "AllergeenMelk",
        "eggs" => "AllergeenEi",
        "nuts" => "AllergeenNoten",
        "peanuts" => "AllergeenPinda",
        "soybeans" => "AllergeenSoja",
        "fish" => "AllergeenVis",
        "crustaceans" => "AllergeenSchaaldieren",
        "molluscs" => "AllergeenWeekdieren",
        "celery" => "AllergeenSelderij",
        "mustard" => "AllergeenMosterd",
        "sesame-seeds" => "AllergeenSesam",
        "sulphur-dioxide-and-sulphites" => "AllergeenSulfiet",
        "lupin" => "AllergeenLupine",
        _ => return woord.replace('-', " "),
    };
    t(sleutel)
}

/// Alles over het product, in gewone regels (voor het tekstvenster).
pub fn uitleg(p: &Product) -> String {
    let mut r = Vec::new();
    r.push(tf("ProductNaam", &[if p.naam.is_empty() { "?" } else { &p.naam }]));
    if !p.merk.is_empty() {
        r.push(tf("ProductMerk", &[&p.merk]));
    }
    if !p.hoeveelheid.is_empty() {
        r.push(tf("ProductInhoud", &[&p.hoeveelheid]));
    }
    if !p.allergenen.is_empty() {
        r.push(tf("ProductAllergenen", &[&p.allergenen.join(", ")]));
    }
    if !p.sporen.is_empty() {
        r.push(tf("ProductSporen", &[&p.sporen.join(", ")]));
    }
    if !p.nutriscore.is_empty() {
        r.push(tf("ProductNutriscore", &[&p.nutriscore]));
    }
    if !p.ingredienten.is_empty() {
        r.push(String::new());
        r.push(tf("ProductIngredienten", &[&p.ingredienten]));
    }
    r.push(String::new());
    r.push(tf("ProductCode", &[&p.code]));
    r.push(tf("ProductBron", &[&p.bron]));
    r.join("\r\n")
}

#[cfg(test)]
mod proeven {
    use super::*;

    #[test]
    fn controlecijfer() {
        assert!(is_productcode("8710398527141"));
        assert!(is_productcode("4006381333931"));
        assert!(!is_productcode("4006381333932"));
        assert!(!is_productcode("12345"));
        assert!(is_productcode("96385074"));
    }
}
