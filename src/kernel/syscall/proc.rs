extern crate alloc;

use crate::arch::x86_64::descriptors::gdt;
use crate::kernel::exec::exec_impl::load_user_elf_into;
use crate::kernel::mm::aspace::address_space::{
    new_user_address_space, AddressSpace, KERNEL_ASPACE, ReclaimCursor, reclaim_inactive_step,
};
use alloc::collections::{BTreeMap, BTreeSet};
use spin::Mutex;
use crate::kernel::mm::aspace::user_copy::copy_to_user;
use crate::kernel::sched::proc as kproc;
use crate::kernel::sched::task;
use crate::kernel::sched::task::{TaskKind, TaskState, TrapFrame};
use micros_abi::errno;
use micros_abi::types::{ProcInfo, ProcListEntry, ProcSpawnArgs};

use super::util::{copy_user_str, copy_user_struct};

// Each userspace process reserves 64 MiB of *virtual* heap address space,
// but receives physical memory only as needed. The heap lives well below the
// GUI shared-memory mappings (0x5000_0000_0000) and user stack.
const USER_HEAP_BASE: u64 = 0x0000_3000_0000_0000;
const USER_HEAP_MAX: u64 = 64 * 1024 * 1024;
const PAGE_SIZE: u64 = 4096;

/// Grow the calling process's anonymous heap to cover `wanted` bytes.
/// Returns the committed byte count, or -errno. Never allows arbitrary VAs.
pub(super) fn sys_heap_commit(wanted: u64) -> i64 {
    if wanted == 0 || wanted > USER_HEAP_MAX {
        return -errno::EINVAL;
    }
    let Some(pid) = kproc::current_pid() else {
        return -errno::EPERM;
    };
    let target = (wanted + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
    let mut procs = kproc::all();
    let Some(proc) = procs.iter_mut().find(|p| p.pid == pid && p.state == kproc::ProcState::Running) else {
        return -errno::ESRCH;
    };
    if target > proc.heap_committed {
        let start = USER_HEAP_BASE + proc.heap_committed;
        let len = target - proc.heap_committed;
        if crate::kernel::exec::elf::map_user_zero(&proc.aspace, start, len, true).is_err() {
            return -errno::ENOMEM;
        }
        proc.heap_committed = target;
        // Direct page-table writes require invalidating stale translations.
        x86_64::instructions::tlb::flush_all();
    }
    proc.heap_committed as i64
}

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

/// Terminating a user process must not invoke `schedule()` from a syscall.
/// `schedule()` switches kernel `Context`s, whereas a userspace task only has
/// a resumable `TrapFrame` for the timer's ring-3 scheduling path.
///
/// Retire the task and request a deferred switch. After `iretq` to ring 3,
/// the next timer IRQ switches to a runnable user task; the exiting task is
/// never enqueued again. `rlibc::proc::exit` loops in userspace until then.
pub(super) fn sys_exit(code: u64) -> i64 {
    let pid = kproc::current_pid();
    let cur = task::current_ptr();
    unsafe {
        if !cur.is_null() {
            (*cur).state = TaskState::Zombie;
        }
    }

    if let Some(pid) = pid {
        let mut procs = kproc::all();
        if let Some(p) = procs.iter_mut().find(|p| p.pid == pid) {
            p.state = kproc::ProcState::Zombie;
        }
        drop(procs);
        crate::ksprintln!("[proc] exit pid={} code={} (deferred user-frame switch)", pid, code);
    }

    task::request_resched();
    0
}

pub(super) fn sys_yield() -> i64 {
    task::request_resched();
    0
}

pub(super) fn sys_sleep_ms(ms: u64) -> i64 {
    // Switching a userspace task via switch_context() here is unsafe: its
    // kernel Context is not a saved userspace TrapFrame. Defer the switch to
    // the next user-mode timer IRQ, which saves/restores the full TrapFrame.
    if ms == 0 { task::request_resched(); return 0; }
    task::sleep_current_user(ms);
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

// A zombie's page tables can contain thousands of pages. Cleaning all of them
// during one WM syscall stops composition for the entire teardown. Keep the
// traversal position between calls and do a bounded number of pages per call.
const REAP_LEAF_BUDGET: usize = 32;
struct ReapState {
    cursor: ReclaimCursor,
    shared_frames: BTreeSet<u64>,
    start_tick: u64,
}
static REAP_STATES: Mutex<BTreeMap<u64, ReapState>> = Mutex::new(BTreeMap::new());

/// Nonblocking process wait. In addition to a live process returning EAGAIN,
/// a zombie whose teardown is still in progress also returns EAGAIN.
/// The WM calls `proc::try_wait` once per iteration, never a busy wait.
pub(super) fn sys_proc_wait(pid: u64, status_out_ptr: u64) -> i64 {
    if pid == 0 { return -errno::EINVAL; }
    if kproc::current_pid() == Some(pid) { return -errno::EINVAL; }

    let space = {
        let procs = kproc::all();
        match procs.iter().find(|p| p.pid == pid) {
            Some(p) if p.state == kproc::ProcState::Zombie => p.aspace,
            Some(_) => return -errno::EAGAIN,
            None => return -errno::ESRCH,
        }
    };

    if x86_64::registers::control::Cr3::read().0 == space.root {
        return -errno::EAGAIN;
    }

    // Defer reading the status until completion so callers can distinguish a
    // process that's still being reclaimed from one that's finished.
    let completed = {
        let mut states = REAP_STATES.lock();
        if !states.contains_key(&pid) {
            let shared_frames = super::shm::shared_frames_snapshot();
            states.insert(pid, ReapState {
                cursor: ReclaimCursor::default(),
                shared_frames,
                start_tick: task::jiffies(),
            });
        }
        let state = states.get_mut(&pid).unwrap();
        match reclaim_inactive_step(&space, &state.shared_frames, &mut state.cursor, REAP_LEAF_BUDGET) {
            Ok(true) => {
                crate::ksprintln!(
                    "[perf] process pid={} vm cleanup {} leaves in {} ticks",
                    pid,
                    state.cursor.pages_freed,
                    task::jiffies().saturating_sub(state.start_tick)
                );
                states.remove(&pid);
                true
            }
            Ok(false) => false,
            Err(_) => return -errno::EAGAIN,
        }
    };
    if !completed { return -errno::EAGAIN; }

    // Teardown has completed. Drop the task only after it is definitely off
    // CPU and remove it from every runnable/waiting queue first.
    let victim = {
        let mut procs = kproc::all();
        let Some(index) = procs.iter().position(|p| p.pid == pid && p.state == kproc::ProcState::Zombie)
            else { return -errno::ESRCH; };
        let ptr = procs[index].main_task.get();
        if ptr.is_null() { return -errno::ESRCH; }
        let dead_tid = unsafe { (*ptr).tid };
        if !task::reap_zombie_task(ptr) { return -errno::EAGAIN; }
        (procs.remove(index), dead_tid)
    };
    let (victim, dead_tid) = victim;
    super::shm::release_process(pid);
    super::chan::release_process(pid, dead_tid);
    const KSTACK_RESERVED_PAGES: u64 = 17;
    let base = victim.kstack_top - KSTACK_RESERVED_PAGES * PAGE_SIZE;
    crate::kernel::mm::virt::vmarena::free_n(base, KSTACK_RESERVED_PAGES as usize);

    if status_out_ptr != 0 {
        let zero: u64 = 0;
        unsafe {
            if copy_to_user(
                status_out_ptr as *mut u8,
                &zero as *const _ as *const u8,
                core::mem::size_of::<u64>(),
            ).is_err() { return -errno::EFAULT; }
        }
    }
    crate::ksprintln!("[proc] reaped pid={} resources released", pid);
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
    let spawn_start_ticks = task::jiffies();
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
    unsafe {
        KERNEL_ASPACE.get().unwrap().activate();
    }

    let pid = kproc::alloc_pid();

    const KSTACK_PAGES: usize = 2;
    let kstack_top = task::alloc_kstack_top(KSTACK_PAGES);

    let aspace = new_user_address_space();

    let loaded = match load_user_elf_into(&aspace, &path) {
        Ok(v) => v,
        Err(e) => {
            crate::ksprintln!(
                "[proc] spawn load_user_elf_into failed: {:?} path={}",
                e,
                path
            );
            unsafe {
                caller_aspace.activate();
            }
            return -errno::EINVAL;
        }
    };

    let (ucode, udata) = gdt::user_segments();
    let cs = (ucode.0 | 3) as u64;
    let ss = (udata.0 | 3) as u64;

    let initial_tf = TrapFrame {
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
            heap_committed: 0,
            name: proc_name,
        });
    }

    unsafe {
        caller_aspace.activate();
    }

    crate::ksprintln!(
        "[perf] spawned {} pid={} elapsed_ticks={}",
        path,
        pid,
        task::jiffies().saturating_sub(spawn_start_ticks)
    );
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
