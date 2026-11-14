use chrono::{Local, SubsecRound};
use color_eyre::{Result, eyre::eyre};
use std::path::PathBuf;

/// Laajentaa `~`-alkuisen polun kotihakemistoksi.
pub fn expand_path(path: &str) -> PathBuf {
    let path = path.trim();
    if let Some(rest) = path.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(rest);
    }
    PathBuf::from(path)
}

/// Lisää kiinteistötunnukseen puuttuvat nollat, jotta pituus on oikea.
///
/// Esim. `12-345-6-789` -> `012-345-0006-0789`.
pub fn pad_property_id(id: &str) -> Result<String> {
    let id = id.trim();
    let parts: Vec<&str> = id.split('-').collect();
    if parts.len() != 4 {
        return Err(eyre!(
            "Virheellinen kiinteistötunnus (muoto: 123-456-7-890)"
        ));
    }

    let padded: Vec<String> = parts
        .iter()
        .enumerate()
        .map(|(index, part)| {
            let width = if index < 2 { 3 } else { 4 };
            if part.is_empty() || part.len() > width || !part.chars().all(|c| c.is_ascii_digit()) {
                return Err(eyre!(
                    "Virheellinen kiinteistötunnus (muoto: 123-456-7-890)"
                ));
            }
            Ok(format!("{part:0>width$}"))
        })
        .collect::<Result<_>>()?;

    Ok(padded.join("-"))
}

/// Kiinteistötunnuksen kuntaosa (ensimmäinen 3-numeroinen osa).
pub fn municipality_code(property_id: &str) -> Option<String> {
    let id = property_id.trim();
    let code = id.split('-').next()?.trim();
    if code.is_empty() {
        None
    } else {
        Some(code.to_string())
    }
}

/// Kiinteistötunnuksen kuntakoodi joko täydennetystä tai raa'asta
/// tunnuksesta. Täydennetty tunnus on ensisijainen, koska se normalisoi
/// kuntakoodin pituuden.
pub fn property_code(property_id: &str) -> Option<String> {
    pad_property_id(property_id)
        .ok()
        .and_then(|padded| municipality_code(&padded))
        .or_else(|| municipality_code(property_id))
}

/// Lisää kuntakoodin alkuun nollia, jos sen pituus on alle 3, jotta
/// kuntamuisti toimii riippumatta siitä, syöttääkö käyttäjä nollia koodin
/// alkuun vai ei.
pub fn pad_municipality_code(code: Option<String>) -> Option<String> {
    let code = code?;
    if code.chars().count() == 3 {
        Some(code)
    } else {
        Some(format!("{:0>3}", code))
    }
}

/// Generoi ohjelman tuottamille tiedostoille nimen (sama kuin CLI:ssä).
pub fn generate_filename(property_id: &str, suffix: &str) -> String {
    let mut time = Local::now().trunc_subsecs(0).to_rfc3339().replace(':', "-");

    // Poistaa ajasta kellonajan ja aikavyöhykkeen (esim. 'T08-00-00 +0300')
    if let Some(pos) = time.find('T') {
        time.truncate(pos);
    }

    format!("{property_id}__{time}{suffix}")
}

/// Suomalainen lukumuoto: välilyönti tuhaterottimena ja pilkku desimaalina.
pub fn fmt_fi(value: f64, decimals: usize) -> String {
    if !value.is_finite() {
        return "–".to_string();
    }

    let text = format!("{value:.decimals$}");
    let (sign, rest) = match text.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", text.as_str()),
    };
    let (integer, fraction) = match rest.split_once('.') {
        Some((integer, fraction)) => (integer, Some(fraction)),
        None => (rest, None),
    };

    let chars: Vec<char> = integer.chars().collect();
    let mut grouped = String::new();
    for (index, ch) in chars.iter().enumerate() {
        if index > 0 && (chars.len() - index).is_multiple_of(3) {
            grouped.push(' ');
        }
        grouped.push(*ch);
    }

    match fraction {
        Some(fraction) if decimals > 0 => format!("{sign}{grouped},{fraction}"),
        _ => format!("{sign}{grouped}"),
    }
}

