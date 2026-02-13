extern crate alloc;

use crate::sprintln;
use x86_64::VirtAddr;

pub fn spawn_init(kstack_top: VirtAddr) {
    crate::kernel::sched::task::spawn_kthread("init", init_main, core::ptr::null_mut(), kstack_top);
}

extern "C" fn init_main(_: *mut u8) -> ! {
    x86_64::instructions::interrupts::enable();

    let path = "/bin/init.elf";

    sprintln!("[init] exec {}", path);

    sprintln!("[init] before vfs_list(/bin)");
    let r = crate::kernel::fs::vfs::ops::vfs_list("/bin");
    sprintln!("[init] after vfs_list(/bin): {:?}", r.as_ref().map(|v| v.len()));

    crate::kernel::exec::run_user_elf(path, "init-user")
}
