extern crate alloc;

use alloc::vec::Vec;
use x86_64::VirtAddr;

use crate::arch::x86_64::descriptors::gdt;
use crate::kernel::mm::aspace::address_space::{new_user_address_space, AddressSpace};
use crate::kernel::sched::proc;
use crate::kernel::sched::task::{self, TrapFrame};
use crate::ksprintln;

#[derive(Debug)]
pub enum ExecError {
    //NotFound,
    BadElf,
    NoMemory,
    //Unsupported,
    VfsReadFail,
}

pub struct LoadedUserElf {
    pub entry: u64,
    pub user_stack_top: u64,
}

pub const USER_STACK_TOP: u64 = 0x0000_7fff_ffff_f000;
pub const USER_STACK_PAGES: usize = 16; // 64 KiB stack

pub fn load_user_elf_into(aspace: &AddressSpace, path: &str) -> Result<LoadedUserElf, ExecError> {
    let img: Vec<u8> = crate::kernel::fs::vfs::ops::vfs_read(path).map_err(|_| ExecError::VfsReadFail)?;
    ksprintln!("[exec] read {} bytes from {}", img.len(), path);
    if img.len() >= 16 {
        ksprintln!(
            "[exec] first16 {:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x}",
            img[0], img[1], img[2], img[3],
            img[4], img[5], img[6], img[7],
            img[8], img[9], img[10], img[11],
            img[12], img[13], img[14], img[15],
        );
    }

    let entry = crate::kernel::exec::elf::load_elf64_user(aspace, &img).map_err(|e| match e {
        crate::kernel::exec::elf::ElfError::NoMem => ExecError::NoMemory,
        //crate::kernel::exec::elf::ElfError::Unsupported => ExecError::Unsupported,
        _ => ExecError::BadElf,
    })?;

    let stack_top = USER_STACK_TOP;
    let stack_size = (USER_STACK_PAGES * 4096) as u64;
    let stack_bottom = stack_top - stack_size;

    crate::kernel::exec::elf::map_user_zero(aspace, stack_bottom, stack_size, true)
        .map_err(|_| ExecError::NoMemory)?;

    let initial_user_rsp = stack_top - 8;

    Ok(LoadedUserElf {
        entry,
        user_stack_top: initial_user_rsp,
    })
}

#[inline(never)]
pub fn enter_user(entry: u64, user_rsp: u64) -> ! {
    let (ucode, udata) = gdt::user_segments();
    let cs = ucode.0 | 3;
    let ss = udata.0 | 3;

    let rflags: u64 = 0x202;

    ksprintln!(
        "[exec] enter_user: rip={:#x} rsp={:#x} cs={:#x} ss={:#x}",
        entry,
        user_rsp,
        cs,
        ss
    );

    unsafe {
        let cur = task::current_ptr();
        if !cur.is_null() {
            (*cur).tf_valid = true;
            (*cur).tf = TrapFrame {
                r15: 0,
                r14: 0,
                r13: 0,
                r12: 0,
                r11: 0,
                r10: 0,
                r9: 0,
                r8: 0,
                rsi: 0,
                rdi: 0,
                rbp: 0,
                rdx: 0,
                rcx: 0,
                rbx: 0,
                rax: 0,

                rip: entry,
                cs: cs as u64,
                rflags,
                user_rsp,
                user_ss: ss as u64,
            };
        }

        core::arch::asm!(
            "mov ax, {udata:x}",
            "mov ds, ax",
            "mov es, ax",

            "push {ss}",
            "push {rsp}",
            "push {rflags}",
            "push {cs}",
            "push {rip}",
            "iretq",

            udata = in(reg) (ss as u16),
            ss = in(reg) ss as u64,
            rsp = in(reg) user_rsp,
            rflags = in(reg) rflags,
            cs = in(reg) cs as u64,
            rip = in(reg) entry,

            options(noreturn)
        );
    }
}

pub fn run_user_elf(path: &str, name: &'static str) -> ! {
    let pid = proc::alloc_pid();
    let aspace = new_user_address_space();

    let loaded = match load_user_elf_into(&aspace, path) {
        Ok(v) => v,
        Err(e) => {
            ksprintln!("[exec] failed to load {}: {:?}", path, e);
            loop {
                x86_64::instructions::hlt();
            }
        }
    };

    {
        let mut procs = proc::all();
        procs.push(proc::Process {
            pid,
            aspace,
            kstack_top: VirtAddr::new(0),
            main_task: unsafe { core::mem::zeroed() },
            state: proc::ProcState::Running,
            name: alloc::string::String::from(name),
        });
    }

    unsafe { aspace.activate(); }

    unsafe {
        let cur = task::current_ptr();
        if !cur.is_null() {
            (*cur).kind = crate::kernel::sched::task::TaskKind::UThread { pid };
        }
    }

    enter_user(loaded.entry, loaded.user_stack_top)
}
