extern crate alloc;

use core::alloc::Layout;
use core::mem::size_of;
use core::ptr::null_mut;

const PAGE_SIZE: usize = 4096;
const ALIGN: usize = 16;

#[inline(always)]
fn align_up(x: usize, align: usize) -> usize {
    (x + align - 1) & !(align - 1)
}

const fn const_align_up(x: usize, align: usize) -> usize {
    (x + align - 1) & !(align - 1)
}

#[repr(C)]
struct Block {
    size: usize,
    prev: *mut Block,
    next: *mut Block,
    free: bool,
}

const HEADER_SIZE: usize = size_of::<Block>();
const FOOTER_SIZE: usize = size_of::<usize>();
const MIN_PAYLOAD: usize = 16;
const MIN_BLOCK: usize = const_align_up(HEADER_SIZE + FOOTER_SIZE + MIN_PAYLOAD, ALIGN);

impl Block {
    #[inline(always)]
    fn as_ptr(&self) -> *mut u8 {
        self as *const _ as *mut u8
    }

    #[inline(always)]
    fn payload_ptr(&self) -> *mut u8 {
        unsafe { self.as_ptr().add(HEADER_SIZE) }
    }

    #[inline(always)]
    fn footer_ptr(&self) -> *mut usize {
        unsafe { (self.as_ptr() as *mut u8).add(self.size - FOOTER_SIZE) as *mut usize }
    }

    #[inline(always)]
    fn set_footer(&mut self) {
        unsafe {
            *self.footer_ptr() = self.size;
        }
    }

    #[inline(always)]
    fn from_payload(ptr: *mut u8) -> *mut Block {
        unsafe { (ptr as *mut u8).sub(HEADER_SIZE) as *mut Block }
    }

    #[inline(always)]
    fn next_phys(&self) -> *mut Block {
        unsafe { (self.as_ptr() as *mut u8).add(self.size) as *mut Block }
    }
}

struct Region {
    start: usize,
    len: usize,
}

struct List {
    head: *mut Block,
}

impl List {
    const fn new() -> Self {
        Self {
            head: core::ptr::null_mut(),
        }
    }

    unsafe fn push(&mut self, b: *mut Block) {
        (*b).free = true;
        (*b).prev = null_mut();
        (*b).next = self.head;
        if !self.head.is_null() {
            (*self.head).prev = b;
        }
        self.head = b;
    }

    unsafe fn remove(&mut self, b: *mut Block) {
        (*b).free = false;
        let prev = (*b).prev;
        let next = (*b).next;
        if !prev.is_null() {
            (*prev).next = next;
        }
        if !next.is_null() {
            (*next).prev = prev;
        }
        if self.head == b {
            self.head = next;
        }
        (*b).prev = null_mut();
        (*b).next = null_mut();
    }

    unsafe fn find_fit(&self, need: usize, align: usize) -> *mut Block {
        let mut cur = self.head;
        while !cur.is_null() {
            let payload = (*cur).payload_ptr() as usize;
            let aligned_payload = align_up(payload, align.max(ALIGN));
            let lead = aligned_payload - payload;
            if (*cur).size >= need + lead && fits_with_optional_lead((*cur).size, need, lead) {
                return cur;
            }
            cur = (*cur).next;
        }
        null_mut()
    }
}

#[inline(always)]
fn fits_with_optional_lead(block_size: usize, need: usize, lead: usize) -> bool {
    if lead == 0 {
        block_size >= need
    } else {
        block_size >= lead + need && lead >= MIN_BLOCK
    }
}

pub struct Heap {
    free: List,
    regions: heapless::Vec<Region, 32>,
}

impl Heap {
    pub const fn new() -> Self {
        Self {
            free: List::new(),
            regions: heapless::Vec::new(),
        }
    }

    fn region_bounds(&self, addr: usize) -> Option<(usize, usize)> {
        for r in self.regions.iter() {
            if addr >= r.start && addr < r.start + r.len {
                return Some((r.start, r.start + r.len));
            }
        }
        None
    }

    pub unsafe fn add_span(&mut self, start: usize, len: usize) {
        if len < MIN_BLOCK {
            return;
        }
        let b = start as *mut Block;
        (*b).size = len;
        (*b).free = true;
        (*b).prev = null_mut();
        (*b).next = null_mut();
        (*b).set_footer();
        self.free.push(b);
        let _ = self.regions.push(Region { start, len });
    }

