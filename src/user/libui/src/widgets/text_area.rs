use alloc::string::String;

use crate::canvas::Canvas;
use crate::color::{INPUT_BG, INPUT_BORDER, INPUT_CURSOR, INPUT_FOCUS, TEXT};
use crate::event::{MouseButton, UiEvent};
use crate::geom::Rect;
use crate::text::{draw_text, CHAR_H};
use crate::widgets::scroll::{
    draw_v_scrollbar, handle_v_scrollbar_event, ScrollbarState, SCROLLBAR_W,
};

#[derive(Debug)]
pub struct TextAreaState {
    pub text: String,
    pub focused: bool,
    pub scroll: ScrollbarState,
}

impl TextAreaState {
    pub fn new(initial: &str) -> Self {
        Self {
            text: String::from(initial),
            focused: false,
            scroll: ScrollbarState::new(),
        }
    }

    pub fn clear(&mut self) {
        self.text.clear();
        self.scroll.offset = 0;
    }

    pub fn append_line(&mut self, line: &str) {
        if !self.text.is_empty() && !self.text.ends_with('\n') {
            self.text.push('\n');
        }
        self.text.push_str(line);
    }
}

fn content_rect(rect: Rect) -> Rect {
    Rect::new(rect.x + 4, rect.y + 4, rect.w - SCROLLBAR_W - 8, rect.h - 8)
}

fn count_lines(text: &str) -> i32 {
    let mut n = 1i32;
    for b in text.as_bytes() {
        if *b == b'\n' {
            n += 1;
        }
    }
    n
}

fn keycode_to_char(code: u16) -> Option<char> {
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
        26 => Some('['),
        27 => Some(']'),
        30 => Some('a'),
        31 => Some('s'),
        32 => Some('d'),
        33 => Some('f'),
        34 => Some('g'),
        35 => Some('h'),
        36 => Some('j'),
        37 => Some('k'),
        38 => Some('l'),
        39 => Some(';'),
        40 => Some('\''),
        41 => Some('`'),
        43 => Some('\\'),
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

pub fn draw_text_area(canvas: &mut Canvas, rect: Rect, state: &TextAreaState) {
    canvas.fill_rect(rect, INPUT_BG);
    canvas.stroke_rect(
        rect,
        if state.focused { INPUT_FOCUS } else { INPUT_BORDER },
    );

    let content = content_rect(rect);
    let line_h = CHAR_H + 2;
    let viewport_h = content.h;
    let content_h = count_lines(&state.text).saturating_mul(line_h);

    let mut y = content.y - state.scroll.offset;
    for line in state.text.lines() {
        if y + line_h >= content.y && y < content.bottom() {
            draw_text(canvas, content.x, y, TEXT, None, line);
        }
        y += line_h;
    }

    if state.focused {
        let caret_y = (content.y + content_h - state.scroll.offset - 2).min(content.bottom() - 2);
        if caret_y >= content.y {
            canvas.fill_rect(
                Rect::new(content.x + 1, caret_y, 1, CHAR_H),
                INPUT_CURSOR,
            );
        }
    }

    draw_v_scrollbar(canvas, rect, &state.scroll, viewport_h, content_h);
}

pub fn handle_text_area_event(rect: Rect, state: &mut TextAreaState, ev: &UiEvent) -> bool {
    let content = content_rect(rect);
    let line_h = CHAR_H + 2;
    let viewport_h = content.h;
    let content_h = count_lines(&state.text).saturating_mul(line_h);

    let mut changed =
        handle_v_scrollbar_event(rect, &mut state.scroll, viewport_h, content_h, ev);

    match *ev {
        UiEvent::MouseDown {
            pos,
            button: MouseButton::Left,
        } => {
            let old = state.focused;
            state.focused = rect.contains(pos);
            changed |= old != state.focused;
        }

        UiEvent::KeyDown { code } if state.focused => {
            match code {
                14 => {
                    if !state.text.is_empty() {
                        state.text.pop();
                        changed = true;
                    }
                }
                28 => {
                    state.text.push('\n');
                    changed = true;
                }
                103 => {
                    state.scroll.scroll_lines(viewport_h, content_h, -1, line_h);
                    changed = true;
                }
                108 => {
                    state.scroll.scroll_lines(viewport_h, content_h, 1, line_h);
                    changed = true;
                }
                _ => {
                    if let Some(ch) = keycode_to_char(code) {
                        state.text.push(ch);
                        changed = true;
                    }
                }
            }
        }

        _ => {}
    }

    changed
}
