#![allow(dead_code)]

use crate::kernel::utils::align::{align_down, align_up};
use crate::kernel::utils::bitset::Bitset;
use crate::platform::limine::memory_map::response as memory_map_response;
use limine::{
    memory_map::{Entry, EntryType},
    request::MemoryMapRequest,
    response::MemoryMapResponse,
};
use spin::{Mutex, Once};
use x86_64::{
    structures::paging::{PhysFrame, Size4KiB},
    PhysAddr,
};

const FRAME_SIZE: u64 = 4096;
const BITMAP_WORDS: usize = 131_072;
const MAX_BITS: usize = BITMAP_WORDS * 64;

struct FramePool {
    base: u64,
    tracked: usize,
    bm: Bitset<MAX_BITS, BITMAP_WORDS>,
}

impl FramePool {
    const fn new() -> Self {
        Self {
            base: 0,
            tracked: 0,
            bm: Bitset::new(),
        }
    }
    #[inline]
    fn phys_of_idx(&self, idx: usize) -> u64 {
        self.base + (idx as u64) * FRAME_SIZE
    }
    #[inline]
    fn idx_of_phys(&self, pa: u64) -> Option<usize> {
        if pa < self.base {
            return None;
        }
        let off = pa - self.base;
        if off % FRAME_SIZE != 0 {
            return None;
        }
        let idx = (off / FRAME_SIZE) as usize;
        (idx < self.tracked).then_some(idx)
    }
}

static INIT: Once = Once::new();
static MEMMAP_REQ: MemoryMapRequest = MemoryMapRequest::new();
static POOL: Mutex<FramePool> = Mutex::new(FramePool::new());

pub fn init() {
    INIT.call_once(init_inner);
}
pub fn alloc() -> Option<PhysFrame<Size4KiB>> {
    try_alloc()
}
pub fn free(frame: PhysFrame<Size4KiB>) {
    free_inner(frame);
}

fn init_inner() {
    let resp: &MemoryMapResponse = memory_map_response();
    let entries: &[&Entry] = resp.entries();

    let mut min_base: u64 = u64::MAX;
    let mut max_end: u64 = 0;

    for &e in entries {
        let base = e.base;
        let end = base.saturating_add(e.length);
        if base < min_base {
            min_base = base;
        }

        if end > max_end {
            max_end = end;
        }
    }

    let min_base = align_down(min_base, FRAME_SIZE);
    let max_end = align_up(max_end, FRAME_SIZE);
    let total_frames = ((max_end - min_base) / FRAME_SIZE) as usize;

    let mut p = POOL.lock();
    p.base = min_base;
    p.tracked = total_frames.min(MAX_BITS);

    let base = p.base;
    let tracked = p.tracked;

    p.bm.fill_tracked(tracked, true);

    for &e in entries {
        if e.entry_type == EntryType::USABLE {
            let s = align_up(e.base, FRAME_SIZE);
            let eend = align_down(e.base.saturating_add(e.length), FRAME_SIZE);
            if eend <= s {
                continue;
            }

            let i0 = ((s - base) / FRAME_SIZE) as usize;
            let mut i1 = ((eend - base) / FRAME_SIZE) as usize;
            if i0 >= tracked {
                continue;
            }

            if i1 > tracked {
                i1 = tracked;
            }

            p.bm.set_range(i0, i1, tracked, false);
        }
    }
}

fn try_alloc() -> Option<PhysFrame<Size4KiB>> {
    let mut p = POOL.lock();
    let tracked = p.tracked;
    if tracked == 0 {
        return None;
    }

    if let Some(idx) = p.bm.first_free(tracked) {
        unsafe {
            p.bm.set_unchecked(idx, true);
        }
        let pa = p.phys_of_idx(idx);
        return Some(PhysFrame::<Size4KiB>::containing_address(PhysAddr::new(pa)));
    }
    None
}

fn free_inner(frame: PhysFrame<Size4KiB>) {
    let mut p = POOL.lock();
    if let Some(idx) = p.idx_of_phys(frame.start_address().as_u64()) {
        unsafe {
            p.bm.set_unchecked(idx, false);
        }
    }
}
