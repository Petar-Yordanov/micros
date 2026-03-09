extern crate alloc;

use alloc::collections::BTreeMap;
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
const USER_SHM_STRIDE: u64 = 0x0100_0000;

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
            None => return -errno::ENOSPC,
        };
        frames.push(fr);
    }

    let id = NEXT_SHM_ID.fetch_add(1, Ordering::Relaxed);
    SHMS.lock().insert(
        id,
        ShmObj {
            size: size_aligned,
            frames,
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

    let obj = {
        let m = SHMS.lock();
        match m.get(&args.shm_id) {
            Some(o) => o.clone(),
            None => return -errno::ESRCH,
        }
    };

    let base_va = if args.desired_va != 0 {
        let va = align_down(args.desired_va, PAGE_SIZE);
        if va == 0 || va >= 0x0000_8000_0000_0000 {
            return -errno::EINVAL;
        }
        va
    } else {
        USER_SHM_BASE + (args.shm_id.saturating_mul(USER_SHM_STRIDE))
    };

    if obj.size > USER_SHM_STRIDE {
        return -errno::EINVAL;
    }

    let prot = Prot::UserRW;
    let pages = (obj.size / PAGE_SIZE) as usize;

    for i in 0..pages {
        let va = VirtAddr::new(base_va + (i as u64) * PAGE_SIZE);
        if page::map_fixed(va, obj.frames[i], prot).is_err() {
            return -errno::EIO;
        }
    }

    base_va as i64
}
