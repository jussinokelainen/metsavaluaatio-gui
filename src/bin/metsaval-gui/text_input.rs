//! Pieni tekstikenttä GPUI:lle.
//!
//! GPUI ei sisällä valmista tekstikenttää, joten tämä moduuli toteuttaa
//! sellaisen GPUI:n `Element`- ja `EntityInputHandler`-rajapinnoilla.
//! Kenttä tukee kirjoittamista, valintaa, leikepöytää ja salasanamaista
//! näyttöä (API-avain).

use std::ops::Range;

use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, Element, ElementId, ElementInputHandler,
    EntityInputHandler, FocusHandle, Focusable, GlobalElementId, Hsla, InspectorElementId,
    IntoElement, KeyBinding, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    PaintQuad, Pixels, Point, Render, ShapedLine, SharedString, Style, TextRun, TextStyle,
    UTF16Selection, Window, actions, div, fill, point, prelude::*, px, relative, size,
};

/// Kentän visuaalinen korkeus.
pub const INPUT_HEIGHT: Pixels = px(32.0);

const BACKSPACE: &str = "backspace";
const DELETE: &str = "delete";
const LEFT: &str = "left";
const RIGHT: &str = "right";
const SELECT_LEFT: &str = "shift-left";
const SELECT_RIGHT: &str = "shift-right";
const SELECT_ALL: &str = "ctrl-a";
const HOME: &str = "home";
const END: &str = "end";
const PASTE: &str = "ctrl-v";
const CUT: &str = "ctrl-x";
const COPY: &str = "ctrl-c";

actions!(
    text_input,
    [
        Backspace,
        Delete,
        Left,
        Right,
        SelectLeft,
        SelectRight,
        SelectAll,
        Home,
        End,
        Paste,
        Cut,
        Copy
    ]
);

/// Näppäimet, jotka tekstikentät tarvitsevat. Rekisteröidään kerran
/// sovelluksen käynnistyksessä.
pub fn key_bindings() -> Vec<KeyBinding> {
    let context = Some("TextInput");
    vec![
        KeyBinding::new(BACKSPACE, Backspace, context),
        KeyBinding::new(DELETE, Delete, context),
        KeyBinding::new(LEFT, Left, context),
        KeyBinding::new(RIGHT, Right, context),
        KeyBinding::new(SELECT_LEFT, SelectLeft, context),
        KeyBinding::new(SELECT_RIGHT, SelectRight, context),
        KeyBinding::new(SELECT_ALL, SelectAll, context),
        KeyBinding::new("cmd-a", SelectAll, context),
        KeyBinding::new(PASTE, Paste, context),
        KeyBinding::new("cmd-v", Paste, context),
        KeyBinding::new(CUT, Cut, context),
        KeyBinding::new("cmd-x", Cut, context),
        KeyBinding::new(COPY, Copy, context),
        KeyBinding::new("cmd-c", Copy, context),
        KeyBinding::new(HOME, Home, context),
        KeyBinding::new(END, End, context),
    ]
}

/// Yksi rivi tekstikentässä.
pub struct TextInput {
    focus_handle: FocusHandle,
    content: SharedString,
    placeholder: SharedString,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    last_layout: Option<ShapedLine>,
    last_bounds: Option<Bounds<Pixels>>,
    is_selecting: bool,
    /// Näytetäänkö sisältö peitettynä (esim. API-avain).
    masked: bool,
}

