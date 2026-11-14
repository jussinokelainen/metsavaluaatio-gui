//! Metsävaluuation graafinen käyttöliittymä.
//!
//! Ohjelma käyttää samoja `metsaval`-kirjaston moduuleja kuin komentorivityökalu,
//! mutta hoitaa kyselyt ja tulosteet ikkunassa. Verkkohaku ja tiedostojen
//! kirjoitus ajetaan taustasäikeissä, jotta käyttöliittymä pysyy responsiivisena.

mod pipeline;
mod settings;
mod util;

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use eframe::egui;
use egui_extras::{Column, TableBuilder};
use metsaval::consts::Cutting;
use metsaval::model::{SpeciesSummary, StandValue};

use pipeline::{Options, Valuation};

const GREEN: egui::Color32 = egui::Color32::from_rgb(88, 214, 141);
const RED: egui::Color32 = egui::Color32::from_rgb(255, 120, 120);
const MUTED: egui::Color32 = egui::Color32::from_rgb(150, 155, 165);

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(egui::vec2(1120.0, 780.0))
            .with_min_inner_size(egui::vec2(840.0, 560.0))
            .with_title("Metsävaluaatio"),
        ..Default::default()
    };

    eframe::run_native(
        "Metsävaluaatio",
        options,
        Box::new(|cc| Ok(Box::new(MetsavalApp::new(cc)))),
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Stands,
    Species,
    Log,
}

enum ExportMessage {
    Done(Vec<PathBuf>),
    Error(String),
}

struct MetsavalApp {
    // Asetukset
    api_key: String,
    show_api_key: bool,
    municipalities: std::collections::HashMap<String, String>,

    // Hakukanavan kentät
    property_id: String,
    municipality_input: String,
    use_gpkg: bool,
    gpkg_path: String,
    gpkg_download_dir: String,
    output_dir: String,

    // Tila
    running: bool,
    exporting: bool,
    result: Option<Box<Valuation>>,
    error: Option<String>,
    notice: Option<String>,
    logs: Vec<String>,
    tab: Tab,
    exported: Vec<PathBuf>,

    // Taustatyön kanavat
    log_rx: Option<Receiver<String>>,
    done_rx: Option<Receiver<Result<Box<Valuation>, String>>>,
    export_rx: Option<Receiver<ExportMessage>>,
}

