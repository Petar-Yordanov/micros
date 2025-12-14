#![allow(dead_code)]

use spin::Mutex;

use crate::kernel::drivers::virtio::input as vin;
use crate::kernel::input::parser::Parser;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Key {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    Enter,
    Esc,
    Backspace,
    Tab,
    Space,
    LeftShift,
    RightShift,
    LeftCtrl,
    RightCtrl,
    LeftAlt,
    RightAlt,
    Up,
    Down,
    Left,
    Right,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    Num1,
    Num2,
    Num3,
    Num4,
    Num5,
    Num6,
    Num7,
    Num8,
    Num9,
    Num0,
    Minus,
    Equal,

    LeftBracket,
    RightBracket,
    Semicolon,
    Apostrophe,
    Grave,
    Backslash,
    NonUsBackslash,
    Comma,
    Dot,
    Slash,

    CapsLock,

    Insert,
    Delete,
    Home,
    End,
    PageUp,
    PageDown,

    LeftMeta,
    RightMeta,

    Mute,
    VolumeDown,
    VolumeUp,
    PlayPause,
    Unknown(u16),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Side,
    Extra,
    Unknown(u16),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum InputEvent {
    Key { key: Key, pressed: bool },
    MouseButton { button: MouseButton, pressed: bool },
    MouseMove { dx: i32, dy: i32 },
    MouseWheel { delta: i32 },
}

static PARSER: Mutex<Parser> = Mutex::new(Parser::new());

pub fn poll_raw() -> Option<vin::InputMsg> {
    vin::poll_msg()
}

pub fn poll() -> Option<InputEvent> {
    let msg = poll_raw()?;
    PARSER.lock().ingest(msg)
}

pub fn next() -> Option<InputEvent> {
    loop {
        let msg = poll_raw()?;
        if let Some(e) = PARSER.lock().ingest(msg) {
            return Some(e);
        }
    }
}
