use crate::arch::x86_64::descriptors::tss;
use crate::kernel::sched::proc as kproc;
use crate::ksprintln;

use alloc::{boxed::Box, collections::VecDeque};
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use spin::Mutex;
use x86_64::VirtAddr;

pub type TaskId = u64;
pub type Pid = u64;

extern "C" {
    fn switch_context(old: *mut u8, new: *const u8);
}

use crate::kernel::mm::map::mapper as page;
use crate::kernel::mm::phys::frame;
use crate::kernel::mm::virt::vmarena;

const MIN_KSTACK_PAGES: usize = 16; // 64 KiB

pub fn alloc_kstack_top(pages: usize) -> VirtAddr {
    let pages = core::cmp::max(pages, MIN_KSTACK_PAGES);

    let total = pages + 1;
    let base = vmarena::alloc_n(total).expect("kstack vmarena alloc");

    if let Ok(pf) = page::unmap(base) {
        frame::free(pf);
    }

    base + ((total as u64) * 4096u64)
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskState {
    New,
    Runnable,
    Running,
    Sleeping,
    Blocked,
    Zombie,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct Context {
    pub rsp: u64,
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub rbx: u64,
    pub rbp: u64,
    pub rdi: u64,
    pub rip: u64,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct TrapFrame {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rbp: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rbx: u64,
    pub rax: u64,

    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub user_rsp: u64,
    pub user_ss: u64,
}

#[allow(dead_code)]
pub enum TaskKind {
    KThread {
        entry: extern "C" fn(*mut u8) -> !,
        arg: *mut u8,
    },
    UThread {
        pid: Pid,
    },
}

pub struct Task {
    pub tid: TaskId,
    pub state: TaskState,
    pub kind: TaskKind,
    pub kstack_top: VirtAddr,
    pub ctx: Context,
    pub wake_jiffies: u64,
    pub timeslice: u32,

    pub tf_valid: bool,
    pub tf: TrapFrame,

    #[allow(dead_code)]
    pub name: &'static str,
}

#[repr(transparent)]
#[derive(Copy, Clone)]
struct TaskPtr(*mut Task);
unsafe impl Send for TaskPtr {}
impl TaskPtr {
    #[inline]
    const fn new(p: *mut Task) -> Self {
        TaskPtr(p)
    }
    #[inline]
    fn get(&self) -> *mut Task {
        self.0
    }
}

static NEXT_TID: AtomicU64 = AtomicU64::new(1);
static JIFFIES: AtomicU64 = AtomicU64::new(0);
const DEFAULT_SLICE: u32 = 8;
static INITED: AtomicBool = AtomicBool::new(false);

static RUNQ: Mutex<VecDeque<TaskPtr>> = Mutex::new(VecDeque::new());
static CURRENT: Mutex<TaskPtr> = Mutex::new(TaskPtr::new(core::ptr::null_mut()));
static NEED_RESCHED: AtomicBool = AtomicBool::new(false);
static IDLE: Mutex<TaskPtr> = Mutex::new(TaskPtr::new(core::ptr::null_mut()));

static IN_SYSCALL: AtomicBool = AtomicBool::new(false);

#[inline]
pub fn syscall_enter() {
    IN_SYSCALL.store(true, Ordering::Release);
}

#[inline]
pub fn syscall_exit() {
    IN_SYSCALL.store(false, Ordering::Release);
}

#[inline]
pub fn in_syscall() -> bool {
    IN_SYSCALL.load(Ordering::Acquire)
}

#[inline]
fn idle_ptr() -> *mut Task {
    let p = IDLE.lock().get();
    if p.is_null() {
        panic!("[sched] IDLE task is NULL (init() not called or corrupted)");
    }
    p
}

#[inline]
pub fn jiffies() -> u64 {
    JIFFIES.load(Ordering::Relaxed)
}

#[inline]
pub fn ms_to_ticks(ms: u64) -> u64 {
    ms
}

static mut BOOT_DUMMY_CTX: Context = Context {
    rsp: 0,
    r15: 0,
    r14: 0,
    r13: 0,
    r12: 0,
    rbx: 0,
    rbp: 0,
    rdi: 0,
    rip: 0,
};

pub fn init(idle_task: *mut Task) {
    assert!(!idle_task.is_null(), "idle_task must not be null");

    *IDLE.lock() = TaskPtr::new(idle_task);

    unsafe {
        (*idle_task).state = TaskState::Runnable;
        (*idle_task).timeslice = u32::MAX;
    }

    {
        let mut rq = RUNQ.lock();
        let mut tmp = VecDeque::new();
        while let Some(tp) = rq.pop_front() {
            if tp.get() != idle_task {
                tmp.push_back(tp);
            }
        }
        *rq = tmp;
    }

    *CURRENT.lock() = TaskPtr::new(core::ptr::null_mut());
    INITED.store(true, Ordering::Release);
}

#[unsafe(naked)]
extern "C" fn kthread_trampoline() -> ! {
    core::arch::naked_asm!(
        r#"
        pop rax
        pop rdi
        sti
        jmp rax
    "#
    );
}

pub fn spawn_kthread(
    name: &'static str,
    entry: extern "C" fn(*mut u8) -> !,
    arg: *mut u8,
    kstack_top: VirtAddr,
) -> *mut Task {
    let tid = NEXT_TID.fetch_add(1, Ordering::Relaxed);

    let mut sp = kstack_top.as_u64();
    unsafe {
        sp -= 8;
        *(sp as *mut u64) = kthread_exit as *const () as u64;
        sp -= 8;
        *(sp as *mut u64) = arg as u64;
        sp -= 8;
        *(sp as *mut u64) = entry as u64;
    }

    let t = Box::new(Task {
        tid,
        state: TaskState::Runnable,
        kind: TaskKind::KThread { entry, arg },
        kstack_top,
        ctx: Context {
            rsp: sp,
            rdi: arg as u64,
            rip: kthread_trampoline as *const () as u64,
            ..Context::default()
        },
        wake_jiffies: 0,
        timeslice: DEFAULT_SLICE,

        tf_valid: false,
        tf: TrapFrame::default(),

        name,
    });

    ksprintln!(
        "[task] spawn tid={} name={} kstack_top={:#x} saved_rsp={:#x}",
        tid,
        name,
        kstack_top.as_u64(),
        sp
    );

    let raw = Box::into_raw(t);
    RUNQ.lock().push_back(TaskPtr::new(raw));
    raw
}

pub fn spawn_uthread(
    name: &'static str,
    pid: Pid,
    kstack_top: VirtAddr,
    initial_tf: TrapFrame,
) -> *mut Task {
    let tid = NEXT_TID.fetch_add(1, Ordering::Relaxed);

    let t = Box::new(Task {
        tid,
        state: TaskState::Runnable,
        kind: TaskKind::UThread { pid },
        kstack_top,
        ctx: Context::default(),
        wake_jiffies: 0,
        timeslice: DEFAULT_SLICE,

        tf_valid: true,
        tf: initial_tf,

        name,
    });

    ksprintln!(
        "[task] spawn tid={} name={} kstack_top={:#x} (uthread)",
        tid,
        name,
        kstack_top.as_u64(),
    );

    let raw = Box::into_raw(t);
    RUNQ.lock().push_back(TaskPtr::new(raw));
    raw
}

#[inline]
pub fn current_ptr() -> *mut Task {
    CURRENT.lock().get()
}

#[inline]
pub fn request_resched() {
    NEED_RESCHED.store(true, Ordering::Release);
}

pub fn yield_now() {
    request_resched();
    // A userspace process must be switched by the ring-3 timer TrapFrame
    // path, never by the kernel-only `switch_context()` mechanism.
    let cur = current_ptr();
    if in_syscall() || (!cur.is_null() && unsafe { matches!(&(*cur).kind, TaskKind::UThread { .. }) }) {
        return;
    }
    schedule();
}

/// Mark the current user process sleeping; the timer IRQ performs the actual
/// TrapFrame switch after the syscall returns to ring 3.
/// A pending receive is handled by the user timer interrupt, never by
/// switch_context() (which is only safe for kernel threads).
pub fn current_tid()->Option<TaskId> {
    let cur=current_ptr();
    if cur.is_null(){None}else{Some(unsafe{(*cur).tid})}
}
pub fn block_current_user() {
    let cur=current_ptr();
    if cur.is_null(){return;}
    unsafe {
        if let TaskKind::UThread { .. }=(*cur).kind {
            if (*cur).state != TaskState::Blocked {
                (*cur).state=TaskState::Blocked;
                RUNQ.lock().push_back(TaskPtr::new(cur));
            }
            request_resched();
        }
    }
}
pub fn wake_tid(tid:TaskId) {
    let rq=RUNQ.lock();
    for tp in rq.iter() {
        let t=tp.get();
        unsafe {
            if !t.is_null() && (*t).tid==tid &&
                ((*t).state==TaskState::Blocked || (*t).state==TaskState::Sleeping) {
                (*t).state=TaskState::Runnable;
                (*t).timeslice=DEFAULT_SLICE;
                request_resched();
                break;
            }
        }
    }
}

pub fn sleep_current_user(ms: u64) {
    let cur = current_ptr();
    if cur.is_null() { return; }
    unsafe {
        if let TaskKind::UThread { .. } = (*cur).kind {
            if (*cur).state != TaskState::Sleeping {
                (*cur).wake_jiffies = jiffies().saturating_add(ms_to_ticks(ms));
                (*cur).state = TaskState::Sleeping;
                RUNQ.lock().push_back(TaskPtr::new(cur));
            }
            request_resched();
        }
    }
}

pub fn sleep_ms(ms: u64) {
    let wake = jiffies().saturating_add(ms_to_ticks(ms));
    unsafe {
        let cur = current_ptr();
        (*cur).state = TaskState::Sleeping;
        (*cur).wake_jiffies = wake;
        RUNQ.lock().push_back(TaskPtr::new(cur));
    }
    NEED_RESCHED.store(true, Ordering::Release);
    schedule();
}

/// Timer IRQs may interrupt userspace syscalls while kernel spin mutexes are
/// held. An IRQ must NEVER wait for a mutex owned by the interrupted context:
/// that context cannot resume to release it until the IRQ returns.
///
/// Advance the clock unconditionally, but skip run-queue bookkeeping on a
/// contended tick. The next tick will revisit all sleeping tasks using their
/// absolute wake deadline, so no wakeup is lost.
pub fn on_tick() {
    let now = JIFFIES.fetch_add(1, Ordering::Relaxed).saturating_add(1);

    // `current_ptr()` and `idle_ptr()` take blocking spin locks. Do not call
    // those helpers from an interrupt that may have interrupted their owners.
    if let (Some(cur_guard), Some(idle_guard)) = (CURRENT.try_lock(), IDLE.try_lock()) {
        let cur = cur_guard.get();
        let idle = idle_guard.get();
        unsafe {
            if !cur.is_null() && cur != idle && (*cur).state == TaskState::Running {
                if (*cur).timeslice > 0 {
                    (*cur).timeslice -= 1;
                }
                if (*cur).timeslice == 0 {
                    NEED_RESCHED.store(true, Ordering::Release);
                }
            }
        }
    }

    // This MUST be non-blocking: syscall paths also hold RUNQ.lock() while
    // interrupts are enabled. Skipping one tick is safe; wake_jiffies is an
    // absolute deadline and will be checked again at the next IRQ.
    if let Some(mut runq) = RUNQ.try_lock() {
        let mut woke_any = false;
        for tp in runq.iter_mut() {
            let t = tp.get();
            unsafe {
                if !t.is_null() && (*t).state == TaskState::Sleeping && (*t).wake_jiffies <= now {
                    (*t).state = TaskState::Runnable;
                    (*t).timeslice = DEFAULT_SLICE;
                    woke_any = true;
                }
            }
        }
        if woke_any {
            NEED_RESCHED.store(true, Ordering::Release);
        }
    }
}

#[inline(always)]
unsafe fn prepare_next_task_machine_state(next_ptr: *mut Task) {
    match (*next_ptr).kind {
        TaskKind::UThread { pid } => {
            if let Some(p) = kproc::for_pid(pid) {
                p.aspace.activate();
            } else {
                ksprintln!("[sched] warning: missing process for pid={} (UThread)", pid);
            }
            tss::set_rsp0_top((*next_ptr).kstack_top.as_u64());
            //ksprintln!(
            //    "[tss] rsp0 <- {:#x} (tid={})",
            //    (*next_ptr).kstack_top.as_u64(),
            //    (*next_ptr).tid
            //);
        }
        TaskKind::KThread { .. } => {
            tss::set_rsp0_top((*next_ptr).kstack_top.as_u64());
            //ksprintln!(
            //    "[tss] rsp0 <- {:#x} (tid={})",
            //    (*next_ptr).kstack_top.as_u64(),
            //    (*next_ptr).tid
            //);
        }
    }
}

pub unsafe fn preempt_from_timer(tf: &mut TrapFrame) -> *const TrapFrame {
    if !INITED.load(Ordering::Acquire) {
        return core::ptr::null();
    }

    if !NEED_RESCHED.load(Ordering::Acquire) {
        return core::ptr::null();
    }

    if (tf.cs & 3) != 3 {
        NEED_RESCHED.store(false, Ordering::Release);
        return core::ptr::null();
    }

    let cur_ptr = CURRENT.lock().get();
    if cur_ptr.is_null() {
        NEED_RESCHED.store(false, Ordering::Release);
        return core::ptr::null();
    }

    (*cur_ptr).tf = *tf;
    (*cur_ptr).tf_valid = true;

    if cur_ptr != idle_ptr() && (*cur_ptr).state == TaskState::Running {
        (*cur_ptr).state = TaskState::Runnable;
        RUNQ.lock().push_back(TaskPtr::new(cur_ptr));
    }

    let next_ptr = {
        let mut runq = RUNQ.lock();
        let len = runq.len();
        let mut picked: *mut Task = core::ptr::null_mut();

        for _ in 0..len {
            if let Some(tp) = runq.pop_front() {
                let t = tp.get();
                if !t.is_null()
                    && (*t).state == TaskState::Runnable
                    && matches!(&(*t).kind, TaskKind::UThread { .. })
                    && (*t).tf_valid
                    && ((*t).tf.cs & 3) == 3
                {
                    picked = t;
                    break;
                } else {
                    runq.push_back(tp);
                }
            } else {
                break;
            }
        }

        // Kernel threads are switched via their saved kernel `Context`, not
        // via an interrupt return frame. If no eligible user task exists,
        // leave this task running until a valid user candidate is available.
        picked
    };

    if next_ptr.is_null() || next_ptr == cur_ptr {
        // There is no alternative ring-3 task yet. A blocked/sleeping task
        // may temporarily continue polling, but NEVER resurrect an exited
        // task as runnable; retry the exit handoff on the next timer tick.
        if (*cur_ptr).state == TaskState::Zombie {
            NEED_RESCHED.store(true, Ordering::Release);
        } else {
            (*cur_ptr).state = TaskState::Running;
            NEED_RESCHED.store(false, Ordering::Release);
        }
        return core::ptr::null();
    }

    if !(*next_ptr).tf_valid || ((*next_ptr).tf.cs & 3) != 3 {
        (*cur_ptr).state = TaskState::Running;
        NEED_RESCHED.store(false, Ordering::Release);
        return core::ptr::null();
    }

    (*next_ptr).state = TaskState::Running;
    prepare_next_task_machine_state(next_ptr);

    *CURRENT.lock() = TaskPtr::new(next_ptr);
    NEED_RESCHED.store(false, Ordering::Release);

    &(*next_ptr).tf as *const TrapFrame
}

pub fn schedule() {
    use core::ptr::{addr_of, addr_of_mut};
    use x86_64::instructions::interrupts;

    if !INITED.load(Ordering::Acquire) {
        return;
    }

    interrupts::disable();

    let cur_ptr = CURRENT.lock().get();
    if !cur_ptr.is_null() && unsafe { matches!(&(*cur_ptr).kind, TaskKind::UThread { .. }) } {
        // A userspace task does not have a kernel `Context` suitable for
        // switch_context(). The ring-3 timer IRQ owns its context switches.
        NEED_RESCHED.store(true, Ordering::Release);
        interrupts::enable();
        return;
    }
    let first_handoff = cur_ptr.is_null();

    let next_ptr = {
        let mut runq = RUNQ.lock();
        let len = runq.len();
        let mut picked: *mut Task = core::ptr::null_mut();

        for _ in 0..len {
            if let Some(tp) = runq.pop_front() {
                let t = tp.get();
                unsafe {
                    if !t.is_null() && (*t).state == TaskState::Runnable
                        && matches!(&(*t).kind, TaskKind::KThread { .. }) {
                        picked = t;
                        break;
                    } else {
                        runq.push_back(tp);
                    }
                }
            } else {
                break;
            }
        }

        if picked.is_null() {
            idle_ptr()
        } else {
            picked
        }
    };

    if next_ptr.is_null() {
        interrupts::enable();
        return;
    }

    if first_handoff {
        unsafe {
            if (*next_ptr).timeslice == 0 {
                (*next_ptr).timeslice = DEFAULT_SLICE;
            }
            (*next_ptr).state = TaskState::Running;
            prepare_next_task_machine_state(next_ptr);
        }

        *CURRENT.lock() = TaskPtr::new(next_ptr);
        NEED_RESCHED.store(false, Ordering::Release);

        unsafe {
            switch_context(
                addr_of_mut!(BOOT_DUMMY_CTX) as *mut u8,
                addr_of!((*next_ptr).ctx) as *const u8,
            );
        }
    }

    unsafe {
        if next_ptr != cur_ptr && !cur_ptr.is_null() && (*cur_ptr).state == TaskState::Running {
            (*cur_ptr).state = TaskState::Runnable;
            RUNQ.lock().push_back(TaskPtr::new(cur_ptr));
        }
    }

    if next_ptr == cur_ptr {
        unsafe {
            if cur_ptr == idle_ptr() {
                (*cur_ptr).timeslice = u32::MAX;
            }
        }
        NEED_RESCHED.store(false, Ordering::Release);
        interrupts::enable();
        return;
    }

    unsafe {
        if (*next_ptr).timeslice == 0 {
            (*next_ptr).timeslice = if next_ptr == idle_ptr() {
                u32::MAX
            } else {
                DEFAULT_SLICE
            };
        }
        (*next_ptr).state = TaskState::Running;
        prepare_next_task_machine_state(next_ptr);
    }

    *CURRENT.lock() = TaskPtr::new(next_ptr);
    NEED_RESCHED.store(false, Ordering::Release);

    unsafe {
        switch_context(
            addr_of_mut!((*cur_ptr).ctx) as *mut u8,
            addr_of!((*next_ptr).ctx) as *const u8,
        );
    }

    interrupts::enable();
}

extern "C" fn kthread_exit(_: *mut u8) -> ! {
    unsafe {
        (*current_ptr()).state = TaskState::Zombie;
    }
    loop {
        yield_now();
        x86_64::instructions::hlt();
    }
}

#[allow(dead_code)]
impl Task {
    pub fn check_stack_bounds(&self) {
        let top = self.kstack_top.as_u64();
        let bottom = top - ((MIN_KSTACK_PAGES as u64) * 4096);

        let rsp = self.ctx.rsp;
        if rsp < bottom || rsp > top {
            ksprintln!(
                "[stack-check][tid={}] RSP out of range: rsp={:#x}, [{:#x}..{:#x})",
                self.tid,
                rsp,
                bottom,
                top
            );
        }
    }
}

/// Remove every run-queue reference before releasing a terminated Task.
/// Called by a *different* process after the victim has stopped running.
pub fn reap_zombie_task(ptr: *mut Task) -> bool {
    if ptr.is_null() { return false; }
    if CURRENT.lock().get() == ptr { return false; }
    unsafe {
        if (*ptr).state != TaskState::Zombie { return false; }
        RUNQ.lock().retain(|entry| entry.get() != ptr);
        drop(Box::from_raw(ptr));
    }
    true
}
