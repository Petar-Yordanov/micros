use crate::kernel::drivers::virtio::input::InputMsg;
use crate::kernel::input::events::{InputEvent, Key, MouseButton};

pub struct Parser {
    rel_dx: i32,
    rel_dy: i32,
    rel_wheel: i32,
    pending_button: Option<(MouseButton, bool)>,
}

impl Parser {
    pub const fn new() -> Self {
        Self {
            rel_dx: 0,
            rel_dy: 0,
            rel_wheel: 0,
            pending_button: None,
        }
    }

    pub fn ingest(&mut self, msg: InputMsg) -> Option<InputEvent> {
        match msg {
            InputMsg::Key { code, pressed, .. } => {
                if (0x110..=0x11f).contains(&code) {
                    let button = match code {
                        0x110 => MouseButton::Left,
                        0x111 => MouseButton::Right,
                        0x112 => MouseButton::Middle,
                        0x113 => MouseButton::Side,
                        0x114 => MouseButton::Extra,
                        _ => MouseButton::Unknown(code),
                    };
                    self.pending_button = Some((button, pressed));
                    None
                } else {
                    Some(InputEvent::Key {
                        key: key_from_evdev(code),
                        pressed,
                    })
                }
            }

            InputMsg::Rel { code, value } => {
                match code {
                    0x00 => self.rel_dx = self.rel_dx.saturating_add(value),
                    0x01 => self.rel_dy = self.rel_dy.saturating_add(value),
                    0x08 => self.rel_wheel = self.rel_wheel.saturating_add(value),
                    _ => {}
                }
                None
            }

            InputMsg::Syn => {
                if let Some((b, p)) = self.pending_button.take() {
                    return Some(InputEvent::MouseButton {
                        button: b,
                        pressed: p,
                    });
                }

                if self.rel_dx != 0 || self.rel_dy != 0 {
                    let dx = self.rel_dx;
                    let dy = self.rel_dy;
                    self.rel_dx = 0;
                    self.rel_dy = 0;
                    return Some(InputEvent::MouseMove { dx, dy });
                }

                if self.rel_wheel != 0 {
                    let delta = self.rel_wheel;
                    self.rel_wheel = 0;
                    return Some(InputEvent::MouseWheel { delta });
                }

                None
            }

            InputMsg::Other { .. } => None,
        }
    }
}

fn key_from_evdev(code: u16) -> Key {
    match code {
        30 => Key::A,
        48 => Key::B,
        46 => Key::C,
        32 => Key::D,
        18 => Key::E,
        33 => Key::F,
        34 => Key::G,
        35 => Key::H,
        23 => Key::I,
        36 => Key::J,
        37 => Key::K,
        38 => Key::L,
        50 => Key::M,
        49 => Key::N,
        24 => Key::O,
        25 => Key::P,
        16 => Key::Q,
        19 => Key::R,
        31 => Key::S,
        20 => Key::T,
        22 => Key::U,
        47 => Key::V,
        17 => Key::W,
        45 => Key::X,
        21 => Key::Y,
        44 => Key::Z,

        28 => Key::Enter,
        1 => Key::Esc,
        14 => Key::Backspace,
        15 => Key::Tab,
        57 => Key::Space,

        42 => Key::LeftShift,
        54 => Key::RightShift,
        29 => Key::LeftCtrl,
        97 => Key::RightCtrl,
        56 => Key::LeftAlt,
        100 => Key::RightAlt,

        103 => Key::Up,
        108 => Key::Down,
        105 => Key::Left,
        106 => Key::Right,

        59 => Key::F1,
        60 => Key::F2,
        61 => Key::F3,
        62 => Key::F4,
        63 => Key::F5,
        64 => Key::F6,
        65 => Key::F7,
        66 => Key::F8,
        67 => Key::F9,
        68 => Key::F10,
        87 => Key::F11,
        88 => Key::F12,

        2 => Key::Num1,
        3 => Key::Num2,
        4 => Key::Num3,
        5 => Key::Num4,
        6 => Key::Num5,
        7 => Key::Num6,
        8 => Key::Num7,
        9 => Key::Num8,
        10 => Key::Num9,
        11 => Key::Num0,
        12 => Key::Minus,
        13 => Key::Equal,

        26 => Key::LeftBracket,
        27 => Key::RightBracket,
        39 => Key::Semicolon,
        40 => Key::Apostrophe,
        41 => Key::Grave,
        43 => Key::Backslash,
        86 => Key::NonUsBackslash,
        51 => Key::Comma,
        52 => Key::Dot,
        53 => Key::Slash,

        58 => Key::CapsLock,
        110 => Key::Insert,
        111 => Key::Delete,
        102 => Key::Home,
        107 => Key::End,
        104 => Key::PageUp,
        109 => Key::PageDown,

        125 => Key::LeftMeta,
        126 => Key::RightMeta,
        113 => Key::Mute,
        114 => Key::VolumeDown,
        115 => Key::VolumeUp,
        164 => Key::PlayPause,

        _ => Key::Unknown(code),
    }
}
