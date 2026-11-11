#![no_std]
#![no_main]
use appkit::AppId;
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! { rlibc::proc::exit(1) }
#[no_mangle]
pub extern "C" fn _start() -> ! {app_host::run(AppId::NetInfo)}
