//! Compact, fixed-size GUI control protocol. Pixels NEVER travel over these channels.
//! Version 1, single application-owned surface per window.
use crate::event::{CursorKind, MouseButton, UiEvent};
use crate::geom::Point;

pub const WM_SERVICE: &str = "micros.wm";
pub const GUI_VERSION: u64 = 1;
pub const REGISTER: u64 = 1;
pub const PRESENT: u64 = 2;
pub const SURFACE: u64 = 3;
pub const OPEN_FILE: u64 = 4;
pub const CLOSE: u64 = 5;
pub const RESIZE: u64 = 6;
pub const CURSOR: u64 = 7;
pub const MOUSE_MOVE: u64 = 10;
pub const MOUSE_DOWN: u64 = 11;
pub const MOUSE_UP: u64 = 12;
pub const MOUSE_WHEEL: u64 = 13;
pub const KEY_DOWN: u64 = 14;
pub const KEY_UP: u64 = 15;
pub const FOCUS: u64 = 16;

#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppId { Hello=1, Demo=2, Explorer=3, TaskManager=4, Clock=5, Notepad=6, NetInfo=7, Browser=8 }
impl AppId {
    pub fn from_u32(v:u32)->Option<Self> {Some(match v {1=>Self::Hello,2=>Self::Demo,3=>Self::Explorer,4=>Self::TaskManager,5=>Self::Clock,6=>Self::Notepad,7=>Self::NetInfo,8=>Self::Browser,_=>return None})}
    pub fn filename(self) -> &'static str {match self {Self::Hello=>"/bin/hello_app.elf",Self::Demo=>"/bin/demo_app.elf",Self::Explorer=>"/bin/explorer_app.elf",Self::TaskManager=>"/bin/taskmgr_app.elf",Self::Clock=>"/bin/clock_app.elf",Self::Notepad=>"/bin/notepad_app.elf",Self::NetInfo=>"/bin/netinfo_app.elf",Self::Browser=>"/bin/browser_app.elf"}}
    pub fn title(self) -> &'static str {match self {Self::Hello=>"HELLO",Self::Demo=>"WIDGET DEMO",Self::Explorer=>"EXPLORER",Self::TaskManager=>"TASK MANAGER",Self::Clock=>"CLOCK",Self::Notepad=>"NOTEPAD",Self::NetInfo=>"NET INFO",Self::Browser=>"MINI BROWSER"}}
    pub fn frame_size(self)->(i32,i32) { match self {Self::Hello=>(360,220),Self::Demo=>(728,430),Self::Explorer=>(600,380),Self::TaskManager=>(620,394),Self::Clock=>(440,340),Self::Notepad=>(640,420),Self::NetInfo=>(460,340),Self::Browser=>(660,460)} }
}

// 32 bytes of u64 + 16 bytes of i32 + 256 bytes of data = 304, with NO padding.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct GuiMessage {
    pub kind: u64,
    pub pid: u64,
    pub channel: u64,
    pub surface: u64,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub data: [u8; 256],
}
impl GuiMessage {
    pub const SIZE: usize = 304;
    pub const fn new(kind: u64) -> Self {Self {kind, pid:0,channel:0,surface:0,x:0,y:0,w:0,h:0,data:[0;256]}}
    pub fn bytes(&self)->&[u8] { unsafe {core::slice::from_raw_parts(self as *const _ as *const u8, Self::SIZE)} }
    pub fn decode(b:&[u8])->Option<Self> {
        if b.len()!=Self::SIZE{return None}
        Some(unsafe{core::ptr::read_unaligned(b.as_ptr() as *const GuiMessage)})
    }
    pub fn event(ev:&UiEvent)->Self {
        let mut m=Self::new(0);
        match ev {
            UiEvent::MouseMove{pos}=>{m.kind=MOUSE_MOVE;m.x=pos.x;m.y=pos.y;},
            UiEvent::MouseDown{pos,button}=>{m.kind=MOUSE_DOWN;m.x=pos.x;m.y=pos.y;m.w=button_code(*button);},
            UiEvent::MouseUp{pos,button}=>{m.kind=MOUSE_UP;m.x=pos.x;m.y=pos.y;m.w=button_code(*button);},
            UiEvent::MouseWheel{pos,delta}=>{m.kind=MOUSE_WHEEL;m.x=pos.x;m.y=pos.y;m.w=*delta;},
            UiEvent::KeyDown{code}=>{m.kind=KEY_DOWN;m.x=*code as i32;},
            UiEvent::KeyUp{code}=>{m.kind=KEY_UP;m.x=*code as i32;},
        }
        m
    }
    pub fn to_event(&self)->Option<UiEvent> {
        let pos=Point::new(self.x,self.y);
        Some(match self.kind {
            MOUSE_MOVE=>UiEvent::MouseMove{pos},
            MOUSE_DOWN=>UiEvent::MouseDown{pos,button:decode_button(self.w)},
            MOUSE_UP=>UiEvent::MouseUp{pos,button:decode_button(self.w)},
            MOUSE_WHEEL=>UiEvent::MouseWheel{pos,delta:self.w},
            KEY_DOWN=>UiEvent::KeyDown{code:self.x as u16},
            KEY_UP=>UiEvent::KeyUp{code:self.x as u16},
            _=>return None,
        })
    }
    pub fn put_str(&mut self, s:&str) { let n=s.len().min(self.data.len()); self.data[..n].copy_from_slice(&s.as_bytes()[..n]); }
    pub fn as_str(&self)->Option<&str> {let end=self.data.iter().position(|x|*x==0).unwrap_or(self.data.len());core::str::from_utf8(&self.data[..end]).ok()}
}
fn button_code(b:MouseButton)->i32 {match b {MouseButton::Left=>1,MouseButton::Right=>2,MouseButton::Middle=>3,MouseButton::Other(x)=>x as i32+1024}}
fn decode_button(v:i32)->MouseButton {match v {1=>MouseButton::Left,2=>MouseButton::Right,3=>MouseButton::Middle,n if n>=1024=>MouseButton::Other((n-1024) as u16),_=>MouseButton::Left}}
pub fn cursor_code(c: CursorKind)->i32 {match c {CursorKind::Arrow=>0,CursorKind::Hand=>1,CursorKind::IBeam=>2,CursorKind::Circle=>3}}
pub fn decode_cursor(v:i32)->CursorKind {match v {1=>CursorKind::Hand,2=>CursorKind::IBeam,3=>CursorKind::Circle,_=>CursorKind::Arrow}}
