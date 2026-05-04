extern crate alloc;

use alloc::string::String;
use core::fmt::Write;

use libui::canvas::Canvas;
use libui::color::{PANEL_TEXT, TEXT, TEXT_DIM};
use libui::event::{CursorKind, MouseButton, UiEvent};
use libui::geom::{Point, Rect};
use libui::text::{draw_text, CHAR_W};
use libui::widgets::button::draw_button;
use libui::widgets::panel::{draw_panel, inner_rect};

use crate::app::App;
use crate::apps::browser_renderer::{paint_document, render_http_response, RenderedDocument};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BrowserButton {
    None,
    Go,
    Home,
}

pub struct MiniBrowserApp {
    url: String,
    status: String,
    document: RenderedDocument,
    url_focused: bool,
    hover_button: BrowserButton,
    pressed_button: BrowserButton,
    scroll_lines: usize,
}

impl MiniBrowserApp {
    pub fn new() -> Self {
        Self {
            url: String::from("httpforever.com/"),
            status: String::from("Ready"),
            document: RenderedDocument::from_text(
                "Type a plain HTTP URL and click Go.\n\nTry:\nhttpforever.com/\nwww.neverssl.com/\nexample.com/\n10.0.2.2/",
                Self::content_rect_local().w,
            ),
            url_focused: false,
            hover_button: BrowserButton::None,
            pressed_button: BrowserButton::None,
            scroll_lines: 0,
        }
    }

    fn toolbar_rect_local() -> Rect {
        Rect::new(8, 8, 620, 32)
    }

    fn url_rect_local() -> Rect {
        Rect::new(16, 15, 462, 18)
    }

    fn go_button_rect_local() -> Rect {
        Rect::new(486, 14, 54, 20)
    }

    fn home_button_rect_local() -> Rect {
        Rect::new(548, 14, 64, 20)
    }

    fn content_rect_local() -> Rect {
        Rect::new(8, 52, 620, 310)
    }

    fn status_rect_local() -> Rect {
        Rect::new(8, 370, 620, 28)
    }

    fn button_at(pos: Point) -> BrowserButton {
        if Self::go_button_rect_local().contains(pos) {
            BrowserButton::Go
        } else if Self::home_button_rect_local().contains(pos) {
            BrowserButton::Home
        } else {
            BrowserButton::None
        }
    }

    fn go_home(&mut self) {
        self.url.clear();
        self.url.push_str("httpforever.com/");
        self.go();
    }

    fn go(&mut self) {
        self.status.clear();
        self.scroll_lines = 0;

        let mut target = String::new();
        target.push_str(self.url.trim());

        if target.is_empty() {
            self.status.push_str("Enter a URL first");
            self.document = RenderedDocument::from_text("(no URL)", Self::content_rect_local().w);
            return;
        }

        let _ = write!(&mut self.status, "Fetching {} ...", target);

        match rlibc::http::get_url(&target) {
            Ok(bytes) => {
                self.status.clear();

                let _ = write!(
                    &mut self.status,
                    "Loaded {} byte HTTP response from {}",
                    bytes.len(),
                    target
                );

                self.document = render_http_response(&bytes, Self::content_rect_local().w);

                if self.document.content_height <= 0 {
                    self.document = RenderedDocument::empty(Self::content_rect_local().w);
                }
            }
            Err(e) => {
                self.status.clear();

                let _ = write!(&mut self.status, "Fetch failed with errno {}", e.0);

                let mut msg = String::new();
                let _ = write!(
                    &mut msg,
                    "Could not load:\n{}\n\nerrno={}\n\nOnly plain http:// works.\nhttps:// is not supported.",
                    target,
                    e.0
                );

                self.document = RenderedDocument::from_text(&msg, Self::content_rect_local().w);
            }
        }
    }

    fn draw_input(canvas: &mut Canvas, rect: Rect, focused: bool, text: &str) {
        draw_panel(canvas, rect);

        let inner = inner_rect(rect, 4);
        let max_chars = (inner.w / CHAR_W).max(0) as usize;

        let start = text.len().saturating_sub(max_chars);
        let visible = &text[start..];

        draw_text(canvas, inner.x, inner.y + 1, PANEL_TEXT, None, visible);

        if focused {
            let cursor_x = inner.x + (visible.chars().count() as i32 * CHAR_W);
            canvas.vline(cursor_x, inner.y, 10, TEXT);
        }
    }

