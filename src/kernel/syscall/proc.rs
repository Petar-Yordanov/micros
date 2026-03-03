extern crate alloc;

use alloc::boxed::Box;
use alloc::string::String;

use micros_abi::errno;

use crate::kernel::mm::aspace::address_space::{AddressSpace, new_user_address_space};
use crate::kernel::mm::aspace::user_copy::copy_to_user;
use crate::kernel::sched::proc as kproc;
use crate::kernel::sched::task;
use crate::kernel::sched::task::{TaskKind, TaskState};
use micros_abi::types::{ProcInfo, ProcSpawnArgs};

use crate::kernel::exec::exec_impl::{enter_user, load_user_elf_into};

use super::util::{copy_user_str, copy_user_struct};

struct SpawnCtx {
    pid: kproc::Pid,
    aspace: AddressSpace,
    path: String,
}

extern "C" fn userproc_entry(arg: *mut u8) -> ! {
    let ctx: Box<SpawnCtx> = unsafe { Box::from_raw(arg as *mut SpawnCtx) };

    unsafe { ctx.aspace.activate(); }

    let loaded = match load_user_elf_into(&ctx.aspace, &ctx.path) {
        Ok(v) => v,
        Err(e) => {
            crate::sprintln!(
                "[proc] spawn load_user_elf_into failed: {:?} path={}",
                e,
                ctx.path
            );
            loop {
                x86_64::instructions::hlt();
            }
        }
    };

    enter_user(loaded.entry, loaded.user_stack_top)
}

pub(super) fn sys_proc_spawn(args_ptr: u64) -> i64 {
    let args: ProcSpawnArgs = match copy_user_struct(args_ptr) {
        Ok(v) => v,
        Err(e) => return e,
    };

    let path = match copy_user_str(args.pathPtr, args.pathLen) {
        Ok(s) => s,
        Err(e) => return e,
    };

    if args.argvPtr != 0 || args.argc != 0 {
        return -errno::ENOSYS;
    }

    let pid = kproc::alloc_pid();
    let aspace = new_user_address_space();

    const KSTACK_SIZE: usize = 2 * 4096;
    let kstack: Box<[u8; KSTACK_SIZE]> = Box::new([0u8; KSTACK_SIZE]);
    let kstack_top = x86_64::VirtAddr::new(kstack.as_ptr() as u64 + KSTACK_SIZE as u64);
    core::mem::forget(kstack);

    let ctx = Box::new(SpawnCtx {
        pid,
        aspace: aspace.clone(),
        path,
    });
    let ctx_ptr = Box::into_raw(ctx) as *mut u8;

    let task_ptr = task::spawn_kthread("userproc", userproc_entry, ctx_ptr, kstack_top);

    unsafe {
        (*task_ptr).kind = TaskKind::UThread { pid };
    }

    {
        let mut procs = kproc::all();
        procs.push(kproc::Process {
            pid,
            aspace,
            kstack_top,
            main_task: kproc::TaskHandle::new(task_ptr),
            state: kproc::ProcState::Running,
            name: "userproc",
        });
    }

    crate::sprintln!("[syscall] sys_proc_spawn pid={} path={}", pid, "<copied>");
    pid as i64
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
    task::yield_now();
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

pub(super) fn sys_proc_list(out_ptr: u64, out_cap_entries: u64) -> i64 {
    if out_ptr == 0 {
        return -errno::EFAULT;
    }

    let cap = out_cap_entries as usize;
    if cap == 0 {
        return 0;
    }

    let procs = kproc::all();
    let n = core::cmp::min(cap, procs.len());

    for i in 0..n {
        let p = &procs[i];
        let state_u32: u32 = match p.state {
            kproc::ProcState::New => 0,
            kproc::ProcState::Running => 1,
            kproc::ProcState::Zombie => 2,
        };

        let info = ProcInfo {
            pid: p.pid as u32,
            state: state_u32,
            namePtr: 0,
            nameLen: 0,
            _pad: 0,
        };

        let dst = out_ptr + (i * core::mem::size_of::<ProcInfo>()) as u64;
        unsafe {
            if copy_to_user(
                dst as *mut u8,
                &info as *const _ as *const u8,
                core::mem::size_of::<ProcInfo>(),
            )
            .is_err()
            {
                return -errno::EFAULT;
            }
        }
    }

    n as i64
}
