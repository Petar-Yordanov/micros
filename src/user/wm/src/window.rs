
use libui::geom::{Point, Rect};

use libui::event::{UiEvent,CursorKind};
use libui::protocol::{GuiMessage,self};
use rlibc::chan;

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
    pub pid: u64,
    pub event_chan: u64,
    pub surface_id: u64,
    pub surface_pixels: *const u32,
    pub surface_w: usize,
    pub surface_h: usize,
    pub client_cursor: CursorKind,
}

impl Window {
    pub fn new(id:u32, frame:Rect, pid:u64, channel:u64, surface_id:u64,
        pixels:*const u32, w:usize, h:usize, title:&'static str)->Self {
        Self {id,title,frame,restore_frame:None,minimized:false,maximized:false,
            pid,event_chan:channel,surface_id,surface_pixels:pixels,surface_w:w,surface_h:h,
            client_cursor:CursorKind::Arrow}
    }
    pub fn send(&self, m:&GuiMessage) {
        // Kernel queue is bounded; never block WM on an application.
        let _=chan::send(self.event_chan,m.bytes());
    }
    pub fn send_event(&self,event:&UiEvent)->bool {
        let m=GuiMessage::event(event);
        self.send(&m);
        false // Damage is driven by asynchronous PRESENT messages.
    }
    pub fn send_resize(&self) {
        let r=self.client_rect();
        let mut m=GuiMessage::new(protocol::RESIZE);
        m.w=r.w;m.h=r.h;
        self.send(&m);
    }
    pub fn send_close(&self) -> bool {
        // Do not silently lose a CLOSE when mouse input filled the channel.
        chan::send(self.event_chan, GuiMessage::new(protocol::CLOSE).bytes()).is_ok()
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
        Rect::new(
            close.x - WIN_BTN_GAP - WIN_BTN_W,
            close.y,
            WIN_BTN_W,
            WIN_BTN_H,
        )
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
