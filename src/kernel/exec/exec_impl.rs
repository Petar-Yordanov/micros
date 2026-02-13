extern crate alloc;

use alloc::vec::Vec;
use x86_64::VirtAddr;

use crate::arch::x86_64::descriptors::gdt;
use crate::kernel::mm::aspace::address_space::{new_user_address_space, AddressSpace};
use crate::kernel::sched::proc;
use crate::sprintln;

#[derive(Debug)]
pub enum ExecError {
    NotFound,
    BadElf,
    NoMemory,
    Unsupported,
    VfsReadFail,
}

pub struct LoadedUserElf {
    pub entry: u64,
    pub user_stack_top: u64,
}

pub const USER_STACK_TOP: u64 = 0x0000_7fff_ffff_f000;
pub const USER_STACK_PAGES: usize = 16; // 64 KiB stack for now

pub fn load_user_elf_into(aspace: &AddressSpace, path: &str) -> Result<LoadedUserElf, ExecError> {
    let img: Vec<u8> = crate::kernel::fs::vfs::ops::vfs_read(path)
        .map_err(|_| ExecError::VfsReadFail)?;

    let entry = crate::kernel::exec::elf::load_elf64_user(aspace, &img)
        .map_err(|_| ExecError::BadElf)?;

    let stack_top = USER_STACK_TOP;
    let stack_size = (USER_STACK_PAGES * 4096) as u64;
    let stack_bottom = stack_top - stack_size;

    crate::kernel::exec::elf::map_user_zero(aspace, stack_bottom, stack_size, true)
        .map_err(|_| ExecError::NoMemory)?;

    Ok(LoadedUserElf {
        entry,
        user_stack_top: stack_top,
    })
}

#[inline(never)]
pub fn enter_user(entry: u64, user_rsp: u64) -> ! {
    let (ucode, udata) = gdt::user_segments();
    let cs = ucode.0 | 3;
    let ss = udata.0 | 3;

    let rflags: u64 = 0x202;

    sprintln!(
        "[exec] enter_user: rip={:#x} rsp={:#x} cs={:#x} ss={:#x}",
        entry,
        user_rsp,
        cs,
        ss
    );

    unsafe {
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
            sprintln!("[exec] failed to load {}: {:?}", path, e);
            loop { x86_64::instructions::hlt(); }
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
            name,
        });
    }

    unsafe { aspace.activate(); }

    enter_user(loaded.entry, loaded.user_stack_top)
}
