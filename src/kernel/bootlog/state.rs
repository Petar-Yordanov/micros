use core::ptr::addr_of_mut;

use super::console::BootConsole;

static mut BOOT_CONSOLE: BootConsole = BootConsole::new();
static mut BOOTLOG_FB_ENABLED: bool = true;

pub unsafe fn try_init() {
    (*addr_of_mut!(BOOT_CONSOLE)).try_init();
}

pub fn bootlog_push_line(s: &str) {
    unsafe {
        if BOOTLOG_FB_ENABLED {
            (*addr_of_mut!(BOOT_CONSOLE)).push_line(s);
        }
    }
}

pub fn bootlog_fb_enable() {
    unsafe {
        *addr_of_mut!(BOOTLOG_FB_ENABLED) = true;
    }
}

pub fn bootlog_fb_disable() {
    unsafe {
        *addr_of_mut!(BOOTLOG_FB_ENABLED) = false;
    }
}

//pub fn bootlog_set_progress(done: usize, total: usize) {
//    unsafe {
//        (*addr_of_mut!(BOOT_CONSOLE)).set_progress(done, total);
//    }
//}

pub fn bootlog_set_progress_total(total: usize) {
    unsafe {
        (*addr_of_mut!(BOOT_CONSOLE)).set_progress_total(total);
    }
}

#[inline(always)]
pub fn boot_progress_step(done: usize) {
    unsafe {
        let total = (*addr_of_mut!(BOOT_CONSOLE)).progress_total();
        (*addr_of_mut!(BOOT_CONSOLE)).set_progress(done, total);
    }
}
