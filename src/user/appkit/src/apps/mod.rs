use alloc::boxed::Box;
use crate::app::{App,AppLaunch};
use crate::AppId;
pub mod browser;
pub mod browser_renderer;
pub mod clock;
pub mod demo;
pub mod explorer;
pub mod hello;
pub mod netinfo;
pub mod notepad;
pub mod taskmgr;

pub fn make_app(id:AppId)->Box<dyn App> {
    match id {
        AppId::Hello=>Box::new(hello::HelloApp::new()),
        AppId::Demo=>Box::new(demo::DemoApp::new()),
        AppId::Explorer=>Box::new(explorer::ExplorerApp::new()),
        AppId::TaskManager=>Box::new(taskmgr::TaskManagerApp::new()),
        AppId::Clock=>Box::new(clock::ClockApp::new()),
        AppId::Notepad=>Box::new(notepad::NotepadApp::new()),
        AppId::NetInfo=>Box::new(netinfo::NetInfoApp::new()),
        AppId::Browser=>Box::new(browser::MiniBrowserApp::new()),
    }
}
pub fn make_launch(req:AppLaunch)->Box<dyn App> {
    match req {AppLaunch::App(id)=>make_app(id),AppLaunch::TextFile(p)=>Box::new(notepad::NotepadApp::open(&p))}
}
