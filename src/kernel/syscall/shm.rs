extern crate alloc;

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};
use spin::Mutex;

use x86_64::structures::paging::{PhysFrame, Size4KiB};
use x86_64::VirtAddr;

use micros_abi::errno;
use micros_abi::types::{ShmCreateArgs, ShmMapArgs};

use crate::kernel::mm::map::mapper as page;
use crate::kernel::mm::map::mapper::Prot;
use crate::kernel::mm::phys::frame;

use super::util::copy_user_struct;

const PAGE_SIZE: u64 = 4096;
const USER_SHM_BASE: u64 = 0x0000_5000_0000_0000;
const USER_SHM_STRIDE: u64 = 0x0400_0000; // 64 MiB per object

#[inline(always)]
fn align_up(x: u64, a: u64) -> u64 {
    (x + (a - 1)) & !(a - 1)
}

#[inline(always)]
fn align_down(x: u64, a: u64) -> u64 {
    x & !(a - 1)
}

#[derive(Clone)]
struct ShmObj {
    size: u64,
    frames: Vec<PhysFrame<Size4KiB>>,
    owner: u64,
    owner_alive: bool,
    // Each process has at most one mapping for a given SHM object.
    mappings: BTreeMap<u64, u64>,
    // The next page to unmap for a process. Bounded teardown prevents long
    // latency spikes on the compositor's input thread.
    unmap_progress: BTreeMap<u64, usize>,
}

static NEXT_SHM_ID: AtomicU64 = AtomicU64::new(1);
static SHMS: Mutex<BTreeMap<u64, ShmObj>> = Mutex::new(BTreeMap::new());

pub(super) fn sys_shm_create(args_ptr: u64) -> i64 {
    let args: ShmCreateArgs = match copy_user_struct(args_ptr) {
        Ok(v) => v,
        Err(e) => return e,
    };

    let size = args.size;
    if size == 0 {
        return -errno::EINVAL;
    }

    const MAX_SIZE: u64 = 64 * 1024 * 1024;
    if size > MAX_SIZE {
        return -errno::EINVAL;
    }

    let size_aligned = align_up(size, PAGE_SIZE);
    let pages = (size_aligned / PAGE_SIZE) as usize;

    let mut frames = Vec::<PhysFrame<Size4KiB>>::with_capacity(pages);

    for _ in 0..pages {
        let fr = match frame::alloc() {
            Some(f) => f,
            None => { for old in frames { frame::free(old); } return -errno::ENOSPC; },
        };
        // Zero physical memory before exposing it to untrusted userspace.
        let phys_offset = crate::platform::limine::hhdm::HHDM_REQ.get_response().unwrap().offset();
        unsafe { core::ptr::write_bytes((phys_offset + fr.start_address().as_u64()) as *mut u8, 0, PAGE_SIZE as usize); }
        frames.push(fr);
    }

    let id = NEXT_SHM_ID.fetch_add(1, Ordering::Relaxed);
    SHMS.lock().insert(
        id,
        ShmObj {
            size: size_aligned,
            frames,
            owner: crate::kernel::sched::proc::current_pid().unwrap_or(0),
            owner_alive: true,
            mappings: BTreeMap::new(),
            unmap_progress: BTreeMap::new(),
        },
    );

    id as i64
}

