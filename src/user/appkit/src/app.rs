use alloc::string::String;
use libui::canvas::Canvas;
use libui::event::{CursorKind,UiEvent};
use libui::geom::{Point,Rect};
use libui::protocol::{GuiMessage,OPEN_FILE,WM_SERVICE};

pub enum AppLaunch { App(libui::protocol::AppId), TextFile(String) }
pub trait App {
    fn title(&self)->&'static str;
    fn handle_event(&mut self,ev:&UiEvent)->bool;
    fn render(&self,canvas:&mut Canvas,rect:Rect,focused:bool);
    fn cursor(&self,_local_pos:Point)->CursorKind{CursorKind::Arrow}
}
// Explorer's existing request_launch call is routed to the WM, not to a local static.
pub fn request_launch(req:AppLaunch) {
    if let AppLaunch::TextFile(path)=req {
        if let Ok(wm)=rlibc::chan::find(WM_SERVICE) {
            let mut msg=GuiMessage::new(OPEN_FILE);
            msg.pid=rlibc::proc::getpid().unwrap_or(0) as u64;
            msg.put_str(&path);
            let _=rlibc::chan::send(wm,msg.bytes());
        }
    }
}