impl TextInput {
    pub fn new(cx: &mut Context<Self>, placeholder: impl Into<SharedString>, masked: bool) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            content: SharedString::default(),
            placeholder: placeholder.into(),
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            last_layout: None,
            last_bounds: None,
            is_selecting: false,
            masked,
        }
    }

    /// Kentän tämänhetkinen sisältö.
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Asettaa sisällön ja siirtää kursorin loppuun.
    pub fn set_content(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.content = text.into();
        let end = self.content.len();
        self.selected_range = end..end;
        self.selection_reversed = false;
        self.marked_range = None;
        cx.notify();
    }

    /// Kytkee salasanamaisen näytön päälle tai pois.
    pub fn set_masked(&mut self, masked: bool, cx: &mut Context<Self>) {
        if self.masked != masked {
            self.masked = masked;
            cx.notify();
        }
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.selected_range = offset..offset;
        cx.notify()
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        self.content[..offset]
            .chars()
            .next_back()
            .map(|ch| offset - ch.len_utf8())
            .unwrap_or(0)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.content[offset..]
            .chars()
            .next()
            .map(|ch| offset + ch.len_utf8())
            .unwrap_or(self.content.len())
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        if self.selection_reversed {
            self.selected_range.start = offset
        } else {
            self.selected_range.end = offset
        };
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        cx.notify()
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.previous_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.start, cx)
        }
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.next_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.end, cx)
        }
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor_offset()), cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
        self.select_to(self.content.len(), cx)
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.content.len(), cx);
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.previous_boundary(self.cursor_offset()), cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.next_boundary(self.cursor_offset()), cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.replace_text_in_range(None, &text.replace(['\n', '\r'], " "), window, cx);
        }
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
        }
    }

    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
            self.replace_text_in_range(None, "", window, cx)
        }
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.is_selecting = true;

        if event.modifiers.shift {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        } else {
            self.move_to(self.index_for_mouse_position(event.position), cx)
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _window: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_selecting {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        }
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        if self.content.is_empty() {
            return 0;
        }

        let (Some(bounds), Some(line)) = (self.last_bounds.as_ref(), self.last_layout.as_ref())
        else {
            return 0;
        };
        if position.y < bounds.top() {
            return 0;
        }
        if position.y > bounds.bottom() {
            return self.content.len();
        }

        let display_x = position.x - bounds.left();
        let display_index = line.closest_index_for_x(display_x);
        self.content_offset_from_display(display_index)
    }

    /// Sisältö sellaisena kuin se piirretään. Salasanatilassa jokainen merkki
    /// korvataan pisteellä.
    fn display_text(&self) -> SharedString {
        if self.masked {
            "•".repeat(self.content.chars().count()).into()
        } else {
            self.content.clone()
        }
    }

    /// Muuntaa sisällön tavuoffsetin näytettäväksi tavuoffsetiksi.
    fn display_offset(&self, offset: usize) -> usize {
        if self.masked {
            self.content[..offset].chars().count() * '•'.len_utf8()
        } else {
            offset
        }
    }

    /// Muuntaa näytettävän tavuoffsetin takaisin sisällön tavuoffsetiksi.
    fn content_offset_from_display(&self, offset: usize) -> usize {
        if self.masked {
            let char_index = offset / '•'.len_utf8();
            self.content
                .char_indices()
                .nth(char_index)
                .map(|(index, _)| index)
                .unwrap_or(self.content.len())
        } else {
            offset.min(self.content.len())
        }
    }

    fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf8_offset = 0;
        let mut utf16_count = 0;

        for ch in self.content.chars() {
            if utf16_count >= offset {
                break;
            }
            utf16_count += ch.len_utf16();
            utf8_offset += ch.len_utf8();
        }

        utf8_offset
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        let mut utf16_offset = 0;
        let mut utf8_count = 0;

        for ch in self.content.chars() {
            if utf8_count >= offset {
                break;
            }
            utf8_count += ch.len_utf8();
            utf16_offset += ch.len_utf16();
        }

        utf16_offset
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range_utf16.start)..self.offset_from_utf16(range_utf16.end)
    }
}

impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.content[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected_range),
            reversed: self.selection_reversed,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.marked_range = None;
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());

        self.content =
            (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..])
                .into();
        self.selected_range = range.start + new_text.len()..range.start + new_text.len();
        self.marked_range.take();
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());

        self.content =
            (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..])
                .into();
        if !new_text.is_empty() {
            self.marked_range = Some(range.start..range.start + new_text.len());
        } else {
            self.marked_range = None;
        }
        self.selected_range = new_selected_range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .map(|new_range| new_range.start + range.start..new_range.end + range.end)
            .unwrap_or_else(|| range.start + new_text.len()..range.start + new_text.len());

        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let last_layout = self.last_layout.as_ref()?;
        let range = self.range_from_utf16(&range_utf16);
        let start = self.display_offset(range.start);
        let end = self.display_offset(range.end);
        Some(Bounds::from_corners(
            point(bounds.left() + last_layout.x_for_index(start), bounds.top()),
            point(
                bounds.left() + last_layout.x_for_index(end),
                bounds.bottom(),
            ),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let line_point = self.last_bounds?.localize(&point)?;
        let last_layout = self.last_layout.as_ref()?;
        let display_index = last_layout.index_for_x(point.x - line_point.x)?;
        let content_index = self.content_offset_from_display(display_index);
        Some(self.offset_to_utf16(content_index))
    }
}

/// Itse piirrettävä tekstirivi. GPUI:n tekstielementti hoitaa vain asettelun,
/// joten teksti, kursori ja valinta piirretään tässä.
struct TextElement {
    input: gpui::Entity<TextInput>,
    text_size: Pixels,
    line_height: Pixels,
    text_color: Hsla,
    placeholder_color: Hsla,
    cursor_color: Hsla,
    selection_color: Hsla,
}

struct PrepaintState {
    line: Option<ShapedLine>,
    cursor: Option<PaintQuad>,
    selection: Option<PaintQuad>,
}

impl IntoElement for TextElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _window: &mut Window,
        _cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = self.line_height.into();
        // Asettelu hoidetaan yläpuolen taffy-puussa; tässä riittää, että
        // elementille varataan rivi.
        (_window.request_layout(style, [], _cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let input = self.input.read(cx);
        let content_empty = input.content.is_empty();
        let display_text = if content_empty {
            input.placeholder.clone()
        } else {
            input.display_text()
        };
        let text_color = if content_empty {
            self.placeholder_color
        } else {
            self.text_color
        };

        let style = TextStyle {
            color: text_color,
            font_size: self.text_size.into(),
            ..Default::default()
        };
        let font_size = style.font_size.to_pixels(window.rem_size());
        let run = TextRun {
            len: display_text.len(),
            font: style.font(),
            color: text_color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let line = window
            .text_system()
            .shape_line(display_text, font_size, &[run], None);

        let selected_range = &input.selected_range;
        let cursor = input.cursor_offset();
        let cursor_pos = line.x_for_index(input.display_offset(cursor));

        let (selection, cursor) = if selected_range.is_empty() {
            (
                None,
                Some(fill(
                    Bounds::new(
                        point(bounds.left() + cursor_pos, bounds.top()),
                        size(px(2.), bounds.bottom() - bounds.top()),
                    ),
                    self.cursor_color,
                )),
            )
        } else {
            (
                Some(fill(
                    Bounds::from_corners(
                        point(
                            bounds.left()
                                + line.x_for_index(input.display_offset(selected_range.start)),
                            bounds.top(),
                        ),
                        point(
                            bounds.left()
                                + line.x_for_index(input.display_offset(selected_range.end)),
                            bounds.bottom(),
                        ),
                    ),
                    self.selection_color,
                )),
                None,
            )
        };

        PrepaintState {
            line: Some(line),
            cursor,
            selection,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.input.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );

        if let Some(selection) = prepaint.selection.take() {
            window.paint_quad(selection)
        }

        let line = prepaint.line.take().expect("tekstirivi puuttuu");
        line.paint(bounds.origin, self.line_height, window, cx)
            .expect("tekstirivin piirto epäonnistui");

        if focus_handle.is_focused(window)
            && let Some(cursor) = prepaint.cursor.take()
        {
            window.paint_quad(cursor);
        }

        // Hiiren sijoittaminen ja IME tarvitsevat viimeisimmän asettelun.
        self.input.update(cx, |input, _cx| {
            input.last_layout = Some(line);
            input.last_bounds = Some(bounds);
        });
    }
}

impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focused = self.focus_handle.is_focused(window);

        div()
            .flex()
            .items_center()
            .key_context("TextInput")
            .track_focus(&self.focus_handle(cx))
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .w_full()
            .h(INPUT_HEIGHT)
            .px_2()
            .bg(crate::ui::input_bg())
            .border_1()
            .border_color(if focused {
                crate::ui::accent()
            } else {
                crate::ui::border()
            })
            .rounded_md()
            .child(TextElement {
                input: cx.entity(),
                text_size: px(14.),
                line_height: px(20.),
                text_color: crate::ui::text(),
                placeholder_color: crate::ui::muted(),
                cursor_color: crate::ui::text(),
                selection_color: crate::ui::selection(),
            })
    }
}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

