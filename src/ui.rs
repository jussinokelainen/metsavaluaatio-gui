//! Yhteiset värit ja pienet uudelleenkäytettävät UI-palaset.

use gpui_kit::{
    App, ClickEvent, FontWeight, Hsla, IntoElement, SharedString, Window,
    component::{Icon, IconName},
    div,
    prelude::*,
    px, rgb,
};

// Värit ovat hex-arvoja, jotta ne on helppo pitää yhdessä paikassa.
const BG_HEX: u32 = 0x191c22;
const PANEL_HEX: u32 = 0x21252e;
const INPUT_BG_HEX: u32 = 0x2a2f3a;
const BORDER_HEX: u32 = 0x3a4150;
const TEXT_HEX: u32 = 0xe6e9ef;
const MUTED_HEX: u32 = 0x969ba5;
const GREEN_HEX: u32 = 0x58d68d;
const RED_HEX: u32 = 0xff7878;
const AMBER_HEX: u32 = 0xf0b450;
const ACCENT_HEX: u32 = 0x4f7cff;

pub fn bg() -> Hsla {
    rgb(BG_HEX).into()
}

pub fn panel() -> Hsla {
    rgb(PANEL_HEX).into()
}

pub fn input_bg() -> Hsla {
    rgb(INPUT_BG_HEX).into()
}

pub fn border() -> Hsla {
    rgb(BORDER_HEX).into()
}

pub fn text() -> Hsla {
    rgb(TEXT_HEX).into()
}

pub fn muted() -> Hsla {
    rgb(MUTED_HEX).into()
}

pub fn green() -> Hsla {
    rgb(GREEN_HEX).into()
}

pub fn red() -> Hsla {
    rgb(RED_HEX).into()
}

pub fn amber() -> Hsla {
    rgb(AMBER_HEX).into()
}

pub fn accent() -> Hsla {
    rgb(ACCENT_HEX).into()
}

/// Ensisijainen, koko leveyden painike.
pub fn primary_button(
    id: &'static str,
    label: impl Into<SharedString>,
    enabled: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let label = label.into();
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .w_full()
        .h(px(38.))
        .rounded_md()
        .bg(accent())
        .text_color(rgb(0xffffff))
        .font_weight(FontWeight::SEMIBOLD)
        .when(enabled, |button| {
            button
                .cursor_pointer()
                .hover(|style| style.bg(accent().opacity(0.85)))
                .on_click(on_click)
        })
        .when(!enabled, |button| button.opacity(0.45))
        .child(label)
}

/// Toissijainen, koko leveyden painike.
pub fn secondary_button(
    id: &'static str,
    label: impl Into<SharedString>,
    enabled: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    text_color: Hsla,
) -> impl IntoElement {
    let label = label.into();
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .w_full()
        .h(px(34.))
        .rounded_md()
        .bg(input_bg())
        .border_1()
        .border_color(border())
        .text_color(text_color)
        .when(enabled, |button| {
            button
                .cursor_pointer()
                .hover(|style| style.bg(border()))
                .on_click(on_click)
        })
        .when(!enabled, |button| button.opacity(0.45))
        .child(label)
}

/// Pieni painike työkaluriville.
pub fn small_button(
    id: &'static str,
    label: impl Into<SharedString>,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let label = label.into();
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .h(px(28.))
        .px_3()
        .rounded_md()
        .bg(input_bg())
        .border_1()
        .border_color(border())
        .text_color(text())
        .cursor_pointer()
        .hover(|style| style.bg(border()))
        .on_click(on_click)
        .child(label)
}

/// Välilehden painike.
pub fn tab_button(
    id: &'static str,
    label: impl Into<SharedString>,
    active: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let label = label.into();
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .h(px(30.))
        .px_3()
        .rounded_md()
        .bg(if active { accent() } else { panel() })
        .text_color(if active { rgb(0xffffff).into() } else { text() })
        .cursor_pointer()
        .hover(|style| style.bg(if active { accent() } else { input_bg() }))
        .on_click(on_click)
        .child(label)
}

/// Pieni kuvakepainike esimerkiksi yläpalkkiin.
pub fn icon_button(
    id: &'static str,
    icon: IconName,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .w(px(32.))
        .h(px(32.))
        .rounded_md()
        .cursor_pointer()
        .hover(|style| style.bg(input_bg()))
        .on_click(on_click)
        .child(Icon::new(icon).w(px(18.)).h(px(18.)).text_color(text()))
}

/// Yhteenvedon tunnuslukukortti.
pub fn stat_card(
    label: impl Into<SharedString>,
    value: impl Into<SharedString>,
    value_color: Hsla,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .w(px(150.))
        .p_3()
        .bg(panel())
        .border_1()
        .border_color(border())
        .rounded_md()
        .child(
            div()
                .text_size(px(11.))
                .text_color(muted())
                .child(label.into()),
        )
        .child(
            div()
                .text_size(px(18.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(value_color)
                .child(value.into()),
        )
}