impl MetsavalApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());

        let output_dir = dirs::download_dir()
            .or_else(dirs::home_dir)
            .map(|path| path.display().to_string())
            .unwrap_or_default();

        Self {
            api_key: settings::load_api_key().unwrap_or_default(),
            show_api_key: false,
            municipalities: settings::load_municipalities(),
            property_id: String::new(),
            municipality_input: String::new(),
            use_gpkg: false,
            gpkg_path: String::new(),
            gpkg_download_dir: String::new(),
            output_dir,
            running: false,
            exporting: false,
            result: None,
            error: None,
            notice: None,
            logs: Vec::new(),
            tab: Tab::Stands,
            exported: Vec::new(),
            log_rx: None,
            done_rx: None,
            export_rx: None,
        }
    }

    /// Tämänhetkinen kunta-koodi joko täydennetystä tai raa'asta tunnuksesta.
    fn current_code(&self) -> Option<String> {
        util::pad_property_id(&self.property_id)
            .ok()
            .and_then(|padded| util::municipality_code(&padded))
            .or_else(|| util::municipality_code(&self.property_id))
    }

    /// Käynnistää haun taustasäikeessä.
    fn start_run(&mut self, ctx: &egui::Context) {
        if self.running {
            return;
        }

        self.error = None;
        self.notice = None;

        let api_key = self.api_key.trim().to_string();
        if api_key.is_empty() {
            self.error = Some(
                "MML API-avain puuttuu. Lisää se vasemman paneelin Lisäasetukset-osiossa."
                    .to_string(),
            );
            return;
        }

        let padded = match util::pad_property_id(&self.property_id) {
            Ok(padded) => padded,
            Err(error) => {
                self.error = Some(error.to_string());
                return;
            }
        };
        let code = util::municipality_code(&padded).unwrap_or_default();

        let gpkg_path = if self.use_gpkg {
            let path = self.gpkg_path.trim();
            if path.is_empty() {
                self.error = Some("Anna käytettävän GeoPackagen polku.".to_string());
                return;
            }
            Some(util::expand_path(path))
        } else {
            None
        };
        let municipality = if gpkg_path.is_some() {
            self.municipalities.get(&code).cloned()
        } else {
            let typed = self.municipality_input.trim().to_string();
            if !typed.is_empty() {
                self.municipalities.insert(code.clone(), typed.clone());
                settings::save_municipalities(&self.municipalities);
                self.municipality_input.clear();
                Some(typed)
            } else if let Some(name) = self.municipalities.get(&code) {
                Some(name.clone())
            } else {
                self.error = Some(format!(
                    "Kunnalle {code} ei ole tallennettua nimeä. Kirjoita kunnan nimi."
                ));
                return;
            }
        };

        // Tallennetaan avain, jotta sitä ei tarvitse syöttää uudelleen.
        if let Err(error) = settings::save_api_key(&api_key) {
            self.notice = Some(format!("API-avaimen tallennus epäonnistui: {error:#}"));
        }

        let gpkg_download_dir = {
            let path = self.gpkg_download_dir.trim();
            if path.is_empty() {
                None
            } else {
                Some(util::expand_path(path))
            }
        };

        let options = Options {
            property_id: padded,
            api_key,
            municipality,
            gpkg_path,
            gpkg_download_dir,
        };

        self.logs.clear();
        self.logs.push("Aloitetaan haku...".to_string());
        self.result = None;
        self.exported.clear();
        self.running = true;
        self.tab = Tab::Stands;

        let (log_tx, log_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        self.log_rx = Some(log_rx);
        self.done_rx = Some(done_rx);

        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = run_worker(options, log_tx);
            let _ = done_tx.send(result);
            ctx.request_repaint();
        });
    }

    /// Kirjoittaa Excel- ja KML-tiedostot valittuun kansioon.
    fn start_export(&mut self, ctx: &egui::Context) {
        if self.exporting {
            return;
        }
        let Some(valuation) = self.result.as_ref() else {
            return;
        };

        let dir = util::expand_path(&self.output_dir);
        if self.output_dir.trim().is_empty() {
            self.error = Some("Anna tulostekansio.".to_string());
            return;
        }

        let valuation = valuation.clone();
        let property_id = valuation.property_id.clone();
        let (tx, rx) = mpsc::channel();
        self.export_rx = Some(rx);
        self.exporting = true;
        self.exported.clear();
        self.error = None;
        self.notice = None;

        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = (|| -> color_eyre::Result<Vec<PathBuf>> {
                std::fs::create_dir_all(&dir)?;
                let excel_path = dir.join(util::generate_filename(&property_id, ".xlsx"));
                let kml_path = dir.join(util::generate_filename(&property_id, ".kml"));
                pipeline::write_excel(&valuation, &excel_path)?;
                pipeline::write_kml(&valuation, &kml_path)?;
                Ok(vec![kml_path, excel_path])
            })();

            let message = match result {
                Ok(paths) => ExportMessage::Done(paths),
                Err(error) => ExportMessage::Error(format!("{error:#}")),
            };
            let _ = tx.send(message);
            ctx.request_repaint();
        });
    }

    /// Lukee taustasäikeiden viestit.
    fn poll(&mut self) {
        if let Some(rx) = self.log_rx.take() {
            for line in rx.try_iter() {
                self.logs.push(line);
            }
            self.log_rx = Some(rx);
        }

        if let Some(rx) = self.done_rx.take() {
            match rx.try_recv() {
                Ok(Ok(valuation)) => {
                    self.result = Some(valuation);
                    self.running = false;
                    self.log_rx = None;
                    self.tab = Tab::Stands;
                    self.notice = Some("Arvio valmis.".to_string());
                }
                Ok(Err(error)) => {
                    self.error = Some(error);
                    self.running = false;
                    self.log_rx = None;
                }
                Err(mpsc::TryRecvError::Empty) => {
                    self.done_rx = Some(rx);
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.running = false;
                    self.log_rx = None;
                    self.error = Some("Taustatyö keskeytyi odottamatta.".to_string());
                }
            }
        }

        if let Some(rx) = self.export_rx.take() {
            match rx.try_recv() {
                Ok(ExportMessage::Done(paths)) => {
                    self.exported = paths;
                    self.exporting = false;
                    self.notice = Some("Tiedostot tallennettu.".to_string());
                }
                Ok(ExportMessage::Error(error)) => {
                    self.exporting = false;
                    self.error = Some(error);
                }
                Err(mpsc::TryRecvError::Empty) => {
                    self.export_rx = Some(rx);
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.exporting = false;
                    self.error = Some("Tiedostojen tallennus keskeytyi.".to_string());
                }
            }
        }
    }

    fn header(&mut self, ui: &mut egui::Ui) {
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.heading("Metsävaluaatio");
            ui.label(egui::RichText::new("metsäomaisuuden arviointi").color(MUTED));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if self.api_key.trim().is_empty() {
                    ui.colored_label(egui::Color32::from_rgb(240, 180, 80), "API-avain puuttuu");
                } else {
                    ui.colored_label(GREEN, "API-avain asetettu");
                }
            });
        });
        ui.add_space(2.0);
    }

    fn form(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.add_space(6.0);
        ui.heading("Haku");
        ui.add_space(6.0);

        ui.label(egui::RichText::new("Kiinteistötunnus").strong());
        ui.add(
            egui::TextEdit::singleline(&mut self.property_id)
                .hint_text("esim. 740-555-2-4")
                .desired_width(f32::INFINITY),
        );

        ui.add_space(8.0);

        let code = self.current_code();
        let known = code
            .as_ref()
            .and_then(|code| self.municipalities.get(code))
            .cloned();
        ui.label(egui::RichText::new("Kunta").strong());
        if self.use_gpkg {
            ui.label(
                egui::RichText::new("Käytetään valmista GeoPackagea, kuntaa ei tarvita")
                    .color(MUTED),
            );
        } else if let Some(name) = known {
            ui.label(format!("{name} ({})", code.unwrap_or_default()));
        } else if let Some(code) = code {
            ui.add(
                egui::TextEdit::singleline(&mut self.municipality_input)
                    .hint_text(format!("Kunnan nimi koodille {code}"))
                    .desired_width(f32::INFINITY),
            );
            ui.label(
                egui::RichText::new("Nimi tallennetaan, jotta sitä ei kysytä uudelleen.")
                    .color(MUTED),
            );
        } else {
            ui.label(egui::RichText::new("Syötä ensin kiinteistötunnus").color(MUTED));
        }

        ui.add_space(10.0);

        egui::CollapsingHeader::new("Lisäasetukset")
            .default_open(self.api_key.trim().is_empty())
            .show(ui, |ui| {
                ui.label(egui::RichText::new("MML API-avain").strong());
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.api_key)
                            .password(!self.show_api_key)
                            .desired_width(180.0),
                    );
                    ui.checkbox(&mut self.show_api_key, "Näytä");
                });
                if ui.button("Tallenna avain").clicked() {
                    match settings::save_api_key(&self.api_key) {
                        Ok(path) => {
                            self.notice =
                                Some(format!("API-avain tallennettu: {}", path.display()));
                            self.error = None;
                        }
                        Err(error) => {
                            self.error =
                                Some(format!("API-avaimen tallennus epäonnistui: {error:#}"));
                        }
                    }
                }

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);

                ui.checkbox(&mut self.use_gpkg, "Käytä valmista GeoPackagea");
                if self.use_gpkg {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.gpkg_path)
                            .hint_text("/polku/MV_Kunta.gpkg")
                            .desired_width(f32::INFINITY),
                    );
                    ui.label(
                        egui::RichText::new(
                            "Vältä lataus, kun haet useita kiinteistöjä samasta kunnasta.",
                        )
                        .color(MUTED)
                        .small(),
                    );
                } else {
                    ui.label(
                        egui::RichText::new("GeoPackagen latauskansio (valinnainen)").strong(),
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut self.gpkg_download_dir)
                            .hint_text("oletus: järjestelmän väliaikaiskansio")
                            .desired_width(f32::INFINITY),
                    );
                }
            });

        ui.add_space(12.0);

        let button_text = if self.running {
            "Haetaan..."
        } else {
            "Hae arvio"
        };
        let button = egui::Button::new(egui::RichText::new(button_text).strong());
        if ui
            .add_enabled(
                !self.running,
                button.min_size(egui::vec2(ui.available_width(), 36.0)),
            )
            .clicked()
        {
            self.start_run(ctx);
        }

        if self.running {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(egui::RichText::new("Haetaan tietoja verkosta...").color(MUTED));
            });
        }

        if let Some(notice) = &self.notice {
            ui.add_space(6.0);
            ui.colored_label(GREEN, notice);
        }

        ui.add_space(16.0);
        ui.separator();
        ui.add_space(6.0);
        ui.label(egui::RichText::new("Tulostekansio").strong());
        ui.add(
            egui::TextEdit::singleline(&mut self.output_dir)
                .hint_text("esim. ~/Downloads")
                .desired_width(f32::INFINITY),
        );
        ui.label(
            egui::RichText::new("Excel- ja KML-tiedostot kirjoitetaan tähän kansioon.")
                .color(MUTED)
                .small(),
        );

        ui.add_space(12.0);
        if ui
            .add_enabled(
                self.result.is_some() && !self.exporting && !self.running,
                egui::Button::new("Tallenna Excel + KML")
                    .min_size(egui::vec2(ui.available_width(), 32.0)),
            )
            .clicked()
        {
            self.start_export(ctx);
        }

        if self.exporting {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(egui::RichText::new("Kirjoitetaan tiedostoja...").color(MUTED));
            });
        }

        if !self.exported.is_empty() {
            ui.add_space(8.0);
            for path in &self.exported {
                ui.horizontal(|ui| {
                    ui.colored_label(GREEN, "✓");
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(path.display().to_string()).monospace(),
                        )
                        .wrap(),
                    );
                });
            }
        }

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui.button("Google My Maps").clicked() {
                metsaval::maps::open_browser("https://www.google.com/mymaps");
            }
            if ui.button("Avaa tulostekansio").clicked() {
                metsaval::maps::open_browser(self.output_dir.trim());
            }
        });
    }

    fn welcome(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(48.0);
            ui.heading("Metsäomaisuuden arvio");
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(
                    "Syötä kiinteistötunnus ja hae arvio. Ohjelma hakee kiinteistön rajat,\n\
                     metsävarakuviot ja puustotiedot sekä laskee laskennallisen arvon.",
                )
                .color(MUTED),
            );
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(
                    "Tulokset voi tallentaa Excel-yhteenvetona ja Google Maps -tasona (KML).",
                )
                .color(MUTED)
                .small(),
            );
        });

        ui.add_space(28.0);
        ui.separator();
        ui.add_space(6.0);
        ui.label(egui::RichText::new("Tapahtumat").strong());
        self.log_view(ui);
    }

    fn content(&mut self, ui: &mut egui::Ui) {
        if let Some(error) = self.error.clone() {
            egui::Frame::group(ui.style())
                .fill(egui::Color32::from_rgb(58, 24, 24))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(150, 55, 55)))
                .show(ui, |ui| {
                    ui.add(egui::Label::new(egui::RichText::new(error).color(RED)).wrap());
                });
            ui.add_space(8.0);
        }

        let Some(valuation) = self.result.take() else {
            self.welcome(ui);
            return;
        };

        ui.horizontal(|ui| {
            ui.heading(&valuation.property_id);
            if let Some(name) = &valuation.municipality {
                ui.label(egui::RichText::new(name).color(MUTED));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "{} palstaa · {} · {}",
                        valuation.parcel_count,
                        valuation.stand_layer,
                        valuation.gpkg_path.display()
                    ))
                    .color(MUTED)
                    .small(),
                );
            });
        });

        ui.add_space(6.0);
        let net_color = if valuation.total_value() >= 0.0 {
            GREEN
        } else {
            RED
        };
        ui.horizontal_wrapped(|ui| {
            stat_card(
                ui,
                "Laskennallinen arvo",
                &util::fmt_eur(valuation.total_value()),
                net_color,
            );
            stat_card(
                ui,
                "Kiinteistön ala",
                &format!("{} ha", util::fmt_fi(valuation.property_area_ha, 2)),
                ui.visuals().text_color(),
            );
            stat_card(
                ui,
                "Kuvioiden ala",
                &format!("{} ha", util::fmt_fi(valuation.totals.area_ha, 2)),
                ui.visuals().text_color(),
            );
            stat_card(
                ui,
                "Puusto",
                &format!("{} m³", util::fmt_fi(valuation.totals.volume_m3, 0)),
                ui.visuals().text_color(),
            );
            stat_card(
                ui,
                "Tulot",
                &util::fmt_eur(valuation.totals.income_eur),
                ui.visuals().text_color(),
            );
            stat_card(
                ui,
                "Muuttuvat kulut",
                &util::fmt_eur(valuation.totals.costs_eur),
                ui.visuals().text_color(),
            );
            stat_card(
                ui,
                "Kiinteät kulut",
                &util::fmt_eur(valuation.fixed_costs),
                ui.visuals().text_color(),
            );
            stat_card(
                ui,
                "Kuvioita",
                &valuation.totals.stand_count.to_string(),
                ui.visuals().text_color(),
            );
        });
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.tab, Tab::Stands, "Kuviot");
            ui.selectable_value(&mut self.tab, Tab::Species, "Puulajit");
            ui.selectable_value(&mut self.tab, Tab::Log, "Loki");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Avaa Google My Maps").clicked() {
                    metsaval::maps::open_browser("https://www.google.com/mymaps");
                }
            });
        });
        ui.separator();

        match self.tab {
            Tab::Stands => stands_table(ui, &valuation.stand_values),
            Tab::Species => species_table(ui, &valuation.species_summary),
            Tab::Log => self.log_view(ui),
        }

        self.result = Some(valuation);
    }

    fn log_view(&self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                if self.logs.is_empty() {
                    ui.label(egui::RichText::new("Ei tapahtumia.").color(MUTED));
                }
                for line in &self.logs {
                    ui.monospace(line);
                }
            });
    }
}

