use micros_abi::types::{AbiInputEvent, ABI_IN_KIND_KEY, ABI_IN_KIND_REL, ABI_IN_KIND_SYN};
use rlibc::input::next_event;

use crate::keymap::{key_from_evdev, mouse_button_from_evdev};
pub use crate::keymap::{Key, MouseButton};

const EAGAIN: i64 = -11;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParsedInputEvent {
    Key { key: Key, pressed: bool },
    MouseButton { button: MouseButton, pressed: bool },
    MouseMove { dx: i32, dy: i32 },
    MouseWheel { delta: i32 },
}

pub struct InputParser {
    rel_dx: i32,
    rel_dy: i32,
    rel_wheel: i32,
    pending_button: Option<(MouseButton, bool)>,
}

impl InputParser {
    pub const fn new() -> Self {
        Self {
            rel_dx: 0,
            rel_dy: 0,
            rel_wheel: 0,
            pending_button: None,
        }
    }

    pub fn ingest(&mut self, raw: AbiInputEvent) -> Option<ParsedInputEvent> {
        match raw.kind {
            ABI_IN_KIND_KEY => {
                if (0x110..=0x11f).contains(&raw.code) {
                    self.pending_button = Some((mouse_button_from_evdev(raw.code), raw.value != 0));
                    None
                } else {
                    Some(ParsedInputEvent::Key {
                        key: key_from_evdev(raw.code),
                        pressed: raw.value != 0,
                    })
                }
            }

            ABI_IN_KIND_REL => {
                match raw.code {
                    0x00 => self.rel_dx = self.rel_dx.saturating_add(raw.value),
                    0x01 => self.rel_dy = self.rel_dy.saturating_add(raw.value),
                    0x08 => self.rel_wheel = self.rel_wheel.saturating_add(raw.value),
                    _ => {}
                }
                None
            }

            ABI_IN_KIND_SYN => {
                if let Some((button, pressed)) = self.pending_button.take() {
                    return Some(ParsedInputEvent::MouseButton { button, pressed });
                }

                if self.rel_dx != 0 || self.rel_dy != 0 {
                    let dx = self.rel_dx;
                    let dy = self.rel_dy;
                    self.rel_dx = 0;
                    self.rel_dy = 0;
                    return Some(ParsedInputEvent::MouseMove { dx, dy });
                }

                if self.rel_wheel != 0 {
                    let delta = self.rel_wheel;
                    self.rel_wheel = 0;
                    return Some(ParsedInputEvent::MouseWheel { delta });
                }

                None
            }

            _ => None,
        }
    }
}

pub fn next_parsed_event(parser: &mut InputParser) -> Option<ParsedInputEvent> {
    let mut raw = AbiInputEvent::default();

    loop {
        let r = next_event(&mut raw);
        if r == EAGAIN || r < 0 {
            return None;
        }

        if let Some(ev) = parser.ingest(raw) {
            return Some(ev);
        }
    }
}