    unsafe fn grow(&mut self, min_bytes: usize) -> bool {
        let pages = align_up(min_bytes, PAGE_SIZE) / PAGE_SIZE;
        if let Some(base_va) = crate::kernel::mm::virt::vmarena::alloc_n(pages) {
            let start = base_va.as_u64() as usize;
            let len = pages * PAGE_SIZE;
            self.add_span(start, len);
            true
        } else {
            false
        }
    }

    pub unsafe fn alloc(&mut self, layout: Layout) -> *mut u8 {
        let payload_align = layout.align().max(ALIGN);
        let payload_size = align_up(layout.size().max(MIN_PAYLOAD), ALIGN);
        let need = HEADER_SIZE + payload_size + FOOTER_SIZE;

        let mut blk = self.free.find_fit(need, payload_align);
        if blk.is_null() {
            if !self.grow(need) {
                return null_mut();
            }
            blk = self.free.find_fit(need, payload_align);
            if blk.is_null() {
                return null_mut();
            }
        }

        self.free.remove(blk);

        let cur_payload = (*blk).payload_ptr() as usize;
        let aligned_payload = align_up(cur_payload, payload_align);
        let lead = aligned_payload - cur_payload;

        let mut use_ptr = blk as *mut u8;
        let mut use_size = (*blk).size;

        let lead_aligned = lead & !(ALIGN - 1);
        let absorbed = if lead_aligned >= MIN_BLOCK {
            let lead_block = blk;
            (*lead_block).size = lead_aligned;
            (*lead_block).set_footer();
            self.free.push(lead_block);

            let rest = (lead_block as *mut u8).add(lead_aligned) as *mut Block;
            (*rest).size = use_size - lead_aligned;
            (*rest).free = false;
            (*rest).prev = null_mut();
            (*rest).next = null_mut();
            (*rest).set_footer();

            use_ptr = rest as *mut u8;
            use_size = (*rest).size;
            0
        } else {
            lead
        };

        let mut alloc_size = HEADER_SIZE + payload_size + FOOTER_SIZE + absorbed;
        alloc_size = align_up(alloc_size, ALIGN);

        if use_size >= alloc_size + MIN_BLOCK {
            let tail = (use_ptr as usize + alloc_size) as *mut Block;
            (*tail).size = use_size - alloc_size;
            (*tail).free = true;
            (*tail).prev = null_mut();
            (*tail).next = null_mut();
            (*tail).set_footer();
            self.free.push(tail);

            let use_blk = use_ptr as *mut Block;
            (*use_blk).size = alloc_size;
            (*use_blk).free = false;
            (*use_blk).set_footer();
        } else {
            let use_blk = use_ptr as *mut Block;
            (*use_blk).free = false;
            (*use_blk).set_footer();
        }

        (use_ptr as *mut Block).as_ref().unwrap().payload_ptr()
    }

    unsafe fn try_coalesce_with_next(&mut self, b: *mut Block) {
        let addr = b as usize;
        let Some((_start, end)) = self.region_bounds(addr) else {
            return;
        };

        let next = (*b).next_phys();
        let next_addr = next as usize;
        if next_addr == 0 || next_addr >= end {
            return;
        }

        if (*next).free {
            self.free.remove(next);
            (*b).size += (*next).size;
            (*b).set_footer();
        }
    }

    unsafe fn try_coalesce_with_prev(&mut self, mut b: *mut Block) -> *mut Block {
        let addr = b as usize;
        let Some((start, _end)) = self.region_bounds(addr) else {
            return b;
        };

        if addr < start + FOOTER_SIZE {
            return b;
        }

        let footer = (addr - FOOTER_SIZE) as *mut usize;
        let prev_sz = *footer;
        if prev_sz == 0 {
            return b;
        }

        let prev_addr = addr.saturating_sub(prev_sz);
        if prev_addr < start {
            return b;
        }

        let prev = prev_addr as *mut Block;
        if (*prev).free {
            self.free.remove(prev);
            (*prev).size += (*b).size;
            (*prev).set_footer();
            b = prev;
        }
        b
    }

    pub unsafe fn dealloc(&mut self, ptr: *mut u8, _layout: Layout) {
        if ptr.is_null() {
            return;
        }
        let mut b = Block::from_payload(ptr);
        (*b).free = true;
        (*b).prev = null_mut();
        (*b).next = null_mut();
        self.free.push(b);

        b = self.try_coalesce_with_prev(b);
        self.try_coalesce_with_next(b);
    }
}

unsafe impl Send for Heap {}
