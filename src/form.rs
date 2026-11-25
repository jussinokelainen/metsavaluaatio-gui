//! Hakukanavan lomakkeen validointi ilman GPUI-riippuvuuksia.

use std::collections::HashMap;

use crate::util;

/// Hakukanavan raakasisältö ennen validointia.
#[derive(Debug)]
pub struct RunForm {
    pub api_key: String,
    pub property_id: String,
}

/// Validoitu haku ja tieto siitä, lisättiinkö kuntamuistiin uusi nimi.
#[derive(Debug)]
pub struct PreparedRun {
    pub property_id: String,
    pub api_key: String,
    pub municipality: Option<String>,
    pub municipality_added: bool,
}

/// Validoi lomakkeen ja muodostaa taustatyön valinnat. Kuntamuistia
/// muutetaan vain, kun käyttäjä on syöttänyt uuden kunnan nimen; tallennus
/// jää kutsujalle.
pub fn prepare_run(
    form: &RunForm,
    municipalities: &mut HashMap<String, String>,
) -> Result<PreparedRun, String> {
    let api_key = form.api_key.trim().to_string();
    if api_key.is_empty() {
        return Err("MML API-avain puuttuu. Lisää se asetuksista (⚙).".to_string());
    }

    let property_id =
        util::pad_property_id(&form.property_id).map_err(|error| error.to_string())?;
    let code = util::municipality_code(&property_id).unwrap_or_default();

    let (municipality, municipality_added) = (municipalities.get(&code).cloned(), false);

    Ok(PreparedRun {
        property_id,
        api_key,
        municipality,
        municipality_added,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_form() -> RunForm {
        RunForm {
            api_key: "avain123".to_string(),
            property_id: "740-555-2-4".to_string(),
            municipality_input: String::new(),
        }
    }

    #[test]
    fn prepare_run_rejects_a_missing_api_key() {
        let mut municipalities = HashMap::new();
        let mut form = run_form();
        form.api_key = "   ".to_string();

        let error = prepare_run(&form, &mut municipalities).unwrap_err();
        assert!(error.contains("API-avain"), "virhe: {error}");
    }

    #[test]
    fn prepare_run_rejects_an_invalid_property_id() {
        let mut municipalities = HashMap::new();
        let mut form = run_form();
        form.property_id = "740-555-2".to_string();

        let error = prepare_run(&form, &mut municipalities).unwrap_err();
        assert!(
            error.contains("Virheellinen kiinteistötunnus"),
            "virhe: {error}"
        );
    }

    #[test]
    fn prepare_run_normalizes_the_property_id_and_trims_the_api_key() {
        let mut municipalities = HashMap::from([("740".to_string(), "Testila".to_string())]);
        let mut form = run_form();
        form.api_key = "  avain123  ".to_string();

        let prepared = prepare_run(&form, &mut municipalities).unwrap();
        assert_eq!(prepared.property_id, "740-555-0002-0004");
        assert_eq!(prepared.api_key, "avain123");
        assert_eq!(prepared.municipality.as_deref(), Some("Testila"));
        assert!(!prepared.municipality_added);
    }

    #[test]
    fn prepare_run_uses_a_typed_municipality_and_stores_it() {
        let mut municipalities = HashMap::new();
        let mut form = run_form();
        form.municipality_input = "  Uusikaupunki  ".to_string();

        let prepared = prepare_run(&form, &mut municipalities).unwrap();
        assert_eq!(prepared.municipality.as_deref(), Some("Uusikaupunki"));
        assert!(prepared.municipality_added);
        assert_eq!(
            municipalities.get("740").map(String::as_str),
            Some("Uusikaupunki")
        );
    }

    #[test]
    fn prepare_run_stores_the_municipality_under_the_padded_code() {
        let mut municipalities = HashMap::new();
        let mut form = run_form();
        form.property_id = "12-345-6-789".to_string();
        form.municipality_input = "Testila".to_string();

        let prepared = prepare_run(&form, &mut municipalities).unwrap();
        assert_eq!(prepared.property_id, "012-345-0006-0789");
        assert_eq!(
            municipalities.get("012").map(String::as_str),
            Some("Testila")
        );
        assert!(!municipalities.contains_key("12"));
    }

    #[test]
    fn prepare_run_rejects_an_unknown_municipality() {
        let mut municipalities = HashMap::new();
        let form = run_form();

        let error = prepare_run(&form, &mut municipalities).unwrap_err();
        assert!(
            error.contains("ei ole tallennettua nimeä"),
            "virhe: {error}"
        );
    }
}
