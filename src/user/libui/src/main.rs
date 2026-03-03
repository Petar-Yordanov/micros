#![no_std]
#![no_main]

use rlibc::syscall;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    let _ = syscall::log("hello: panic\n");
    loop {}
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let _ = syscall::log("hello: hi from userland\n");
    loop {}
}
