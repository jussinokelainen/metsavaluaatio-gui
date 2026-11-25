//! Arviointiketju ilman komentorivin kyselyitä.
//!
//! Sama laskenta kuin CLI:n `run`-funktiossa, mutta eteneminen raportoidaan
//! kanavan kautta ja kaikki välitulokset palautetaan `Valuation`-rakenteessa.
//! Tiedostot kirjoitetaan vasta, kun käyttäjä painaa tallennusnappia.

use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

use color_eyre::{Result, eyre::eyre};
use geo::{HasDimensions, MultiPolygon};
use geojson::Feature;
use metsaval_core::consts::{EPSG_TM35FIN, EPSG_WGS84};
use metsaval_core::model::{RawRow, SpeciesSummary, Stand, StandValue, TimberValue, Totals};
use metsaval_core::{excel, geo_utils, maps, metsakeskus, mml, valuation, wfs};
use proj::Proj;

use crate::{util, util::log};

/// Käyttäjän valinnat yhdelle arvioinnille.
#[derive(Debug, Clone)]
pub struct Options {
    pub property_id: String,
    pub api_key: String,
    /// Kunnan nimi. Tarvitaan vain, jos GeoPackage ladataan verkosta.
    pub municipality: Option<String>,
}

/// Kaikki arvioinnin tulokset. Kevyt kopioida ja siirtää säikeiden välillä.
#[derive(Clone)]
pub struct Valuation {
    pub property_id: String,
    pub municipality: Option<String>,
    pub property_geometry: MultiPolygon<f64>,
    pub property_area_ha: f64,
    pub parcel_count: usize,
    pub stand_features: Vec<Feature>,
    pub stand_values: Vec<StandValue>,
    pub species_summary: Vec<SpeciesSummary>,
    pub timber_values: Vec<TimberValue>,
    pub raw_rows: Vec<RawRow>,
    pub fixed_costs: f64,
    pub totals: Totals,
    pub stand_layer: String,
}

impl Valuation {
    /// Sama luku kuin CLI:n "LASKENNALLINEN ARVO".
    pub fn total_value(&self) -> f64 {
        self.totals.net_eur - self.fixed_costs
    }
}