impl eframe::App for MetsavalApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll();

        let ctx = ui.ctx().clone();
        if self.running || self.exporting {
            ctx.request_repaint_after(Duration::from_millis(100));
        }

        egui::Panel::top("header").show(ui, |ui| self.header(ui));
        egui::Panel::left("controls")
            .resizable(false)
            .default_size(340.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.form(ui, &ctx));
            });
        egui::CentralPanel::default().show(ui, |ui| self.content(ui));
    }
}

/// Taustasäikeen työ: luo Tokio-runtime ja ajaa koko hakukanavan.
fn run_worker(options: Options, log_tx: Sender<String>) -> Result<Box<Valuation>, String> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("Taustasäikeen käynnistys epäonnistui: {error}"))?;

    runtime
        .block_on(pipeline::run(options, log_tx))
        .map(Box::new)
        .map_err(|error| format!("{error:#}"))
}

fn stat_card(ui: &mut egui::Ui, label: &str, value: &str, color: egui::Color32) {
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.set_min_width(132.0);
        ui.vertical(|ui| {
            ui.label(egui::RichText::new(label).small().color(MUTED));
            ui.label(egui::RichText::new(value).size(18.0).strong().color(color));
        });
    });
}

fn cutting_text(cutting: Option<Cutting>, year: Option<i64>) -> String {
    match (cutting, year) {
        (Some(cutting), Some(year)) => format!("{} {year}", cutting.name()),
        (Some(cutting), None) => cutting.name().to_string(),
        (None, _) => "–".to_string(),
    }
}