/// Euromäärä suomalaisessa muodossa.
pub fn fmt_eur(value: f64) -> String {
    format!("{} €", fmt_fi(value, 0))
}

/// Kartalla näytettävä puulajin nimi koodista.
#[allow(dead_code)]
pub fn species_name(code: Option<i64>) -> &'static str {
    match code {
        Some(code) => metsaval::consts::Species::from_code(code).name(),
        None => "–",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn property_id_is_padded_to_full_length() {
        assert_eq!(
            pad_property_id("12-345-6-789").unwrap(),
            "012-345-0006-0789"
        );
        assert_eq!(pad_property_id("740-555-2-4").unwrap(), "740-555-0002-0004");
        assert_eq!(
            pad_property_id(" 001-002-0003-0004 ").unwrap(),
            "001-002-0003-0004"
        );
    }

    #[test]
    fn invalid_property_ids_are_rejected() {
        assert!(pad_property_id("").is_err());
        assert!(pad_property_id("12-345-6").is_err());
        assert!(pad_property_id("12-345-6-789-0").is_err());
        assert!(pad_property_id("1234-345-6-789").is_err());
        assert!(pad_property_id("12-345-abc-789").is_err());
    }

    #[test]
    fn municipality_code_is_the_first_part() {
        assert_eq!(
            municipality_code("740-555-0002-0000").as_deref(),
            Some("740")
        );
        assert_eq!(municipality_code("12-345-6-789").as_deref(), Some("12"));
        assert_eq!(municipality_code("  "), None);
    }

    #[test]
    fn property_code_uses_the_padded_identifier_first() {
        assert_eq!(property_code("740-555-2-4").as_deref(), Some("740"));
        assert_eq!(property_code("12-345-6-789").as_deref(), Some("012"));
        assert_eq!(property_code("1-2-3-4").as_deref(), Some("001"));
    }

    #[test]
    fn property_code_falls_back_to_the_raw_identifier() {
        assert_eq!(property_code("740").as_deref(), Some("740"));
        assert_eq!(property_code(" 740 ").as_deref(), Some("740"));
        assert_eq!(property_code(""), None);
        assert_eq!(property_code("   "), None);
    }

    #[test]
    fn municipality_codes_are_padded_to_three_characters() {
        assert_eq!(pad_municipality_code(None), None);
        assert_eq!(
            pad_municipality_code(Some("1".to_string())).as_deref(),
            Some("001")
        );
        assert_eq!(
            pad_municipality_code(Some("12".to_string())).as_deref(),
            Some("012")
        );
        assert_eq!(
            pad_municipality_code(Some("740".to_string())).as_deref(),
            Some("740")
        );
        // Pidempiä koodeja ei katkaista.
        assert_eq!(
            pad_municipality_code(Some("1234".to_string())).as_deref(),
            Some("1234")
        );
    }

    #[test]
    fn filename_contains_property_id_date_and_suffix() {
        let name = generate_filename("012-345-0006-0789", ".xlsx");
        assert!(name.starts_with("012-345-0006-0789__"));
        assert!(name.ends_with(".xlsx"));
        assert!(!name.contains('T'));
        assert!(!name.contains(':'));
    }

    #[test]
    fn numbers_use_finnish_separators() {
        assert_eq!(fmt_fi(999.0, 0), "999");
        assert_eq!(fmt_fi(1000.0, 0), "1 000");
        assert_eq!(fmt_fi(1234567.891, 2), "1 234 567,89");
        assert_eq!(fmt_fi(-1234.5, 2), "-1 234,50");
    }

    #[test]
    fn euro_format_has_no_decimals() {
        assert_eq!(fmt_eur(1234.56), "1 235 €");
    }

    #[test]
    fn tilde_expands_to_the_home_directory() {
        let expanded = expand_path("~/Downloads");
        assert_eq!(expanded, dirs::home_dir().unwrap().join("Downloads"));

        assert_eq!(expand_path("/tmp/kansio"), PathBuf::from("/tmp/kansio"));
        assert_eq!(expand_path("sopiva"), PathBuf::from("sopiva"));
    }
}
