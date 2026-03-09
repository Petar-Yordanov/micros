use alloc::boxed::Box;

use libui::geom::{Point, Rect};

use crate::app::App;

pub const TITLEBAR_H: i32 = 26;
pub const FRAME_PAD: i32 = 2;

pub const WIN_BTN_W: i32 = 20;
pub const WIN_BTN_H: i32 = 18;
pub const WIN_BTN_GAP: i32 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowHit {
    None,
    Client,
    Titlebar,
    MinimizeButton,
    MaximizeButton,
    CloseButton,
}

pub struct Window {
    pub id: u32,
    pub title: &'static str,
    pub frame: Rect,
    pub restore_frame: Option<Rect>,
    pub minimized: bool,
    pub maximized: bool,
    pub app: Box<dyn App>,
}

impl Window {
    pub fn new(id: u32, frame: Rect, app: Box<dyn App>) -> Self {
        let title = app.title();
        Self {
            id,
            title,
            frame,
            restore_frame: None,
            minimized: false,
            maximized: false,
            app,
        }
    }

    pub fn client_rect(&self) -> Rect {
        Rect::new(
            self.frame.x + FRAME_PAD,
            self.frame.y + TITLEBAR_H,
            self.frame.w - (FRAME_PAD * 2),
            self.frame.h - TITLEBAR_H - FRAME_PAD,
        )
    }

    pub fn titlebar_rect(&self) -> Rect {
        Rect::new(
            self.frame.x + FRAME_PAD,
            self.frame.y + FRAME_PAD,
            self.frame.w - (FRAME_PAD * 2),
            TITLEBAR_H - FRAME_PAD,
        )
    }

    pub fn close_button_rect(&self) -> Rect {
        let tb = self.titlebar_rect();
        Rect::new(
            tb.right() - WIN_BTN_W - 2,
            tb.y + ((tb.h - WIN_BTN_H) / 2).max(0),
            WIN_BTN_W,
            WIN_BTN_H,
        )
    }

    pub fn maximize_button_rect(&self) -> Rect {
        let close = self.close_button_rect();
        Rect::new(close.x - WIN_BTN_GAP - WIN_BTN_W, close.y, WIN_BTN_W, WIN_BTN_H)
    }

    pub fn minimize_button_rect(&self) -> Rect {
        let max = self.maximize_button_rect();
        Rect::new(max.x - WIN_BTN_GAP - WIN_BTN_W, max.y, WIN_BTN_W, WIN_BTN_H)
    }

    pub fn contains_frame(&self, p: Point) -> bool {
        !self.minimized && self.frame.contains(p)
    }

    pub fn contains_client(&self, p: Point) -> bool {
        !self.minimized && self.client_rect().contains(p)
    }

    pub fn hit_test(&self, p: Point) -> WindowHit {
        if !self.contains_frame(p) {
            return WindowHit::None;
        }
        if self.close_button_rect().contains(p) {
            return WindowHit::CloseButton;
        }
        if self.maximize_button_rect().contains(p) {
            return WindowHit::MaximizeButton;
        }
        if self.minimize_button_rect().contains(p) {
            return WindowHit::MinimizeButton;
        }
        if self.titlebar_rect().contains(p) {
            return WindowHit::Titlebar;
        }
        if self.contains_client(p) {
            return WindowHit::Client;
        }
        WindowHit::None
    }
}