fn standtitle(standnumber: Option<i64>, standid: i64) -> String {
    match standnumber {
        Some(number) => number.to_string(),
        None => standid.to_string(),
    }
}

fn stands_table(ui: &mut egui::Ui, values: &[StandValue]) {
    if values.is_empty() {
        ui.label(egui::RichText::new("Ei kuvioita.").color(MUTED));
        return;
    }

    let available = ui.available_height().max(120.0);
    TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::auto().at_least(56.0))
        .column(Column::auto().at_least(120.0))
        .column(Column::auto().at_least(64.0))
        .column(Column::auto().at_least(86.0))
        .column(Column::auto().at_least(78.0))
        .column(Column::auto().at_least(78.0))
        .column(Column::auto().at_least(72.0))
        .column(Column::remainder().at_least(90.0))
        .max_scroll_height(available)
        .header(24.0, |mut header| {
            for title in [
                "Kuvio",
                "Hakkuu",
                "Ala, ha",
                "Tilavuus m³/ha",
                "Tukkia m³/ha",
                "Kuitua m³/ha",
                "Kasvu, %",
                "Netto, €",
            ] {
                header.col(|ui| {
                    ui.strong(title);
                });
            }
        })
        .body(|body| {
            body.rows(22.0, values.len(), |mut row| {
                let value = &values[row.index()];
                let is_negative = value.net_eur < 0.0;

                row.col(|ui| {
                    ui.monospace(standtitle(value.standnumber, value.standid));
                });
                row.col(|ui| {
                    ui.label(cutting_text(value.cutting, value.cuttingproposalyear));
                });
                row.col(|ui| {
                    ui.monospace(
                        value
                            .area
                            .map(|a| util::fmt_fi(a, 2))
                            .unwrap_or_else(|| "–".into()),
                    );
                });
                row.col(|ui| {
                    ui.monospace(util::fmt_fi(value.volume, 1));
                });
                row.col(|ui| {
                    ui.monospace(util::fmt_fi(value.sawlogvolume, 1));
                });
                row.col(|ui| {
                    ui.monospace(util::fmt_fi(value.pulpwoodvolume, 1));
                });
                row.col(|ui| {
                    ui.monospace(
                        value
                            .growth_pct
                            .map(|p| util::fmt_fi(p, 1))
                            .unwrap_or_else(|| "–".into()),
                    );
                });
                row.col(|ui| {
                    ui.colored_label(
                        if is_negative { RED } else { GREEN },
                        util::fmt_eur(value.net_eur),
                    );
                });
            });
        });
}

