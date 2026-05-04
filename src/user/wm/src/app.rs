use alloc::string::String;

use libui::canvas::Canvas;
use libui::event::{CursorKind, UiEvent};
use libui::geom::{Point, Rect};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppId {
    Hello,
    Demo,
    Explorer,
    TaskManager,
    Clock,
    Notepad,
    NetInfo,
    Browser,
}

pub enum AppLaunch {
    App(AppId),
    TextFile(String),
}

static mut PENDING_LAUNCH: Option<AppLaunch> = None;

pub fn request_launch(req: AppLaunch) {
    unsafe {
        core::ptr::write(core::ptr::addr_of_mut!(PENDING_LAUNCH), Some(req));
    }
}

pub fn take_launch_request() -> Option<AppLaunch> {
    unsafe {
        let slot = core::ptr::addr_of_mut!(PENDING_LAUNCH);
        let value = core::ptr::read(slot);
        core::ptr::write(slot, None);
        value
    }
}

pub trait App {
    fn title(&self) -> &'static str;
    fn handle_event(&mut self, ev: &UiEvent) -> bool;
    fn render(&self, canvas: &mut Canvas, client_rect: Rect, focused: bool);

    fn cursor(&self, _local_pos: Point) -> CursorKind {
        CursorKind::Arrow
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartMenuAction {
    Launch(AppId),
    Shutdown,
}

#[derive(Clone, Copy)]
pub struct StartSubmenuItem {
    pub label: &'static str,
    pub action: StartMenuAction,
}

#[derive(Clone, Copy)]
pub struct StartMenuItem {
    pub label: &'static str,
    pub action: Option<StartMenuAction>,
    pub submenu: &'static [StartSubmenuItem],
}

#[derive(Clone, Copy)]
pub struct DesktopIcon {
    pub app_id: AppId,
    pub label: &'static str,
    pub icon_ico_bytes: &'static [u8],
    pub placeholder_text: &'static str,
}
