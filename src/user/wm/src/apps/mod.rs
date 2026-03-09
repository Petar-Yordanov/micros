use alloc::boxed::Box;

use crate::app::{
    App, AppId, AppLaunch, DesktopIcon, StartMenuAction, StartMenuItem, StartSubmenuItem,
};

pub mod clock;
pub mod demo;
pub mod explorer;
pub mod hello;
pub mod notepad;
pub mod taskmgr;

const HELLO_ICON_ICO: &[u8] = include_bytes!("../../../../../assets/icons/hello.ico");
const DEMO_ICON_ICO: &[u8] = include_bytes!("../../../../../assets/icons/demo.ico");
const EXPLORER_ICON_ICO: &[u8] = include_bytes!("../../../../../assets/icons/explorer.ico");
const TASKMGR_ICON_ICO: &[u8] = include_bytes!("../../../../../assets/icons/taskmgr.ico");
const CLOCK_ICON_ICO: &[u8] = include_bytes!("../../../../../assets/icons/clock.ico");
const NOTEPAD_ICON_ICO: &[u8] = include_bytes!("../../../../../assets/icons/notepad.ico");

const APPLICATIONS_START_SUBMENU: &[StartSubmenuItem] = &[
    StartSubmenuItem {
        label: "Open Notepad",
        action: StartMenuAction::Launch(AppId::Notepad),
    },
    StartSubmenuItem {
        label: "Open Clock App",
        action: StartMenuAction::Launch(AppId::Clock),
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

#[allow(dead_code)]
pub fn make_app(id: AppId) -> Box<dyn App> {
    make_launch(AppLaunch::App(id))
}

pub fn make_launch(req: AppLaunch) -> Box<dyn App> {
    match req {
        AppLaunch::App(AppId::Hello) => Box::new(hello::HelloApp::new()),
        AppLaunch::App(AppId::Demo) => Box::new(demo::DemoApp::new()),
        AppLaunch::App(AppId::Explorer) => Box::new(explorer::ExplorerApp::new()),
        AppLaunch::App(AppId::TaskManager) => Box::new(taskmgr::TaskManagerApp::new()),
        AppLaunch::App(AppId::Clock) => Box::new(clock::ClockApp::new()),
        AppLaunch::App(AppId::Notepad) => Box::new(notepad::NotepadApp::new()),
        AppLaunch::TextFile(path) => Box::new(notepad::NotepadApp::open(&path)),
    }
}

pub fn start_menu_items() -> &'static [StartMenuItem] {
    START_MENU_ITEMS
}

pub fn desktop_icons() -> &'static [DesktopIcon] {
    DESKTOP_ICONS
}
