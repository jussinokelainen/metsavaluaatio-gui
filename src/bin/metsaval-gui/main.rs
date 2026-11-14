//! Metsävaluuation graafinen käyttöliittymä.
//!
//! Ohjelma käyttää samoja `metsaval`-kirjaston moduuleja kuin komentorivityökalu,
//! mutta hoitaa kyselyt ja tulosteet GPUI-ikkunassa. Verkkohaku ja tiedostojen
//! kirjoitus ajetaan taustasäikeissä, jotta käyttöliittymä pysyy responsiivisena.

mod form;
mod pipeline;
mod settings;
mod text_input;
mod ui;
mod util;

use form::{RunForm, prepare_run};

use std::collections::HashMap;
use std::ops::Range;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use gpui::{
    AnyElement, App, Application, Bounds, Context, Div, Entity, FontWeight, Hsla, IntoElement,
    Render, ScrollHandle, SharedString, TitlebarOptions, Window, WindowBounds, WindowOptions, div,
    prelude::*, px, rgb, size,
};
use metsaval::consts::Cutting;
use metsaval::model::{SpeciesSummary, StandValue};

use pipeline::{Options, Valuation};
use text_input::TextInput;
use ui::{
    amber, bg, border, checkbox_row, green, input_bg, muted, panel, primary_button, red,
    secondary_button, small_button, stat_card, tab_button, text,
};

fn main() {
    let app = Application::new();

    app.on_reopen(|cx| {
        if cx.windows().is_empty() {
            open_main_window(cx);
        } else {
            cx.activate(true);
        }
    });

    app.run(|cx: &mut App| {
        cx.bind_keys(text_input::key_bindings());
        open_main_window(cx);
    });
}

fn open_main_window(cx: &mut App) {
    let bounds = Bounds::centered(None, size(px(1120.), px(780.)), cx);
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(840.), px(560.))),
            titlebar: Some(TitlebarOptions {
                title: Some("Metsävaluaatio".into()),
                ..Default::default()
            }),
            app_id: Some("metsaval".to_string()),
            ..Default::default()
        },
        |_, cx| cx.new(MetsavalApp::new),
    )
    .expect("ikkunan avaaminen epäonnistui");

    cx.activate(true);
}

/// Yläpalkin tilat.
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
    api_key: Entity<TextInput>,
    show_api_key: bool,
    show_advanced: bool,
    municipalities: HashMap<String, String>,

    // Hakukanavan kentät
    property_id: Entity<TextInput>,
    municipality_input: Entity<TextInput>,
    use_gpkg: bool,
    gpkg_path: Entity<TextInput>,
    gpkg_download_dir: Entity<TextInput>,
    output_dir: Entity<TextInput>,

    // Tila
    running: bool,
    exporting: bool,
    result: Option<Box<Valuation>>,
    error: Option<String>,
    notice: Option<String>,
    logs: Vec<String>,
    log_scroll: ScrollHandle,
    tab: Tab,
    exported: Vec<PathBuf>,

    // Taustatyön kanavat
    log_rx: Option<Receiver<String>>,
    done_rx: Option<Receiver<Result<Box<Valuation>, String>>>,
    export_rx: Option<Receiver<ExportMessage>>,
}

