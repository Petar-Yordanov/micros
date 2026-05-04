use crate::geom::Point;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Other(u16),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CursorKind {
    Arrow,
    Hand,
    IBeam,
    Circle,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UiEvent {
    MouseMove { pos: Point },
    MouseDown { pos: Point, button: MouseButton },
    MouseUp { pos: Point, button: MouseButton },
    MouseWheel { pos: Point, delta: i32 },
    KeyDown { code: u16 },
    KeyUp { code: u16 },
}