    fn key_to_char(code: u16) -> Option<char> {
        match code {
            2 => Some('1'),
            3 => Some('2'),
            4 => Some('3'),
            5 => Some('4'),
            6 => Some('5'),
            7 => Some('6'),
            8 => Some('7'),
            9 => Some('8'),
            10 => Some('9'),
            11 => Some('0'),
            12 => Some('-'),
            13 => Some('='),

            16 => Some('q'),
            17 => Some('w'),
            18 => Some('e'),
            19 => Some('r'),
            20 => Some('t'),
            21 => Some('y'),
            22 => Some('u'),
            23 => Some('i'),
            24 => Some('o'),
            25 => Some('p'),

            30 => Some('a'),
            31 => Some('s'),
            32 => Some('d'),
            33 => Some('f'),
            34 => Some('g'),
            35 => Some('h'),
            36 => Some('j'),
            37 => Some('k'),
            38 => Some('l'),

            44 => Some('z'),
            45 => Some('x'),
            46 => Some('c'),
            47 => Some('v'),
            48 => Some('b'),
            49 => Some('n'),
            50 => Some('m'),

            51 => Some(','),
            52 => Some('.'),
            53 => Some('/'),
            57 => Some(' '),

            _ => None,
        }
    }
}

impl App for MiniBrowserApp {
    fn title(&self) -> &'static str {
        "MINI BROWSER"
    }

    fn cursor(&self, local_pos: Point) -> CursorKind {
        if Self::url_rect_local().contains(local_pos) {
            CursorKind::IBeam
        } else if Self::button_at(local_pos) != BrowserButton::None {
            CursorKind::Hand
        } else {
            CursorKind::Arrow
        }
    }

    fn handle_event(&mut self, ev: &UiEvent) -> bool {
        let mut changed = false;

        match *ev {
            UiEvent::MouseMove { pos } => {
                let hover = Self::button_at(pos);
                if hover != self.hover_button {
                    self.hover_button = hover;
                    changed = true;
                }
            }

            UiEvent::MouseDown {
                pos,
                button: MouseButton::Left,
            } => {
                let was_focused = self.url_focused;
                self.url_focused = Self::url_rect_local().contains(pos);
                changed |= self.url_focused != was_focused;

                let btn = Self::button_at(pos);
                if btn != BrowserButton::None {
                    self.pressed_button = btn;
                    changed = true;
                }
            }

            UiEvent::MouseUp {
                pos,
                button: MouseButton::Left,
            } => {
                let released_over = Self::button_at(pos);
                let pressed = self.pressed_button;

                if self.pressed_button != BrowserButton::None {
                    self.pressed_button = BrowserButton::None;
                    changed = true;
                }

                if pressed != BrowserButton::None && pressed == released_over {
                    match pressed {
                        BrowserButton::Go => self.go(),
                        BrowserButton::Home => self.go_home(),
                        BrowserButton::None => {}
                    }

                    changed = true;
                }
            }

            UiEvent::MouseWheel { delta, .. } => {
                if delta < 0 {
                    self.scroll_lines = self.scroll_lines.saturating_add(3);
                } else if delta > 0 {
                    self.scroll_lines = self.scroll_lines.saturating_sub(3);
                }

                changed = true;
            }

            UiEvent::KeyDown { code } => {
                if self.url_focused {
                    match code {
                        14 => {
                            self.url.pop();
                            changed = true;
                        }
                        28 => {
                            self.go();
                            changed = true;
                        }
                        _ => {
                            if let Some(ch) = Self::key_to_char(code) {
                                if self.url.len() < 128 {
                                    self.url.push(ch);
                                    changed = true;
                                }
                            }
                        }
                    }
                } else if code == 63 {
                    self.go();
                    changed = true;
                }
            }

            UiEvent::KeyUp { .. } => {}
            UiEvent::MouseDown { .. } => {}
            UiEvent::MouseUp { .. } => {}
        }

        changed
    }

    fn render(&self, canvas: &mut Canvas, client_rect: Rect, _focused: bool) {
        let toolbar = Self::toolbar_rect_local().translate(client_rect.x, client_rect.y);
        let url_rect = Self::url_rect_local().translate(client_rect.x, client_rect.y);
        let go_btn = Self::go_button_rect_local().translate(client_rect.x, client_rect.y);
        let home_btn = Self::home_button_rect_local().translate(client_rect.x, client_rect.y);
        let content = Self::content_rect_local().translate(client_rect.x, client_rect.y);
        let status = Self::status_rect_local().translate(client_rect.x, client_rect.y);

        draw_panel(canvas, toolbar);

        Self::draw_input(canvas, url_rect, self.url_focused, &self.url);

        draw_button(
            canvas,
            go_btn,
            "Go",
            self.hover_button == BrowserButton::Go,
            self.pressed_button == BrowserButton::Go,
        );

        draw_button(
            canvas,
            home_btn,
            "Home",
            self.hover_button == BrowserButton::Home,
            self.pressed_button == BrowserButton::Home,
        );

        draw_panel(canvas, content);
        draw_text(
            canvas,
            content.x + 8,
            content.y - 12,
            TEXT_DIM,
            None,
            "Page preview",
        );

        let scroll_y = (self.scroll_lines as i32).saturating_mul(14);
        paint_document(canvas, content, &self.document, scroll_y);

        draw_panel(canvas, status);
        let status_inner = inner_rect(status, 6);
        draw_text(
            canvas,
            status_inner.x,
            status_inner.y + 5,
            TEXT,
            None,
            &self.status,
        );
    }
}