fn species_table(ui: &mut egui::Ui, values: &[SpeciesSummary]) {
    if values.is_empty() {
        ui.label(egui::RichText::new("Ei puulajitietoja.").color(MUTED));
        return;
    }

    let available = ui.available_height().max(120.0);
    TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::auto().at_least(56.0))
        .column(Column::auto().at_least(80.0))
        .column(Column::auto().at_least(120.0))
        .column(Column::auto().at_least(60.0))
        .column(Column::auto().at_least(90.0))
        .column(Column::auto().at_least(84.0))
        .column(Column::remainder().at_least(90.0))
        .max_scroll_height(available)
        .header(24.0, |mut header| {
            for title in [
                "Kuvio",
                "Puulaji",
                "Hakkuu",
                "Ikä, v",
                "Tilavuus m³/ha",
                "Tukkia m³/ha",
                "Arvo, €",
            ] {
                header.col(|ui| {
                    ui.strong(title);
                });
            }
        })
        .body(|body| {
            body.rows(22.0, values.len(), |mut row| {
                let value = &values[row.index()];

                row.col(|ui| {
                    ui.monospace(standtitle(value.standnumber, value.standid));
                });
                row.col(|ui| {
                    ui.label(value.species.name());
                });
                row.col(|ui| {
                    ui.label(cutting_text(value.cutting, value.cuttingproposalyear));
                });
                row.col(|ui| {
                    ui.monospace(
                        value
                            .age
                            .map(|age| util::fmt_fi(age, 0))
                            .unwrap_or_else(|| "–".into()),
                    );
                });
                row.col(|ui| {
                    ui.monospace(util::fmt_fi(value.volume, 1));
                });
                row.col(|ui| {
                    ui.monospace(util::fmt_fi(value.sawlogvolume, 1));
                });
                row.col(|ui| {
                    ui.colored_label(GREEN, util::fmt_eur(value.value_eur));
                });
            });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cutting_text_combines_type_and_year() {
        assert_eq!(
            cutting_text(Some(Cutting::Uudistus), Some(2030)),
            "uudistus 2030"
        );
        assert_eq!(cutting_text(Some(Cutting::Harvennus), None), "harvennus");
        assert_eq!(cutting_text(None, Some(2030)), "–");
    }

    #[test]
    fn stand_title_prefers_the_stand_number() {
        assert_eq!(standtitle(Some(7), 100), "7");
        assert_eq!(standtitle(None, 100), "100");
    }
}
