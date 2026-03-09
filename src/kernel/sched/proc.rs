#![allow(dead_code)]

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

use core::sync::atomic::{AtomicU64, Ordering};
use spin::Mutex;
use x86_64::VirtAddr;

use crate::kernel::mm::aspace::address_space::{new_user_address_space, AddressSpace};
use crate::kernel::sched::task::{self, current_ptr, TaskKind};
use crate::ksprintln;

pub type Pid = task::Pid;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ProcState {
    New,
    Running,
    Zombie,
}

#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct TaskHandle(*mut task::Task);
unsafe impl Send for TaskHandle {}

impl TaskHandle {
    #[inline]
    pub const fn new(p: *mut task::Task) -> Self {
        TaskHandle(p)
    }

    #[inline]
    pub fn get(self) -> *mut task::Task {
        self.0
    }
}

#[derive(Clone)]
pub struct Process {
    pub pid: Pid,
    pub aspace: AddressSpace,
    pub kstack_top: VirtAddr,
    pub main_task: TaskHandle,
    pub state: ProcState,
    pub name: String,
}

static NEXT_PID: AtomicU64 = AtomicU64::new(1);
static PROCS: Mutex<Vec<Process>> = Mutex::new(Vec::new());

#[inline]
pub fn alloc_pid() -> Pid {
    NEXT_PID.fetch_add(1, Ordering::Relaxed)
}

pub fn for_pid(pid: Pid) -> Option<Process> {
    let procs = PROCS.lock();
    procs.iter().find(|p| p.pid == pid).cloned()
}

pub fn all<'a>() -> spin::MutexGuard<'a, Vec<Process>> {
    PROCS.lock()
}

pub fn create_kernel_backed_process(
    name: &'static str,
    entry: extern "C" fn(*mut u8) -> !,
    arg: *mut u8,
    kstack_top: VirtAddr,
) -> Pid {
    let pid = alloc_pid();
    let aspace = new_user_address_space();

    let task_ptr = task::spawn_kthread(name, entry, arg, kstack_top);

    unsafe {
        (*task_ptr).kind = TaskKind::UThread { pid };
    }

    {
        let mut procs = PROCS.lock();
        procs.push(Process {
            pid,
            aspace,
            kstack_top,
            main_task: TaskHandle::new(task_ptr),
            state: ProcState::Running,
            name: String::from(name),
        });
    }

    ksprintln!("[proc] created process pid={}", pid);
    pid
}

pub fn current_pid() -> Option<Pid> {
    let cur = current_ptr();
    if cur.is_null() {
        return None;
    }
    unsafe {
        match (*cur).kind {
            TaskKind::KThread { .. } => None,
            TaskKind::UThread { pid } => Some(pid),
        }
    }
}