/// Hakee kiinteistön tiedot, metsävarakuviot ja puustotiedot sekä laskee
/// arvion. Eteneminen lähetetään `log`-kanavaan.
pub async fn run(options: Options, log_tx: Sender<String>) -> Result<Valuation> {
    let property_id = util::pad_property_id(&options.property_id)?;
    let municipality = options
        .municipality
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string);

    if municipality.is_none() {
        return Err(eyre!(
            "Kunnan nimeä ei tiedetä. Kirjoita kunnan nimi, jotta GeoPackage voidaan hakea."
        ));
    }

    log(&log_tx, format!("Kiinteistötunnus: {property_id}"));
    if let Some(name) = &municipality {
        log(&log_tx, format!("Kunta: {name}"));
    }

    // GeoPackage-lataus alkaa heti kun kunta tiedetään, ja etenee taustalla
    // muun haun ajan jos ei ole valmista GeoPackagea olemassa
    let gpkg_task = {
        let name = municipality
            .clone()
            .ok_or_else(|| eyre!("Kunnan nimi puuttuu"))?;
        let gpkg_dir = util::get_gpkg_dir(Some(&log_tx));

        // Halutun kunnan GeoPackage tiedoston nimi
        let target_file_name = format!("MV_{}.gpkg", name);

        // Etsii tallennetuista GeoPackageista oikean kunnan tiedoston
        let mut found_gpkgs = Vec::new();
        for entry in std::fs::read_dir(gpkg_dir.clone())? {
            let entry = entry?;
            if let Some(filename) = entry.path().file_name()
                && let Some(path) = filename.to_str()
                && path == target_file_name
            {
                let found_path = entry.path();
                log(
                    &log_tx,
                    format!(
                        "Valmis GeoPackage löytyi kohteesta {}",
                        found_path.display()
                    ),
                );
                found_gpkgs.push(found_path.to_owned());
                break;
            }
        }

        if let Some(gpkg_path) = found_gpkgs.into_iter().next() {
            // Valmis GeoPackage löytyi, palauttaa sen oikeassa muodossa
            tokio::spawn(async move { Ok::<PathBuf, color_eyre::Report>(gpkg_path) })
        } else {
            // Valmista GeoPackagea ei löytynyt kyseiselle kunnalle, ladataan se
            log(
                &log_tx,
                format!("GeoPackage ladataan kohteeseen {}...", gpkg_dir.display()),
            );
            log(&log_tx, format!("Haetaan GeoPackage kunnalle {name}..."));

            let task_log = log_tx.clone();
            tokio::spawn(async move {
                let start = std::time::Instant::now();
                let path = metsakeskus::download_gpkg(&name, gpkg_dir).await?;
                let seconds = start.elapsed().as_secs_f64();
                let _ = task_log.send(format!(
                    "GeoPackage valmis ({seconds:.1} s): {}",
                    path.display()
                ));
                Ok::<PathBuf, color_eyre::Report>(path)
            })
        }
    };

    log(&log_tx, "Haetaan kiinteistön palstat MML:stä...");
    let property_features = mml::get_property_features(&property_id, &options.api_key).await?;
    let parcel_count = property_features.len();
    let property_geometry = combine_property_geometries(&property_id, &property_features)?;
    let property_area_ha = geo_utils::area_m2(&property_geometry) / 10_000.0;

    log(
        &log_tx,
        format!(
            "Kiinteistöltä löytyi {parcel_count} palstaa ({} ha).",
            util::fmt_fi(property_area_ha, 2)
        ),
    );

    let wfs_url = metsaval_core::consts::stand_wfs_url();

    log(&log_tx, "Haetaan metsävarakerroksia...");
    let stand_layers = wfs::get_layers(&wfs_url).await?;
    let stand_layer = wfs::select_stand_layer(&stand_layers)?;
    log(&log_tx, format!("Käytettävä metsävaralayer: {stand_layer}"));

    let bounds = geo_utils::bounds(&property_geometry)
        .ok_or_else(|| eyre!("Kiinteistön geometria on tyhjä."))?;
    let bbox = format!(
        "{},{},{},{},{}",
        bounds.0, bounds.1, bounds.2, bounds.3, EPSG_TM35FIN
    );

    log(&log_tx, "Haetaan metsävarakuviot...");
    let stand_features =
        wfs::get_features_bbox(&wfs_url, &stand_layer, &bbox, EPSG_TM35FIN, 10_000).await?;
    log(
        &log_tx,
        format!("BBOX-alueelta löytyi {} kuviota.", stand_features.len()),
    );

    let stands: Vec<Stand> = stand_features
        .iter()
        .filter_map(|feature| geo_utils::clip_stand(feature, &property_geometry))
        .collect();

    log(
        &log_tx,
        format!(
            "Kiinteistöllä sijaitsevia/leikkaavia metsäkuvioita: {}",
            stands.len()
        ),
    );

    if stands.is_empty() {
        return Err(eyre!("Kiinteistöltä ei löytynyt yhtään metsäkuviota."));
    }

    let stand_ids = collect_stand_ids(&stands)?;

    let gpkg_path = gpkg_task.await??;

    log(
        &log_tx,
        format!("Luetaan puustotiedot: {}", gpkg_path.display()),
    );
    let (strata, stand_summaries) =
        metsakeskus::read_trees_async(gpkg_path.clone(), stand_ids).await?;

    log(&log_tx, "Lasketaan arviota...");
    let species_summary = valuation::build_species_summary(&strata, &stands)?;
    let timber_values = valuation::build_timber_values(&species_summary);
    let stand_values = valuation::build_stand_values(&species_summary, &stand_summaries, &stands);
    let raw_rows = valuation::build_raw_rows(&strata, &stands, &stand_summaries)?;
    let fixed_costs = valuation::build_fixed_costs(&stand_values);
    let totals = valuation::compute_totals(&stands, &stand_values);

    log(
        &log_tx,
        format!(
            "Valmis: {} kuviota, laskennallinen arvo {}.",
            stands.len(),
            util::fmt_eur(totals.net_eur - fixed_costs)
        ),
    );

    Ok(Valuation {
        property_id,
        municipality,
        property_geometry,
        property_area_ha,
        parcel_count,
        stand_features,
        stand_values,
        species_summary,
        timber_values,
        raw_rows,
        fixed_costs,
        totals,
        stand_layer,
    })
}

