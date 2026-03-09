use alloc::string::String;

use crate::canvas::Canvas;
use crate::color::{INPUT_BG, INPUT_BORDER, INPUT_CURSOR, INPUT_FOCUS, TEXT};
use crate::event::{MouseButton, UiEvent};
use crate::geom::Rect;
use crate::text::{draw_text, CHAR_H, CHAR_W};

#[derive(Debug)]
pub struct TextFieldState {
    pub text: String,
    pub cursor: usize,
    pub focused: bool,
    pub hovered: bool,
}

impl TextFieldState {
    pub fn new(initial: &str) -> Self {
        Self {
            text: String::from(initial),
            cursor: initial.len(),
            focused: false,
            hovered: false,
        }
    }
}

fn inner_rect(rect: Rect) -> Rect {
    Rect::new(rect.x + 4, rect.y + 2, rect.w - 8, rect.h - 4)
}

fn visible_text<'a>(text: &'a str, cursor: usize, max_chars: usize) -> (&'a str, usize) {
    if text.len() <= max_chars {
        return (text, 0);
    }

    let mut start = 0usize;
    if cursor > max_chars {
        start = cursor - max_chars;
    }
    let end = (start + max_chars).min(text.len());
    (&text[start..end], start)
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

pub fn draw_text_field(canvas: &mut Canvas, rect: Rect, state: &TextFieldState) {
    canvas.fill_rect(rect, INPUT_BG);
    canvas.stroke_rect(
        rect,
        if state.focused { INPUT_FOCUS } else { INPUT_BORDER },
    );

    let inner = inner_rect(rect);
    let max_chars = (inner.w / CHAR_W).max(0) as usize;
    let (visible, start) = visible_text(&state.text, state.cursor, max_chars);

    let ty = inner.y + ((inner.h - CHAR_H) / 2).max(0);
    draw_text(canvas, inner.x, ty, TEXT, None, visible);

    if state.focused {
        let cursor_col = state.cursor.saturating_sub(start).min(visible.len()) as i32;
        let cx = inner.x + cursor_col * CHAR_W;
        canvas.fill_rect(Rect::new(cx, inner.y + 2, 1, inner.h.saturating_sub(4)), INPUT_CURSOR);
    }
}

pub fn handle_text_field_event(rect: Rect, state: &mut TextFieldState, ev: &UiEvent) -> bool {
    match *ev {
        UiEvent::MouseMove { pos } => {
            let new_hover = rect.contains(pos);
            let changed = new_hover != state.hovered;
            state.hovered = new_hover;
            changed
        }

        UiEvent::MouseDown {
            pos,
            button: MouseButton::Left,
        } => {
            let was_focused = state.focused;
            state.focused = rect.contains(pos);

            if state.focused {
                let inner = inner_rect(rect);
                let col = ((pos.x - inner.x).max(0) / CHAR_W) as usize;
                state.cursor = col.min(state.text.len());
            }

            state.focused != was_focused
        }

        UiEvent::KeyDown { code } if state.focused => {
            match code {
                14 => {
                    if state.cursor > 0 && state.cursor <= state.text.len() {
                        state.cursor -= 1;
                        state.text.remove(state.cursor);
                        return true;
                    }
                }
                105 => {
                    if state.cursor > 0 {
                        state.cursor -= 1;
                        return true;
                    }
                }
                106 => {
                    if state.cursor < state.text.len() {
                        state.cursor += 1;
                        return true;
                    }
                }
                102 => {
                    if state.cursor != 0 {
                        state.cursor = 0;
                        return true;
                    }
                }
                107 => {
                    if state.cursor != state.text.len() {
                        state.cursor = state.text.len();
                        return true;
                    }
                }
                _ => {
                    if let Some(ch) = keycode_to_char(code) {
                        if state.cursor <= state.text.len() {
                            state.text.insert(state.cursor, ch);
                            state.cursor += 1;
                            return true;
                        }
                    }
                }
            }
            false
        }

        _ => false,
    }
}
