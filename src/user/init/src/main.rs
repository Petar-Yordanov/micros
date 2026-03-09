#![no_std]
#![no_main]

mod bootgfx;

use rlibc::exec::exec;
use rlibc::log::log;
use rlibc::vfs::mount_root_auto;
use crate::bootgfx::splash::run_splash;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    let _ = log("init: panic\n");
    loop {}
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let _ = log("init: starting\n");

    let r = mount_root_auto();
    if r.is_err() {
        let _ = log("init: mount_root_auto failed\n");
        loop {}
    }

    let _ = log("init: mounted root; showing splash\n");
    run_splash();

    let _ = log("init: splash done; exec wm\n");
    let _ = exec("/bin/wm.elf");

    let _ = log("init: exec returned (error)\n");
    loop {}
}