/// Yhdistää palstojen geometriat ja muuntaa ne ETRS-TM35FIN-koordinaatistoon.
fn combine_property_geometries(
    property_id: &str,
    property_features: &[Feature],
) -> Result<MultiPolygon<f64>> {
    if property_features.is_empty() {
        return Err(eyre!(
            "Kiinteistölle {property_id} ei löytynyt palstoja MML:stä."
        ));
    }

    let mut geometries_wgs84 = Vec::new();
    for feature in property_features {
        if let Some(geometry) = geo_utils::feature_to_multipolygon(feature)
            && !geometry.is_empty()
        {
            geometries_wgs84.push(geometry);
        }
    }

    if geometries_wgs84.is_empty() {
        return Err(eyre!(
            "MML palautti palstat, mutta niillä ei ollut geometriaa."
        ));
    }

    let to_tm35 = Proj::new_known_crs(EPSG_WGS84, EPSG_TM35FIN, None)?;
    let property_wgs84 = geo_utils::union_all(&geometries_wgs84);
    Ok(geo_utils::transform(&property_wgs84, &to_tm35))
}

/// Kerää kuvioiden `standid`-tunnukset kasvavassa järjestyksessä.
fn collect_stand_ids(stands: &[Stand]) -> Result<Vec<i64>> {
    let mut stand_ids: Vec<i64> = stands.iter().filter_map(|stand| stand.standid).collect();
    stand_ids.sort_unstable();
    stand_ids.dedup();

    if stand_ids.is_empty() {
        return Err(eyre!("Kuvioilta ei löytynyt standid-tunnuksia."));
    }

    Ok(stand_ids)
}

/// Kirjoittaa Excel-yhteenvedon annettuun polkuun.
pub fn write_excel(valuation: &Valuation, path: &Path) -> Result<()> {
    excel::export(
        path,
        &valuation.totals,
        &valuation.stand_values,
        &valuation.species_summary,
        &valuation.timber_values,
        &valuation.raw_rows,
        &valuation.fixed_costs,
    )
}

/// Kirjoittaa Google Maps -tason (KML) annettuun polkuun.
pub fn write_kml(valuation: &Valuation, path: &Path) -> Result<()> {
    let kml = maps::generate_kml(
        &valuation.stand_features,
        &valuation.property_geometry,
        &valuation.stand_values,
    )?;
    maps::write_kml(&kml, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stand_ids_are_sorted_and_deduplicated() {
        let stands = vec![
            Stand {
                standid: Some(3),
                ..Stand::default()
            },
            Stand {
                standid: Some(1),
                ..Stand::default()
            },
            Stand {
                standid: Some(1),
                ..Stand::default()
            },
            Stand::default(),
        ];

        assert_eq!(collect_stand_ids(&stands).unwrap(), vec![1, 3]);
    }

    #[test]
    fn missing_stand_ids_are_an_error() {
        assert!(collect_stand_ids(&[]).is_err());
        assert!(collect_stand_ids(&[Stand::default()]).is_err());
    }

    #[test]
    fn total_value_subtracts_fixed_costs() {
        let valuation = Valuation {
            property_id: String::new(),
            municipality: None,
            property_geometry: MultiPolygon::new(vec![]),
            property_area_ha: 0.0,
            parcel_count: 0,
            stand_features: Vec::new(),
            stand_values: Vec::new(),
            species_summary: Vec::new(),
            timber_values: Vec::new(),
            raw_rows: Vec::new(),
            fixed_costs: 250.0,
            totals: Totals {
                net_eur: 1_000.0,
                ..Totals::default()
            },
            stand_layer: String::new(),
        };

        assert_eq!(valuation.total_value(), 750.0);
    }
}
