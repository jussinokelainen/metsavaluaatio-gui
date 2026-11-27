use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use color_eyre::{Result, eyre::eyre};
use serde::{Deserialize, Serialize};

/// CLI:n config.toml-rakenne.
#[derive(Deserialize)]
struct CliConfig {
    api_key: String,
}

#[derive(Deserialize, Serialize)]
pub struct GlobalOptions {
    pub gpkg_age_days: i64,
    pub api_key: Option<String>,
}

impl Default for GlobalOptions {
    fn default() -> Self {
        Self {
            // 2 viikkoa
            gpkg_age_days: 14,
            api_key: None,
        }
    }
}

/// `~/.config/metsavaluaatio/config.toml` (sama kuin CLI käyttää).
pub fn cli_config_file() -> Option<PathBuf> {
    let config_dir = dirs::home_dir()?.join(".config/metsavaluaatio");
    fs::create_dir_all(&config_dir).ok()?;
    Some(config_dir.join("config.toml"))
}

/// Lukee .config hakemistosta (nykyään käytössä vain CLI työkalussa)
/// tallennetun MML API-avaimen, jos sellainen on.
pub fn load_cli_api_key() -> Option<String> {
    let content = fs::read_to_string(cli_config_file()?).ok()?;
    let config: CliConfig = toml::from_str(&content).ok()?;
    Some(config.api_key)
}

fn data_dir() -> Option<PathBuf> {
    let data_dir = dirs::home_dir()?.join(".local/share/metsavaluaatio");
    fs::create_dir_all(&data_dir).ok()?;
    Some(data_dir)
}

/// `~/.local/share/metsavaluaatio/options.json`
fn options_file_path() -> Option<PathBuf> {
    Some(data_dir()?.join("options.json"))
}

/// Lataa ohjelman asetukset. Jos asetusten lataaminen epäonnistuu, palauttaa
/// vakio asetukset
pub fn load_options() -> GlobalOptions {
    let Some(path) = options_file_path() else {
        return GlobalOptions::default();
    };
    let Ok(json) = fs::read_to_string(path) else {
        return GlobalOptions::default();
    };
    serde_json::from_str(&json).unwrap_or_default()
}

/// Tallentaa asetukset
pub fn save_options(opts: &GlobalOptions) {
    let Some(path) = options_file_path() else {
        return;
    };
    if let Ok(json) = serde_json::to_string_pretty(opts) {
        let _ = fs::write(path, json);
    }
}

/// `~/.local/share/metsavaluaatio/municipalities.json` (sama kuin CLI käyttää).
fn municipalities_file_path() -> Option<PathBuf> {
    Some(data_dir()?.join("municipalities.json"))
}

/// Lukee tallennetut kunta-koodi -> kunta-nimi -parit.
pub fn load_municipalities() -> HashMap<String, String> {
    let Some(path) = municipalities_file_path() else {
        return HashMap::new();
    };
    let Ok(json) = fs::read_to_string(path) else {
        return HashMap::new();
    };
    serde_json::from_str(&json).unwrap_or_default()
}

/// Tallentaa kunta-koodi -> kunta-nimi -parit.
pub fn save_municipalities(municipalities: &HashMap<String, String>) {
    let Some(path) = municipalities_file_path() else {
        return;
    };
    if let Ok(json) = serde_json::to_string_pretty(municipalities) {
        let _ = fs::write(path, json);
    }
}

/// Poistaa kaikki tallennetut kunnat
pub fn clear_saved_municipalities() -> Result<()> {
    if let Some(path) = municipalities_file_path() {
        fs::remove_file(path)?;
    } else {
        return Err(eyre!("Kunta-lista tiedostoa ei löytynyt"));
    }
    Ok(())
}

/// Muistaa yhden kunnan nimen ja tallentaa listan.
#[allow(dead_code)]
pub fn remember_municipality(code: &str, name: &str) {
    let mut municipalities = load_municipalities();
    municipalities.insert(code.to_string(), name.to_string());
    save_municipalities(&municipalities);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_parsing_accepts_the_cli_format() {
        let config: CliConfig = toml::from_str("api_key = \"avain123\"").unwrap();
        assert_eq!(config.api_key, "avain123");
    }

    #[test]
    fn municipality_json_round_trips() {
        let municipalities = HashMap::from([
            ("740".to_string(), "Testila".to_string()),
            ("577".to_string(), "Uusikaupunki".to_string()),
        ]);
        let json = serde_json::to_string(&municipalities).unwrap();
        assert_eq!(
            serde_json::from_str::<HashMap<String, String>>(&json).unwrap(),
            municipalities
        );
    }
}