impl MetsavalApp {
    fn new(cx: &mut Context<Self>) -> Self {
        let api_key_text = settings::load_api_key().unwrap_or_default();
        let output_dir_text = dirs::download_dir()
            .or_else(dirs::home_dir)
            .map(|path| path.display().to_string())
            .unwrap_or_default();

        let api_key = cx.new(|cx| TextInput::new(cx, "MML API-avain", true));
        api_key.update(cx, |input, cx| input.set_content(api_key_text.clone(), cx));

        let output_dir = cx.new(|cx| TextInput::new(cx, "esim. ~/Downloads", false));
        output_dir.update(cx, |input, cx| input.set_content(output_dir_text, cx));

        let app = Self {
            api_key,
            show_api_key: false,
            show_advanced: api_key_text.trim().is_empty(),
            municipalities: settings::load_municipalities(),
            property_id: cx.new(|cx| TextInput::new(cx, "esim. 740-555-2-4", false)),
            municipality_input: cx.new(|cx| TextInput::new(cx, "Kunnan nimi", false)),
            use_gpkg: false,
            gpkg_path: cx.new(|cx| TextInput::new(cx, "/polku/MV_Kunta.gpkg", false)),
            gpkg_download_dir: cx
                .new(|cx| TextInput::new(cx, "oletus: järjestelmän väliaikaiskansio", false)),
            output_dir,
            running: false,
            exporting: false,
            result: None,
            error: None,
            notice: None,
            logs: Vec::new(),
            log_scroll: ScrollHandle::new(),
            tab: Tab::Stands,
            exported: Vec::new(),
            log_rx: None,
            done_rx: None,
            export_rx: None,
        };

        // Tekstikentät ovat omia GPUI-entiteettejään, joten niiden
        // muutoksista pitää kertoa myös pää-näkymälle. Muuten esimerkiksi
        // API-avaimen tila ja kunnan nimi eivät päivity kirjoittaessa.
        for input in [
            app.api_key.clone(),
            app.property_id.clone(),
            app.municipality_input.clone(),
            app.gpkg_path.clone(),
            app.gpkg_download_dir.clone(),
            app.output_dir.clone(),
        ] {
            cx.observe(&input, |_, _, cx| cx.notify()).detach();
        }

        app.start_background_poller(cx);
        app
    }

    /// Taustasäikeiden kanavia pollataan säännöllisesti GPUI:n
    /// foreground-executorissa.
    fn start_background_poller(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                gpui::Timer::after(Duration::from_millis(100)).await;

                let alive = this.update(cx, |this, cx| {
                    if this.poll() {
                        cx.notify();
                    }
                });

                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    /// Tämänhetkinen kunta-koodi joko täydennetystä tai raa'asta tunnuksesta.
    fn current_code(&self, cx: &App) -> Option<String> {
        util::property_code(&input_text(cx, &self.property_id))
    }

    /// Käynnistää haun taustasäikeessä.
    fn start_run(&mut self, cx: &mut Context<Self>) {
        if self.running {
            return;
        }

        self.error = None;
        self.notice = None;

        let form = RunForm {
            api_key: input_text(cx, &self.api_key),
            property_id: input_text(cx, &self.property_id),
            municipality_input: input_text(cx, &self.municipality_input),
            use_gpkg: self.use_gpkg,
            gpkg_path: input_text(cx, &self.gpkg_path),
            gpkg_download_dir: input_text(cx, &self.gpkg_download_dir),
        };

        let prepared = match prepare_run(&form, &mut self.municipalities) {
            Ok(prepared) => prepared,
            Err(error) => {
                self.error = Some(error);
                cx.notify();
                return;
            }
        };

        if prepared.municipality_added {
            settings::save_municipalities(&self.municipalities);
            set_input(cx, &self.municipality_input, "");
        }

        // Tallennetaan avain, jotta sitä ei tarvitse syöttää uudelleen.
        if let Err(error) = settings::save_api_key(&prepared.api_key) {
            self.notice = Some(format!("API-avaimen tallennus epäonnistui: {error:#}"));
        }

        let options = Options {
            property_id: prepared.property_id,
            api_key: prepared.api_key,
            municipality: prepared.municipality,
            gpkg_path: prepared.gpkg_path,
            gpkg_download_dir: prepared.gpkg_download_dir,
        };

        self.logs.clear();
        self.logs.push("Aloitetaan haku...".to_string());
        self.log_scroll.scroll_to_bottom();
        self.result = None;
        self.exported.clear();
        self.running = true;
        self.tab = Tab::Stands;

        let (log_tx, log_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        self.log_rx = Some(log_rx);
        self.done_rx = Some(done_rx);

        std::thread::spawn(move || {
            let result = run_worker(options, log_tx);
            let _ = done_tx.send(result);
        });

        cx.notify();
    }

    /// Kirjoittaa Excel- ja KML-tiedostot valittuun kansioon.
    fn start_export(&mut self, cx: &mut Context<Self>) {
        if self.exporting {
            return;
        }
        let Some(valuation) = self.result.as_ref() else {
            return;
        };

        let dir_text = input_text(cx, &self.output_dir);
        if dir_text.is_empty() {
            self.error = Some("Anna tulostekansio.".to_string());
            cx.notify();
            return;
        }
        let dir = util::expand_path(&dir_text);

        let valuation = valuation.clone();
        let property_id = valuation.property_id.clone();
        let (tx, rx) = mpsc::channel();
        self.export_rx = Some(rx);
        self.exporting = true;
        self.exported.clear();
        self.error = None;
        self.notice = None;

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
        });

        cx.notify();
    }

