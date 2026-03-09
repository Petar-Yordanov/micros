use core::fmt::Write;

use libui::canvas::Canvas;
use libui::color::TEXT;
use libui::event::{MouseButton, UiEvent};
use libui::geom::Rect;
use libui::text::draw_text;
use libui::widgets::button::draw_button;

use crate::app::App;

pub struct HelloApp {
    count: u32,
    hovered: bool,
    pressed: bool,
}

impl HelloApp {
    pub fn new() -> Self {
        Self {
            count: 0,
            hovered: false,
            pressed: false,
        }
    }

    fn button_rect_local() -> Rect {
        Rect::new(16, 48, 120, 32)
    }
}

impl App for HelloApp {
    fn title(&self) -> &'static str {
        "HELLO"
    }

    //fn cursor(&self, local_pos: Point) -> CursorKind {
    //    if Self::button_rect_local().contains(local_pos) {
    //        CursorKind::Hand
    //    } else {
    //        CursorKind::Arrow
    //    }
    //}

    fn handle_event(&mut self, ev: &UiEvent) -> bool {
        let btn = Self::button_rect_local();

        match *ev {
            UiEvent::MouseMove { pos } => {
                let new_hover = btn.contains(pos);
                if new_hover != self.hovered {
                    self.hovered = new_hover;
                    return true;
                }
                false
            }

            UiEvent::MouseDown {
                pos,
                button: MouseButton::Left,
            } => {
                if btn.contains(pos) && !self.pressed {
                    self.pressed = true;
                    return true;
                }
                false
            }

            UiEvent::MouseUp {
                pos,
                button: MouseButton::Left,
            } => {
                let was_pressed = self.pressed;
                let hovered = btn.contains(pos);
                self.pressed = false;

                if was_pressed && hovered {
                    self.count = self.count.wrapping_add(1);
                    self.hovered = hovered;
                    return true;
                }

                was_pressed
            }

            _ => false,
        }
    }

    fn render(&self, canvas: &mut Canvas, client_rect: Rect, _focused: bool) {
        draw_text(
            canvas,
            client_rect.x + 16,
            client_rect.y + 16,
            TEXT,
            None,
            "Embedded app",
        );

        let mut buf = TextBuf::new();
        let _ = write!(&mut buf, "Count={}", self.count);

        draw_text(
            canvas,
            client_rect.x + 16,
            client_rect.y + 32,
            TEXT,
            None,
            buf.as_str(),
        );

        let btn = Self::button_rect_local().translate(client_rect.x, client_rect.y);
        draw_button(canvas, btn, "Click me", self.hovered, self.pressed);
    }
}

struct TextBuf {
    buf: [u8; 64],
    len: usize,
}

impl TextBuf {
    const fn new() -> Self {
        Self {
            buf: [0; 64],
            len: 0,
        }
    }

    fn as_str(&self) -> &str {
        unsafe { core::str::from_utf8_unchecked(&self.buf[..self.len]) }
    }
}

impl Write for TextBuf {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let bytes = s.as_bytes();
        let n = core::cmp::min(bytes.len(), self.buf.len().saturating_sub(self.len));
        self.buf[self.len..self.len + n].copy_from_slice(&bytes[..n]);
        self.len += n;
        Ok(())
    }
}
