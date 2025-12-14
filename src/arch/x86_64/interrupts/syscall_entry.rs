use x86_64::structures::idt::InterruptStackFrame;

pub extern "x86-interrupt" fn syscall_entry(_sf: InterruptStackFrame) {
    let nr: u64;
    let a0: u64;
    let a1: u64;
    let a2: u64;
    let a3: u64;

    unsafe {
        core::arch::asm!(
            "mov {nr}, rax",
            "mov {a0}, rdi",
            "mov {a1}, rsi",
            "mov {a2}, rdx",
            "mov {a3}, r10",
            nr = out(reg) nr,
            a0 = out(reg) a0,
            a1 = out(reg) a1,
            a2 = out(reg) a2,
            a3 = out(reg) a3,
            options(nostack, preserves_flags),
        );
    }

    let ret = dispatch(nr, a0, a1, a2, a3);

    unsafe {
        core::arch::asm!(
            "mov rax, {ret}",
            ret = in(reg) ret,
            options(nostack, preserves_flags),
        );
    }
}

pub fn dispatch(nr: u64, a0: u64, a1: u64, a2: u64, a3: u64) -> u64 {
    match nr {
        nr::SYS_FB_INFO => sys_fb_info(a0) as u64,

        // TODO: VFS syscalls:
        // nr::SYS_VFS_READ  => sys_vfs_read(...),
        // nr::SYS_VFS_WRITE => sys_vfs_write(...),
        _ => {
            sprintln!(
                "[syscall] unknown nr={} (a0={:#x} a1={:#x} a2={:#x} a3={:#x})",
                nr,
                a0,
                a1,
                a2,
                a3
            );
            !0u64
        }
    }
}
