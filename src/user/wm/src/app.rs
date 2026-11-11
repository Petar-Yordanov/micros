use alloc::string::String;
pub use libui::protocol::AppId;
pub enum AppLaunch {App(AppId),TextFile(String)}
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
