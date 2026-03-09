#![no_std]
#![no_main]

extern crate alloc;

mod app;
mod apps;
mod boot;
mod desktop;
mod icon;
mod input;
mod keymap;
mod render;
mod runtime;
mod window;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    rlibc::proc::exit(1)
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    runtime::run()
}
