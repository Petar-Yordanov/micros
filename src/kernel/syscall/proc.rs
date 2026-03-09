extern crate alloc;

use crate::arch::x86_64::descriptors::gdt;
use micros_abi::errno;
use micros_abi::types::{ProcInfo, ProcListEntry, ProcSpawnArgs};
use crate::kernel::exec::exec_impl::load_user_elf_into;
use crate::kernel::mm::aspace::address_space::{new_user_address_space, AddressSpace, KERNEL_ASPACE};
use crate::kernel::mm::aspace::user_copy::copy_to_user;
use crate::kernel::sched::proc as kproc;
use crate::kernel::sched::task;
use crate::kernel::sched::task::{TaskKind, TaskState, TrapFrame};

use super::util::{copy_user_str, copy_user_struct};

fn proc_state_to_u32(state: kproc::ProcState) -> u32 {
    match state {
        kproc::ProcState::New => 0,
        kproc::ProcState::Running => 1,
        kproc::ProcState::Zombie => 2,
    }
}

fn fill_name_buf(dst: &mut [u8; 32], name: &str) -> u32 {
    let bytes = name.as_bytes();
    let n = core::cmp::min(bytes.len(), dst.len());
    dst[..n].copy_from_slice(&bytes[..n]);
    n as u32
}

pub(super) fn sys_exit(_code: u64) -> i64 {
    unsafe {
        let cur = task::current_ptr();
        if !cur.is_null() {
            (*cur).state = TaskState::Zombie;
        }
    }

    if let Some(pid) = crate::kernel::sched::proc::current_pid() {
        let mut procs = crate::kernel::sched::proc::all();
        for p in procs.iter_mut() {
            if p.pid == pid {
                p.state = crate::kernel::sched::proc::ProcState::Zombie;
                break;
            }
        }
    }

    loop {
        task::yield_now();
        x86_64::instructions::hlt();
    }
}

pub(super) fn sys_yield() -> i64 {
    task::request_resched();
    0
}

pub(super) fn sys_sleep_ms(ms: u64) -> i64 {
    task::sleep_ms(ms);
    0
}

pub(super) fn sys_getpid() -> i64 {
    unsafe {
        let cur = task::current_ptr();
        if cur.is_null() {
            return -errno::ENOSYS;
        }

        match (*cur).kind {
            TaskKind::UThread { pid } => pid as i64,
            TaskKind::KThread { .. } => 0,
        }
    }
}

pub(super) fn sys_proc_wait(pid: u64, status_out_ptr: u64) -> i64 {
    if pid == 0 {
        return -errno::EINVAL;
    }

    loop {
        let st = {
            let procs = kproc::all();
            match procs.iter().find(|p| p.pid == pid) {
                Some(p) => p.state,
                None => return -errno::ESRCH,
            }
        };

        if st == kproc::ProcState::Zombie {
            break;
        }

        task::yield_now();
    }

    if status_out_ptr != 0 {
        let zero: u64 = 0;
        unsafe {
            if copy_to_user(
                status_out_ptr as *mut u8,
                &zero as *const _ as *const u8,
                core::mem::size_of::<u64>(),
            )
            .is_err()
            {
                return -errno::EFAULT;
            }
        }
    }

    pid as i64
}

pub(super) fn sys_proc_kill(pid: u64, _signal_or_reason: u64) -> i64 {
    if pid == 0 {
        return -errno::EINVAL;
    }

    let mut found = false;

    let mut procs = kproc::all();
    for p in procs.iter_mut() {
        if p.pid == pid {
            found = true;
            p.state = kproc::ProcState::Zombie;

            let tp = p.main_task.get();
            unsafe {
                if !tp.is_null() {
                    (*tp).state = TaskState::Zombie;
                }
            }
            break;
        }
    }

    if !found {
        return -errno::ESRCH;
    }

    0
}

