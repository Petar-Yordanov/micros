use crate::app::{AppId,DesktopIcon,StartMenuAction,StartMenuItem,StartSubmenuItem};
const HELLO_ICON_ICO: &[u8] = include_bytes!("../../../../../assets/icons/hello.ico");
const DEMO_ICON_ICO: &[u8] = include_bytes!("../../../../../assets/icons/demo.ico");
const EXPLORER_ICON_ICO: &[u8] = include_bytes!("../../../../../assets/icons/explorer.ico");
const TASKMGR_ICON_ICO: &[u8] = include_bytes!("../../../../../assets/icons/taskmgr.ico");
const CLOCK_ICON_ICO: &[u8] = include_bytes!("../../../../../assets/icons/clock.ico");
const NOTEPAD_ICON_ICO: &[u8] = include_bytes!("../../../../../assets/icons/notepad.ico");
const BROWSER_ICON_ICO: &[u8] = include_bytes!("../../../../../assets/icons/browser.ico");
const NETINFO_ICON_ICO: &[u8] = include_bytes!("../../../../../assets/icons/netinfo.ico");

const APPLICATIONS_START_SUBMENU: &[StartSubmenuItem] = &[
    StartSubmenuItem {
        label: "Open Mini Browser",
        action: StartMenuAction::Launch(AppId::Browser),
    },
    StartSubmenuItem {
        label: "Open Notepad",
        action: StartMenuAction::Launch(AppId::Notepad),
    },
    StartSubmenuItem {
        label: "Open Clock App",
        action: StartMenuAction::Launch(AppId::Clock),
    },
    StartSubmenuItem {
        label: "Open Net Info",
        action: StartMenuAction::Launch(AppId::NetInfo),
    },
    StartSubmenuItem {
        label: "Open Task Manager",
        action: StartMenuAction::Launch(AppId::TaskManager),
    },
    StartSubmenuItem {
        label: "Open File Explorer",
        action: StartMenuAction::Launch(AppId::Explorer),
    },
    StartSubmenuItem {
        label: "Open Widget Demo",
        action: StartMenuAction::Launch(AppId::Demo),
    },
    StartSubmenuItem {
        label: "Open Hello",
        action: StartMenuAction::Launch(AppId::Hello),
    },
];

const START_MENU_ITEMS: &[StartMenuItem] = &[
    StartMenuItem {
        label: "Applications",
        action: None,
        submenu: APPLICATIONS_START_SUBMENU,
    },
    StartMenuItem {
        label: "Shut Down",
        action: Some(StartMenuAction::Shutdown),
        submenu: &[],
    },
];

const DESKTOP_ICONS: &[DesktopIcon] = &[
    DesktopIcon {
        app_id: AppId::Browser,
        label: "Browser",
        icon_ico_bytes: BROWSER_ICON_ICO,
        placeholder_text: "B",
    },
    DesktopIcon {
        app_id: AppId::Clock,
        label: "Clock",
        icon_ico_bytes: CLOCK_ICON_ICO,
        placeholder_text: "C",
    },
    DesktopIcon {
        app_id: AppId::Notepad,
        label: "Notepad",
        icon_ico_bytes: NOTEPAD_ICON_ICO,
        placeholder_text: "N",
    },
    DesktopIcon {
        app_id: AppId::NetInfo,
        label: "NetInfo",
        icon_ico_bytes: NETINFO_ICON_ICO,
        placeholder_text: "N",
    },
    DesktopIcon {
        app_id: AppId::TaskManager,
        label: "TaskMgr",
        icon_ico_bytes: TASKMGR_ICON_ICO,
        placeholder_text: "T",
    },
    DesktopIcon {
        app_id: AppId::Explorer,
        label: "Explorer",
        icon_ico_bytes: EXPLORER_ICON_ICO,
        placeholder_text: "E",
    },
    DesktopIcon {
        app_id: AppId::Demo,
        label: "Demo",
        icon_ico_bytes: DEMO_ICON_ICO,
        placeholder_text: "D",
    },
    DesktopIcon {
        app_id: AppId::Hello,
        label: "Hello",
        icon_ico_bytes: HELLO_ICON_ICO,
        placeholder_text: "H",
    },
];

pub fn start_menu_items()->&'static [StartMenuItem] {START_MENU_ITEMS}
pub fn desktop_icons()->&'static [DesktopIcon] {DESKTOP_ICONS}