pub(super) fn sys_shm_map(args_ptr: u64) -> i64 {
    let args: ShmMapArgs = match copy_user_struct(args_ptr) {
        Ok(v) => v,
        Err(e) => return e,
    };

    if args.shm_id == 0 {
        return -errno::EINVAL;
    }

    let caller = crate::kernel::sched::proc::current_pid().unwrap_or(0);
    let mut objects = SHMS.lock();
    let Some(obj) = objects.get_mut(&args.shm_id) else { return -errno::ESRCH };
    if obj.unmap_progress.contains_key(&caller) { return -errno::EAGAIN; }
    if let Some(&old_va) = obj.mappings.get(&caller) { return old_va as i64; }

    let base_va = if args.desired_va != 0 {
        let va = align_down(args.desired_va, PAGE_SIZE);
        if va == 0 || va >= 0x0000_8000_0000_0000 {
            return -errno::EINVAL;
        }
        va
    } else {
        match args.shm_id.checked_mul(USER_SHM_STRIDE).and_then(|n| USER_SHM_BASE.checked_add(n)) {
            Some(va) if va < 0x0000_7000_0000_0000 => va,
            _ => return -errno::EINVAL,
        }
    };

    if obj.size > USER_SHM_STRIDE ||
        base_va.checked_add(obj.size).map_or(true, |end| end > 0x0000_7000_0000_0000) {
        return -errno::EINVAL;
    }

    let prot = Prot::UserRW;
    let pages = (obj.size / PAGE_SIZE) as usize;

    for i in 0..pages {
        let va = VirtAddr::new(base_va + (i as u64) * PAGE_SIZE);
        if page::map_fixed(va, obj.frames[i], prot).is_err() {
            // Roll back partially completed mappings: never leave an
            // untracked mapping to a SHM frame behind on error.
            for j in 0..i {
                let _ = page::unmap(VirtAddr::new(base_va + (j as u64) * PAGE_SIZE));
            }
            return -errno::EIO;
        }
    }

    obj.mappings.insert(caller, base_va);
    base_va as i64
}

/// Unmap at most 32 pages in each syscall. A partial unmap returns EAGAIN;
/// callers must retry before reusing this mapping or declaring it released.
/// No pages are freed until *all* mappings have been removed and the owner
/// has exited. This also prevents the WM from blocking for an entire surface.
const UNMAP_PAGE_BUDGET: usize = 32;
pub(super) fn sys_shm_unmap(id: u64) -> i64 {
    let caller = crate::kernel::sched::proc::current_pid().unwrap_or(0);
    let mut objects = SHMS.lock();
    let Some(obj) = objects.get_mut(&id) else { return -errno::ESRCH };
    let Some(&base) = obj.mappings.get(&caller) else { return -errno::EINVAL };

    let start = obj.unmap_progress.get(&caller).copied().unwrap_or(0);
    let total = (obj.size / PAGE_SIZE) as usize;
    let end = start.saturating_add(UNMAP_PAGE_BUDGET).min(total);
    for i in start..end {
        let va = VirtAddr::new(base + (i as u64) * PAGE_SIZE);
        let expected = obj.frames[i].start_address().as_u64();
        if page::translate(va).map(|a| a.as_u64()) != Some(expected) {
            obj.unmap_progress.insert(caller, i);
            return -errno::EIO;
        }
        if page::unmap(va).is_err() {
            obj.unmap_progress.insert(caller, i);
            return -errno::EIO;
        }
    }
    if end != total {
        obj.unmap_progress.insert(caller, end);
        return -errno::EAGAIN;
    }

    obj.unmap_progress.remove(&caller);
    obj.mappings.remove(&caller);
    if !obj.owner_alive && obj.mappings.is_empty() {
        if let Some(dead) = objects.remove(&id) {
            frame::free_many(&dead.frames);
        }
    }
    0
}

/// Collect all shared data frames before tearing down a dead address space.
/// Its private page tables may reference these frames but must not free them.
pub(super) fn shared_frames_snapshot() -> BTreeSet<u64> {
    let objects = SHMS.lock();
    let mut result = BTreeSet::new();
    for obj in objects.values() {
        for fr in &obj.frames { result.insert(fr.start_address().as_u64()); }
    }
    result
}

/// The victim's address space has already been destroyed and all its leaf
/// page mappings are gone. Only now may its registry references be removed.
pub(super) fn release_process(pid: u64) {
    let mut objects = SHMS.lock();
    let mut dead = Vec::new();
    for (&id, obj) in objects.iter_mut() {
        obj.mappings.remove(&pid);
        obj.unmap_progress.remove(&pid);
        if obj.owner == pid { obj.owner_alive = false; }
        if !obj.owner_alive && obj.mappings.is_empty() { dead.push(id); }
    }
    for id in dead {
        if let Some(obj) = objects.remove(&id) {
            frame::free_many(&obj.frames);
        }
    }
}

