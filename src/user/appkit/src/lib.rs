#![no_std]
extern crate alloc;
pub mod app;
pub mod apps;
pub use app::{App, AppLaunch};
pub use libui::protocol::AppId;