    /// Lukee taustasäikeiden viestit. Palauttaa `true`, jos tila muuttui.
    fn poll(&mut self) -> bool {
        let mut changed = false;

        if let Some(rx) = self.log_rx.take() {
            for line in rx.try_iter() {
                self.logs.push(line);
                changed = true;
            }
            if changed {
                self.log_scroll.scroll_to_bottom();
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
                    changed = true;
                }
                Ok(Err(error)) => {
                    self.error = Some(error);
                    self.running = false;
                    self.log_rx = None;
                    changed = true;
                }
                Err(mpsc::TryRecvError::Empty) => {
                    self.done_rx = Some(rx);
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.running = false;
                    self.log_rx = None;
                    self.error = Some("Taustatyö keskeytyi odottamatta.".to_string());
                    changed = true;
                }
            }
        }

        if let Some(rx) = self.export_rx.take() {
            match rx.try_recv() {
                Ok(ExportMessage::Done(paths)) => {
                    self.exported = paths;
                    self.exporting = false;
                    self.notice = Some("Tiedostot tallennettu.".to_string());
                    changed = true;
                }
                Ok(ExportMessage::Error(error)) => {
                    self.exporting = false;
                    self.error = Some(error);
                    changed = true;
                }
                Err(mpsc::TryRecvError::Empty) => {
                    self.export_rx = Some(rx);
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.exporting = false;
                    self.error = Some("Tiedostojen tallennus keskeytyi.".to_string());
                    changed = true;
                }
            }
        }

