pub extern "C" fn idle(_: *mut u8) -> ! {
    x86_64::instructions::interrupts::enable();
    loop {
        x86_64::instructions::hlt();
    }
}