#[cfg(all(test, feature = "gui-test-support"))]
mod gpui_tests {
    use super::*;
    use gpui::{TestAppContext, VisualTestContext};

    fn focused_input<'a>(
        cx: &'a mut TestAppContext,
        placeholder: &'static str,
        masked: bool,
    ) -> (gpui::Entity<TextInput>, &'a mut VisualTestContext) {
        cx.update(|cx| cx.bind_keys(key_bindings()));

        let (input, cx) = cx.add_window_view(|_, cx| TextInput::new(cx, placeholder, masked));
        cx.update(|window, cx| {
            let focus_handle = input.read(cx).focus_handle.clone();
            window.focus(&focus_handle);
            window.activate_window();
        });

        (input, cx)
    }

    #[gpui::test]
    fn typing_inserts_text(cx: &mut TestAppContext) {
        let (input, cx) = focused_input(cx, "kirjoita", false);

        cx.simulate_input("abc");

        input.update(cx, |input, _| assert_eq!(input.content(), "abc"));
    }

    #[gpui::test]
    fn backspace_removes_the_last_character(cx: &mut TestAppContext) {
        let (input, cx) = focused_input(cx, "kirjoita", false);

        cx.simulate_input("abc");
        cx.simulate_keystrokes("backspace");

        input.update(cx, |input, _| assert_eq!(input.content(), "ab"));
    }

    #[gpui::test]
    fn select_all_then_typing_replaces_the_content(cx: &mut TestAppContext) {
        let (input, cx) = focused_input(cx, "kirjoita", false);
        let select_all = if cfg!(target_os = "macos") {
            "cmd-a"
        } else {
            "ctrl-a"
        };

        cx.simulate_input("vanha");
        cx.simulate_keystrokes(select_all);
        cx.simulate_input("uusi");

        input.update(cx, |input, _| assert_eq!(input.content(), "uusi"));
    }

    #[gpui::test]
    fn left_arrow_moves_the_insertion_point(cx: &mut TestAppContext) {
        let (input, cx) = focused_input(cx, "kirjoita", false);

        cx.simulate_input("ac");
        cx.simulate_keystrokes("left");
        cx.simulate_input("b");

        input.update(cx, |input, _| assert_eq!(input.content(), "abc"));
    }

    #[gpui::test]
    fn clipboard_paste_inserts_text(cx: &mut TestAppContext) {
        let (input, cx) = focused_input(cx, "kirjoita", false);
        let paste = if cfg!(target_os = "macos") {
            "cmd-v"
        } else {
            "ctrl-v"
        };

        cx.write_to_clipboard(ClipboardItem::new_string("liitetty".to_string()));
        cx.simulate_keystrokes(paste);

        input.update(cx, |input, _| assert_eq!(input.content(), "liitetty"));
    }

    #[gpui::test]
    fn masked_input_keeps_the_real_content(cx: &mut TestAppContext) {
        let (input, cx) = focused_input(cx, "salainen", true);

        cx.simulate_input("abc");

        input.update(cx, |input, _| {
            assert_eq!(input.content(), "abc");
            assert_eq!(input.display_text().as_ref(), "•••");
        });
    }

    #[gpui::test]
    fn set_content_places_the_cursor_at_the_end(cx: &mut TestAppContext) {
        let (input, cx) = focused_input(cx, "kirjoita", false);

        input.update(cx, |input, cx| input.set_content("valmis", cx));
        cx.simulate_input("!");

        input.update(cx, |input, _| assert_eq!(input.content(), "valmis!"));
    }
}