        changed
    }

    fn header(&self, cx: &App) -> AnyElement {
        let has_key = !input_text(cx, &self.api_key).is_empty();

        div()
            .flex()
            .items_center()
            .justify_between()
            .h(px(58.))
            .px_5()
            .bg(panel())
            .border_b_1()
            .border_color(border())
            .child(
                div().flex().items_center().gap_3().child(
                    div()
                        .text_size(px(20.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(text())
                        .child("Metsävaluaatio"),
                ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_size(px(12.))
                    .text_color(if has_key { green() } else { amber() })
                    .child(if has_key {
                        "● API-avain asetettu"
                    } else {
                        "● API-avain puuttuu"
                    }),
            )
            .into_any_element()
    }

    fn sidebar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let form = self.form(cx);

        div()
            .flex()
            .flex_col()
            .h_full()
            .w(px(360.))
            .flex_shrink_0()
            .bg(panel())
            .border_r_1()
            .border_color(border())
            .child(
                div()
                    .id("sidebar-scroll")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .p_4()
                    .child(form),
            )
            .into_any_element()
    }

    fn form(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let code = util::pad_municipality_code(self.current_code(cx));
        let known = code
            .as_ref()
            .and_then(|code| self.municipalities.get(code))
            .cloned();

        let mut form = div().flex().flex_col().gap_3().w_full();

        form = form
            .child(section_title("Haku"))
            .child(field_label("Kiinteistötunnus"))
            .child(self.property_id.clone());

        form = form.child(match (self.use_gpkg, known) {
            (true, _) => muted_text("Käytetään valmista GeoPackagea, kuntaa ei tarvita."),
            (false, Some(name)) => div()
                .flex()
                .flex_col()
                .gap_1()
                .child(field_label("Kunta"))
                .child(
                    div()
                        .text_size(px(13.))
                        .text_color(text())
                        .child(format!("{name} ({})", code.unwrap_or_default())),
                )
                .into_any_element(),
            (false, None) => match code {
                Some(code) => div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(field_label(format!("Kunta (koodi {code})")))
                    .child(self.municipality_input.clone())
                    .child(muted_text(
                        "Nimi tallennetaan, jotta sitä ei kysytä uudelleen.",
                    ))
                    .into_any_element(),
                None => muted_text("Syötä ensin kiinteistötunnus."),
            },
        });

        // Lisäasetukset-kokonaisuus. GPUI:ssa ei ole valmista haitarikomponenttia,
        // joten avaaminen hoidetaan omalla rivillä.
        let advanced_label = if self.show_advanced {
            "▾ Lisäasetukset"
        } else {
            "▸ Lisäasetukset"
        };
        form = form.child(
            div()
                .id("advanced-toggle")
                .flex()
                .items_center()
                .gap_1()
                .cursor_pointer()
                .text_size(px(13.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(muted())
                .hover(|style| style.text_color(text()))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.show_advanced = !this.show_advanced;
                    cx.notify();
                }))
                .child(advanced_label),
        );

        if self.show_advanced {
            let show_key = self.show_api_key;
            let mut advanced = div()
                .flex()
                .flex_col()
                .gap_2()
                .p_3()
                .bg(bg())
                .border_1()
                .border_color(border())
                .rounded_md()
                .child(field_label("MML API-avain"))
                .child(self.api_key.clone())
                .child(checkbox_row(
                    "show-api-key",
                    "Näytä avain",
                    show_key,
                    cx.listener(|this, _, _, cx| {
                        this.show_api_key = !this.show_api_key;
                        let masked = !this.show_api_key;
                        this.api_key
                            .update(cx, |input, cx| input.set_masked(masked, cx));
                        cx.notify();
                    }),
                ))
                .child(secondary_button(
                    "save-api-key",
                    "Tallenna avain",
                    true,
                    cx.listener(|this, _, _, cx| {
                        let key = input_text(cx, &this.api_key);
                        match settings::save_api_key(&key) {
                            Ok(path) => {
                                this.notice =
                                    Some(format!("API-avain tallennettu: {}", path.display()));
                                this.error = None;
                            }
                            Err(error) => {
                                this.error =
                                    Some(format!("API-avaimen tallennus epäonnistui: {error:#}"));
                            }
                        }
                        cx.notify();
                    }),
                ))
                .child(div().h(px(1.)).w_full().bg(border()))
                .child(checkbox_row(
                    "use-gpkg",
                    "Käytä valmista GeoPackagea",
                    self.use_gpkg,
                    cx.listener(|this, _, _, cx| {
                        this.use_gpkg = !this.use_gpkg;
                        cx.notify();
                    }),
                ));

            if self.use_gpkg {
                advanced = advanced.child(self.gpkg_path.clone()).child(muted_text(
                    "Vältä lataus, kun haet useita kiinteistöjä samasta kunnasta.",
                ));
            } else {
                advanced = advanced
                    .child(field_label("GeoPackagen latauskansio (valinnainen)"))
                    .child(self.gpkg_download_dir.clone());
            }

            form = form.child(advanced);
        }

        let button_text = if self.running {
            "Haetaan..."
        } else {
            "Hae arvio"
        };
        form = form.child(primary_button(
            "run",
            button_text,
            !self.running,
            cx.listener(|this, _, _, cx| this.start_run(cx)),
        ));

        if self.running {
            form = form.child(muted_text("Haetaan tietoja verkosta..."));
        }

        if let Some(notice) = &self.notice {
            form = form.child(
                div()
                    .text_size(px(13.))
                    .text_color(green())
                    .child(notice.clone()),
            );
        }

        form = form
            .child(div().h(px(1.)).w_full().bg(border()))
            .child(field_label("Tulostekansio"))
            .child(self.output_dir.clone())
            .child(muted_text(
                "Excel- ja KML-tiedostot kirjoitetaan tähän kansioon.",
            ))
            .child(secondary_button(
                "export",
                "Tallenna Excel + KML",
                self.result.is_some() && !self.exporting && !self.running,
                cx.listener(|this, _, _, cx| this.start_export(cx)),
            ));

        if self.exporting {
            form = form.child(muted_text("Kirjoitetaan tiedostoja..."));
        }

        if !self.exported.is_empty() {
            let mut exported = div().flex().flex_col().gap_1();
            for path in &self.exported {
                exported = exported.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(div().text_color(green()).child("✓"))
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .text_size(px(11.))
                                .font_family("monospace")
                                .text_color(text())
                                .child(path.display().to_string()),
                        ),
                );
            }
            form = form.child(exported);
        }

        form = form.child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(small_button("open-mymaps", "Google My Maps", |_, _, _| {
                    metsaval::maps::open_browser("https://www.google.com/mymaps");
                }))
                .child(small_button(
                    "open-output",
                    "Avaa tulostekansio",
                    cx.listener(|this, _, _, cx| {
                        let dir = input_text(cx, &this.output_dir);
                        if !dir.is_empty() {
                            metsaval::maps::open_browser(
                                &util::expand_path(&dir).display().to_string(),
                            );
                        }
                    }),
                )),
        );

        form.into_any_element()
    }

    fn welcome(&self) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .p_5()
            .bg(panel())
            .border_1()
            .border_color(border())
            .rounded_lg()
            .child(
                div()
                    .text_size(px(22.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(text())
                    .child("Metsäomaisuuden arvio"),
            )
            .child(muted_text(
                "Syötä kiinteistötunnus ja hae arvio. Ohjelma hakee kiinteistön rajat, \
                 metsävarakuviot ja puustotiedot sekä määrittää laskennallisen arvon.",
            ))
            .child(muted_text(
                "Tulokset voi tallentaa Excel-yhteenvetona ja Google Maps -tasona (KML).",
            ))
            .into_any_element()
    }

    fn content(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let mut root = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w(px(0.))
            .min_h(px(0.))
            .gap_3()
            .p_5()
            .bg(bg());

        let mut error_banner = None;
        if let Some(error) = &self.error {
            error_banner = Some(
                div()
                    .p_3()
                    .bg(rgb(0x3a1818))
                    .border_1()
                    .border_color(rgb(0x963737))
                    .rounded_md()
                    .text_size(px(13.))
                    .text_color(red())
                    .child(error.clone())
                    .into_any_element(),
            );
        }

        if let Some(error) = error_banner {
            root = root.child(error);
        }

        let Some(valuation) = self.result.as_ref() else {
            return root
                .child(self.welcome())
                .child(section_title("Tapahtumat"))
                .child(self.log_view())
                .into_any_element();
        };

        root = root
            .child(self.result_header(valuation))
            .child(self.stats(valuation))
            .child(self.tabs_row(cx));

        root = root.child(match self.tab {
            Tab::Stands => self.stands_table(cx),
            Tab::Species => self.species_table(cx),
            Tab::Log => self.log_view(),
        });

        root.into_any_element()
    }

    fn result_header(&self, valuation: &Valuation) -> AnyElement {
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .text_size(px(22.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(text())
                            .child(valuation.property_id.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(muted())
                            .child(valuation.municipality.clone().unwrap_or_default()),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .flex()
                    .justify_end()
                    .text_size(px(11.))
                    .text_color(muted())
                    .child(format!(
                        "{} palstaa · {} · {}",
                        valuation.parcel_count,
                        valuation.stand_layer,
                        valuation.gpkg_path.display()
                    )),
            )
            .into_any_element()
    }

    fn stats(&self, valuation: &Valuation) -> AnyElement {
        let net_color = if valuation.total_value() >= 0.0 {
            green()
        } else {
            red()
        };
        let neutral = text();

        div()
            .flex()
            .flex_wrap()
            .gap_2()
            .child(stat_card(
                "Laskennallinen arvo",
                util::fmt_eur(valuation.total_value()),
                net_color,
            ))
            .child(stat_card(
                "Kiinteistön ala",
                format!("{} ha", util::fmt_fi(valuation.property_area_ha, 2)),
                neutral,
            ))
            .child(stat_card(
                "Kuvioiden ala",
                format!("{} ha", util::fmt_fi(valuation.totals.area_ha, 2)),
                neutral,
            ))
            .child(stat_card(
                "Puusto",
                format!("{} m³", util::fmt_fi(valuation.totals.volume_m3, 0)),
                neutral,
            ))
            .child(stat_card(
                "Tulot",
                util::fmt_eur(valuation.totals.income_eur),
                neutral,
            ))
            .child(stat_card(
                "Muuttuvat kulut",
                util::fmt_eur(valuation.totals.costs_eur),
                neutral,
            ))
            .child(stat_card(
                "Kiinteät kulut",
                util::fmt_eur(valuation.fixed_costs),
                neutral,
            ))
            .child(stat_card(
                "Kuvioita",
                valuation.totals.stand_count.to_string(),
                neutral,
            ))
            .into_any_element()
    }

    fn tabs_row(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex()
            .items_center()
            .gap_1()
            .child(tab_button(
                "tab-stands",
                "Kuviot",
                self.tab == Tab::Stands,
                cx.listener(|this, _, _, cx| {
                    this.tab = Tab::Stands;
                    cx.notify();
                }),
            ))
            .child(tab_button(
                "tab-species",
                "Puulajit",
                self.tab == Tab::Species,
                cx.listener(|this, _, _, cx| {
                    this.tab = Tab::Species;
                    cx.notify();
                }),
            ))
            .child(tab_button(
                "tab-log",
                "Loki",
                self.tab == Tab::Log,
                cx.listener(|this, _, _, cx| {
                    this.tab = Tab::Log;
                    cx.notify();
                }),
            ))
            .child(div().flex_1())
            .child(small_button(
                "open-mymaps-result",
                "Avaa Google My Maps",
                |_, _, _| {
                    metsaval::maps::open_browser("https://www.google.com/mymaps");
                },
            ))
            .into_any_element()
    }

    fn log_view(&self) -> AnyElement {
        div()
            .id("log-scroll")
            .flex()
            .flex_col()
            .gap_1()
            .flex_1()
            .min_h(px(0.))
            .p_3()
            .bg(panel())
            .border_1()
            .border_color(border())
            .rounded_md()
            .overflow_y_scroll()
            .track_scroll(&self.log_scroll)
            .when(self.logs.is_empty(), |view| {
                view.child(muted_text("Ei tapahtumia."))
            })
            .children(
                self.logs
                    .iter()
                    .map(|line| {
                        div()
                            .text_size(px(12.))
                            .font_family("monospace")
                            .text_color(text())
                            .child(line.clone())
                    })
                    .collect::<Vec<_>>(),
            )
            .into_any_element()
    }

    fn stands_table(&self, cx: &mut Context<Self>) -> AnyElement {
        let count = self
            .result
            .as_ref()
            .map(|valuation| valuation.stand_values.len())
            .unwrap_or(0);

        if count == 0 {
            return muted_text("Ei kuvioita.");
        }

        let rows = cx.processor(move |this, range: Range<usize>, _, _| {
            let Some(valuation) = this.result.as_ref() else {
                return Vec::new();
            };
            range
                .filter_map(|index| {
                    valuation
                        .stand_values
                        .get(index)
                        .map(|value| stand_row(index, value))
                })
                .collect::<Vec<_>>()
        });

        table("stands", &STAND_COLUMNS, count, rows)
    }

    fn species_table(&self, cx: &mut Context<Self>) -> AnyElement {
        let count = self
            .result
            .as_ref()
            .map(|valuation| valuation.species_summary.len())
            .unwrap_or(0);

        if count == 0 {
            return muted_text("Ei puulajitietoja.");
        }

        let rows = cx.processor(move |this, range: Range<usize>, _, _| {
            let Some(valuation) = this.result.as_ref() else {
                return Vec::new();
            };
            range
                .filter_map(|index| {
                    valuation
                        .species_summary
                        .get(index)
                        .map(|value| species_row(index, value))
                })
                .collect::<Vec<_>>()
        });

        table("species", &SPECIES_COLUMNS, count, rows)
    }
}

impl Render for MetsavalApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let header = self.header(cx);
        let sidebar = self.sidebar(cx);
        let content = self.content(cx);

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(bg())
            .text_size(px(13.))
            .text_color(text())
            .child(header)
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h(px(0.))
                    .child(sidebar)
                    .child(content),
            )
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

fn input_text(cx: &App, input: &Entity<TextInput>) -> String {
    input.read(cx).content().trim().to_string()
}

fn set_input(
    cx: &mut Context<MetsavalApp>,
    input: &Entity<TextInput>,
    value: impl Into<SharedString>,
) {
    input.update(cx, |input, cx| input.set_content(value, cx));
}

fn section_title(title: impl Into<SharedString>) -> AnyElement {
    div()
        .text_size(px(15.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(text())
        .child(title.into())
        .into_any_element()
}

fn field_label(label: impl Into<SharedString>) -> AnyElement {
    div()
        .text_size(px(12.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(muted())
        .child(label.into())
        .into_any_element()
}

fn muted_text(message: impl Into<SharedString>) -> AnyElement {
    div()
        .text_size(px(12.))
        .text_color(muted())
        .child(message.into())
        .into_any_element()
}

// ---------------------------------------------------------------------------
// Taulukot
// ---------------------------------------------------------------------------

const ROW_HEIGHT: f32 = 26.0;

/// Sarakkeen otsikko ja leveys. Leveys 0 tarkoittaa joustavaa saraketta.
struct Column {
    title: &'static str,
    width: f32,
    monospace: bool,
}

const STAND_COLUMNS: [Column; 8] = [
    Column {
        title: "Kuvio",
        width: 60.0,
        monospace: true,
    },
    Column {
        title: "Hakkuu",
        width: 110.0,
        monospace: false,
    },
    Column {
        title: "Ala, ha",
        width: 70.0,
        monospace: true,
    },
    Column {
        title: "Tilavuus m³/ha",
        width: 95.0,
        monospace: true,
    },
    Column {
        title: "Tukkia m³/ha",
        width: 95.0,
        monospace: true,
    },
    Column {
        title: "Kuitua m³/ha",
        width: 95.0,
        monospace: true,
    },
    Column {
        title: "Kasvu, %",
        width: 70.0,
        monospace: true,
    },
    Column {
        title: "Netto, €",
        width: 0.0,
        monospace: false,
    },
];

const SPECIES_COLUMNS: [Column; 7] = [
    Column {
        title: "Kuvio",
        width: 70.0,
        monospace: true,
    },
    Column {
        title: "Puulaji",
        width: 100.0,
        monospace: false,
    },
    Column {
        title: "Hakkuu",
        width: 130.0,
        monospace: false,
    },
    Column {
        title: "Ikä, v",
        width: 70.0,
        monospace: true,
    },
    Column {
        title: "Tilavuus m³/ha",
        width: 110.0,
        monospace: true,
    },
    Column {
        title: "Tukkia m³/ha",
        width: 110.0,
        monospace: true,
    },
    Column {
        title: "Arvo, €",
        width: 0.0,
        monospace: false,
    },
];

fn table(
    id: &'static str,
    columns: &'static [Column],
    count: usize,
    rows: impl Fn(Range<usize>, &mut Window, &mut App) -> Vec<AnyElement> + 'static,
) -> AnyElement {
    let content_width: f32 = columns
        .iter()
        .map(|column| {
            if column.width <= 0.0 {
                100.0
            } else {
                column.width
            }
        })
        .sum::<f32>()
        + 16.0;

    let list_id = SharedString::from(format!("{id}-list"));

    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_h(px(0.))
        .bg(panel())
        .border_1()
        .border_color(border())
        .rounded_md()
        .overflow_hidden()
        .child(
            div()
                .id(id)
                .flex()
                .flex_col()
                .flex_1()
                .min_h(px(0.))
                .overflow_x_scroll()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_h(px(0.))
                        .min_w(px(content_width))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .h(px(30.))
                                .px_2()
                                .bg(input_bg())
                                .border_b_1()
                                .border_color(border())
                                .text_size(px(11.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(muted())
                                .children(columns.iter().map(|column| {
                                    column_cell(column)
                                        .text_size(px(11.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(muted())
                                        .child(column.title)
                                })),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .flex_1()
                                .min_h(px(0.))
                                .child(gpui::uniform_list(list_id, count, rows).flex_1()),
                        ),
                ),
        )
        .into_any_element()
}

fn column_cell(column: &Column) -> Div {
    let cell = div()
        .flex()
        .items_center()
        .h_full()
        .px_2()
        .overflow_hidden()
        .whitespace_nowrap();

    let cell = if column.monospace {
        cell.font_family("monospace")
    } else {
        cell
    };

    if column.width <= 0.0 {
        cell.flex_1().min_w(px(100.))
    } else {
        cell.flex_shrink_0().w(px(column.width))
    }
}

fn monospace(value: impl Into<SharedString>, color: Hsla) -> Div {
    div()
        .text_size(px(12.))
        .font_family("monospace")
        .text_color(color)
        .child(value.into())
}

fn text_cell(value: impl Into<SharedString>, color: Hsla) -> Div {
    div()
        .text_size(px(12.))
        .text_color(color)
        .child(value.into())
}

fn row_shell(index: usize) -> Div {
    div()
        .flex()
        .items_center()
        .h(px(ROW_HEIGHT))
        .px_2()
        .border_b_1()
        .border_color(border().opacity(0.45))
        .when(index % 2 == 1, |row| row.bg(input_bg().opacity(0.35)))
}

fn stand_row(index: usize, value: &StandValue) -> AnyElement {
    let net_color = if value.net_eur < 0.0 { red() } else { green() };
    let area = value
        .area
        .map(|a| util::fmt_fi(a, 2))
        .unwrap_or_else(|| "–".into());
    let growth = value
        .growth_pct
        .map(|p| util::fmt_fi(p, 1))
        .unwrap_or_else(|| "–".into());

    let mut row = row_shell(index);

    for column in &STAND_COLUMNS {
        let cell = column_cell(column);
        row = row.child(match column.title {
            "Kuvio" => cell.child(monospace(
                standtitle(value.standnumber, value.standid),
                text(),
            )),
            "Hakkuu" => cell.child(text_cell(
                cutting_text(value.cutting, value.cuttingproposalyear),
                text(),
            )),
            "Ala, ha" => cell.child(monospace(area.clone(), text())),
            "Tilavuus m³/ha" => cell.child(monospace(util::fmt_fi(value.volume, 1), text())),
            "Tukkia m³/ha" => cell.child(monospace(util::fmt_fi(value.sawlogvolume, 1), text())),
            "Kuitua m³/ha" => cell.child(monospace(util::fmt_fi(value.pulpwoodvolume, 1), text())),
            "Kasvu, %" => cell.child(monospace(growth.clone(), text())),
            _ => cell.child(text_cell(util::fmt_eur(value.net_eur), net_color)),
        });
    }

    row.into_any_element()
}

fn species_row(index: usize, value: &SpeciesSummary) -> AnyElement {
    let age = value
        .age
        .map(|age| util::fmt_fi(age, 0))
        .unwrap_or_else(|| "–".into());

    let mut row = row_shell(index);

    for column in &SPECIES_COLUMNS {
        let cell = column_cell(column);
        row = row.child(match column.title {
            "Kuvio" => cell.child(monospace(
                standtitle(value.standnumber, value.standid),
                text(),
            )),
            "Puulaji" => cell.child(text_cell(value.species.name(), text())),
            "Hakkuu" => cell.child(text_cell(
                cutting_text(value.cutting, value.cuttingproposalyear),
                text(),
            )),
            "Ikä, v" => cell.child(monospace(age.clone(), text())),
            "Tilavuus m³/ha" => cell.child(monospace(util::fmt_fi(value.volume, 1), text())),
            "Tukkia m³/ha" => cell.child(monospace(util::fmt_fi(value.sawlogvolume, 1), text())),
            _ => cell.child(text_cell(util::fmt_eur(value.value_eur), green())),
        });
    }

    row.into_any_element()
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
