use crate::kernel::mm::map::mapper::{self, Prot};
use crate::kernel::utils::bitset::Bitset;
use core::sync::atomic::{AtomicBool, Ordering};
use spin::Mutex;
use x86_64::VirtAddr;

const PAGE_SIZE: u64 = 4096;
const MAX_PAGES: usize = 256 * 1024;
const BITWORDS: usize = (MAX_PAGES + 63) / 64;

struct Arena {
    base: u64,
    pages: usize,
    bm: Bitset<MAX_PAGES, BITWORDS>,
    inited: bool,
}

static ONCE: AtomicBool = AtomicBool::new(false);
static ARENA: Mutex<Arena> = Mutex::new(Arena {
    base: 0,
    pages: 0,
    bm: Bitset::new(),
    inited: false,
});

pub fn init(arena_base: VirtAddr, arena_size: u64) {
    if ONCE.swap(true, Ordering::SeqCst) {
        return;
    }
    let pages = (arena_size / PAGE_SIZE) as usize;

    let mut a = ARENA.lock();
    a.base = arena_base.as_u64();
    a.pages = pages;
    a.inited = true;
}

pub fn alloc() -> Option<VirtAddr> {
    alloc_n(1)
}

pub fn alloc_n(n: usize) -> Option<VirtAddr> {
    let (base, start_idx) = {
        let mut a = ARENA.lock();
        assert!(a.inited && a.pages > 0, "vmarena not initialized");
        let pages = a.pages;

        let idx = a.bm.find_run(pages, n)?;
        a.bm.set_range(idx, idx + n, pages, true);
        (a.base, idx)
    };

    let mut idx = start_idx;
    for _ in 0..n {
        let va = VirtAddr::new(base + (idx as u64) * PAGE_SIZE);
        let frame = crate::kernel::mm::phys::frame::alloc()?;
        mapper::map_fixed(va, frame, Prot::RW).ok()?;
        idx += 1;
    }

    Some(VirtAddr::new(base + (start_idx as u64) * PAGE_SIZE))
}

#[allow(unused)]
pub fn alloc_aligned(n: usize, align_pages: usize) -> Option<VirtAddr> {
    let (base, idx) = {
        let mut a = ARENA.lock();
        assert!(a.inited);
        let pages = a.pages;

        let idx = find_aligned_bitset(&a.bm, pages, n, align_pages)?;
        a.bm.set_range(idx, idx + n, pages, true);
        (a.base, idx)
    };

    let mut cur = idx;
    for _ in 0..n {
        let va = VirtAddr::new(base + (cur as u64) * PAGE_SIZE);
        let frame = crate::kernel::mm::phys::frame::alloc()?;
        mapper::map_fixed(va, frame, Prot::RW).ok()?;
        cur += 1;
    }
    Some(VirtAddr::new(base + (idx as u64) * PAGE_SIZE))
}

pub fn free(base: VirtAddr) {
    free_n(base, 1)
}

pub fn free_n(base: VirtAddr, n: usize) {
    let (arena_base, start_idx, pages) = {
        let a = ARENA.lock();
        let start = base.as_u64();
        assert!(a.inited);
        assert!(start >= a.base && (start - a.base) % PAGE_SIZE == 0);
        let idx = ((start - a.base) / PAGE_SIZE) as usize;
        assert!(idx + n <= a.pages);
        (a.base, idx, a.pages)
    };

    let mut idx = start_idx;
    for _ in 0..n {
        let va = VirtAddr::new(arena_base + (idx as u64) * PAGE_SIZE);
        if let Ok(frame) = mapper::unmap(va) {
            crate::kernel::mm::phys::frame::free(frame);
        }
        idx += 1;
    }

    let mut a = ARENA.lock();
    a.bm.set_range(start_idx, start_idx + n, pages, false);
}

pub fn is_mapped(va: VirtAddr) -> bool {
    mapper::translate(va).is_some()
}

#[allow(unused)]
pub fn used_pages() -> usize {
    let a = ARENA.lock();
    let pages = a.pages;
    let mut c = 0usize;
    for i in 0..pages {
        unsafe {
            if a.bm.get_unchecked(i) {
                c += 1;
            }
        }
    }
    c
}

#[allow(unused)]
pub fn total_pages() -> usize {
    ARENA.lock().pages
}

#[allow(unused)]
fn find_aligned_bitset(
    bm: &Bitset<MAX_PAGES, BITWORDS>,
    pages: usize,
    n: usize,
    align: usize,
) -> Option<usize> {
    let mut i = 0usize;
    while i + n <= pages {
        i = i + (align - (i % align)) % align;
        let mut ok = true;
        for k in 0..n {
            unsafe {
                if bm.get_unchecked(i + k) {
                    ok = false;
                    break;
                }
            }
        }
        if ok {
            return Some(i);
        }
        i += 1;
    }
    None
}