pub(super) fn sys_proc_list(out_ptr: u64, out_cap_entries: u64, total_out_ptr: u64) -> i64 {
    let procs = kproc::all();
    let total = procs.len() as u64;

    if total_out_ptr != 0 {
        unsafe {
            if copy_to_user(
                total_out_ptr as *mut u8,
                &total as *const _ as *const u8,
                core::mem::size_of::<u64>(),
            )
            .is_err()
            {
                return -errno::EFAULT;
            }
        }
    }

    if out_cap_entries == 0 {
        return 0;
    }

    if out_ptr == 0 {
        return -errno::EFAULT;
    }

    let cap = out_cap_entries as usize;
    let n = core::cmp::min(cap, procs.len());

    for i in 0..n {
        let p = &procs[i];
        let mut entry = ProcListEntry::default();
        entry.pid = p.pid as u32;
        entry.state = proc_state_to_u32(p.state);
        entry.name_len = fill_name_buf(&mut entry.name, &p.name);

        let dst = out_ptr + (i * core::mem::size_of::<ProcListEntry>()) as u64;
        unsafe {
            if copy_to_user(
                dst as *mut u8,
                &entry as *const _ as *const u8,
                core::mem::size_of::<ProcListEntry>(),
            )
            .is_err()
            {
                return -errno::EFAULT;
            }
        }
    }

    n as i64
}

pub(super) fn sys_proc_info(pid: u64, out_ptr: u64) -> i64 {
    if pid == 0 {
        return -errno::EINVAL;
    }

    if out_ptr == 0 {
        return -errno::EFAULT;
    }

    let procs = kproc::all();
    let p = match procs.iter().find(|p| p.pid == pid) {
        Some(v) => v,
        None => return -errno::ESRCH,
    };

    let mut info = ProcInfo::default();
    info.pid = p.pid as u32;
    info.state = proc_state_to_u32(p.state);
    info.main_tid = unsafe {
        let tp = p.main_task.get();
        if tp.is_null() {
            0
        } else {
            (*tp).tid
        }
    };
    info.name_len = fill_name_buf(&mut info.name, &p.name);

    unsafe {
        if copy_to_user(
            out_ptr as *mut u8,
            &info as *const _ as *const u8,
            core::mem::size_of::<ProcInfo>(),
        )
        .is_err()
        {
            return -errno::EFAULT;
        }
    }

    0
}

pub(super) fn sys_proc_spawn(args_ptr: u64) -> i64 {
    let args: ProcSpawnArgs = match copy_user_struct(args_ptr) {
        Ok(v) => v,
        Err(e) => return e,
    };

    let path = match copy_user_str(args.path_ptr, args.path_len) {
        Ok(s) => s,
        Err(e) => return e,
    };

    if args.argv_ptr != 0 || args.argc != 0 {
        return -errno::ENOSYS;
    }

    let caller_aspace = AddressSpace::from_current();
    unsafe { KERNEL_ASPACE.get().unwrap().activate(); }

    let pid = kproc::alloc_pid();

    const KSTACK_PAGES: usize = 2;
    let kstack_top = task::alloc_kstack_top(KSTACK_PAGES);

    let aspace = new_user_address_space();

    let loaded = match load_user_elf_into(&aspace, &path) {
        Ok(v) => v,
        Err(e) => {
            crate::ksprintln!("[proc] spawn load_user_elf_into failed: {:?} path={}", e, path);
            unsafe { caller_aspace.activate(); }
            return -errno::EINVAL;
        }
    };

    let (ucode, udata) = gdt::user_segments();
    let cs = (ucode.0 | 3) as u64;
    let ss = (udata.0 | 3) as u64;

    let initial_tf = TrapFrame {
        r15: 0, r14: 0, r13: 0, r12: 0,
        r11: 0, r10: 0, r9: 0,  r8: 0,
        rsi: 0, rdi: 0, rbp: 0,
        rdx: 0, rcx: 0, rbx: 0, rax: 0,

        rip: loaded.entry,
        cs,
        rflags: 0x202,
        user_rsp: loaded.user_stack_top,
        user_ss: ss,
    };

    let proc_name = path_basename(&path);
    let task_ptr = task::spawn_uthread("userproc", pid, kstack_top, initial_tf);

    {
        let mut procs = kproc::all();
        procs.push(kproc::Process {
            pid,
            aspace: aspace.clone(),
            kstack_top,
            main_task: kproc::TaskHandle::new(task_ptr),
            state: kproc::ProcState::Running,
            name: proc_name,
        });
    }

    unsafe { caller_aspace.activate(); }

    pid as i64
}

fn path_basename(path: &str) -> alloc::string::String {
    let mut last = "";

    for part in path.split('/') {
        if !part.is_empty() {
            last = part;
        }
    }

    if last.is_empty() {
        alloc::string::String::from("userproc")
    } else {
        alloc::string::String::from(last)
    }
}
