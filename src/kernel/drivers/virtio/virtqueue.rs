extern crate alloc;

use core::mem::size_of;
use core::ptr::{read_volatile, write_volatile};
use core::sync::atomic::{fence, Ordering::SeqCst};

use x86_64::{PhysAddr, VirtAddr};

use crate::kernel::mm::map::mapper as page;
use crate::kernel::mm::virt::vmarena;

#[repr(C, packed)]
pub struct VirtqDesc {
    pub addr: u64,
    pub len: u32,
    pub flags: u16,
    pub next: u16,
}

pub const VIRTQ_DESC_F_NEXT: u16 = 1;
pub const VIRTQ_DESC_F_WRITE: u16 = 2;

#[repr(C)]
pub struct VirtqAvail {
    pub flags: u16,
    pub idx: u16,
    pub ring: [u16; 0],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct VirtqUsedElem {
    pub id: u32,
    pub len: u32,
}

#[repr(C)]
pub struct VirtqUsed {
    pub flags: u16,
    pub idx: u16,
    pub ring: [VirtqUsedElem; 0],
}

pub struct QueueMem {
    pub va: VirtAddr,
    pub pa: PhysAddr,
    pub _size: usize,
}

impl QueueMem {
    pub fn alloc_pages(n_pages: usize) -> Self {
        let va_base = vmarena::alloc_n(n_pages).expect("vq vmarena");
        let first_pa = page::translate(va_base).expect("vq map");
        Self {
            va: va_base,
            pa: first_pa,
            _size: n_pages * 4096,
        }
    }
}

pub struct VirtQueue {
    pub qsz: u16,
    pub desc: *mut VirtqDesc,
    pub avail: *mut VirtqAvail,
    pub used: *mut VirtqUsed,

    next_free: u16,
    last_used_idx: u16,

    #[allow(dead_code)]
    mem: QueueMem,

    notify_base: *mut u8,
    notify_mul: u32,
    notify_off: u16,
}

unsafe impl Send for VirtQueue {}

impl VirtQueue {
    pub fn new(
        common: *mut super::pci::VirtioPciCommonCfg,
        qsel: u16,
        qsz: u16,
        notify_base: *mut u8,
        notify_mul: u32,
    ) -> Self {
        unsafe {
            (*common).queue_select = qsel;

            let desc_sz = (size_of::<VirtqDesc>() * qsz as usize + 15) & !15;
            let avail_sz = (size_of::<VirtqAvail>() + 2 * qsz as usize + 2 + 3) & !3;
            let used_sz = (size_of::<VirtqUsed>() + 8 * qsz as usize + 3) & !3;

            let total = desc_sz + avail_sz + used_sz + 4096;
            let pages = (total + 4095) / 4096;

            let slab = QueueMem::alloc_pages(pages);
            let slab_va = slab.va.as_u64();

            let desc_va = slab_va;
            let avail_va = desc_va + desc_sz as u64;
            let used_va = (avail_va + avail_sz as u64 + 4095) & !4095u64;

            let desc_pa = slab.pa.as_u64();
            let avail_pa = desc_pa + (avail_va - desc_va);
            let used_pa = desc_pa + (used_va - desc_va);

            (*common).queue_size = qsz;
            (*common).queue_desc = desc_pa;
            (*common).queue_driver = avail_pa;
            (*common).queue_device = used_pa;

            (*common).queue_enable = 1;
            fence(SeqCst);

            let notify_off = (*common).queue_notify_off;

            Self {
                qsz,
                desc: desc_va as *mut VirtqDesc,
                avail: avail_va as *mut VirtqAvail,
                used: used_va as *mut VirtqUsed,
                next_free: 0,
                last_used_idx: 0,
                mem: slab,
                notify_base,
                notify_mul,
                notify_off,
            }
        }
    }

    pub fn alloc_desc(&mut self) -> u16 {
        let i = self.next_free;
        self.next_free = (self.next_free + 1) % self.qsz;
        i
    }

    pub fn push(&mut self, head: u16) {
        unsafe {
            let a = &mut *self.avail;
            let idx = (*a).idx;

            let ring_ptr = (self.avail as *mut u8).add(size_of::<VirtqAvail>()) as *mut u16;
            let slot = ring_ptr.add((idx as usize) % (self.qsz as usize));

            write_volatile(slot, head);
            fence(SeqCst);
            (*a).idx = idx.wrapping_add(1);
        }
    }

    pub fn pop_used(&mut self) -> Option<VirtqUsedElem> {
        unsafe {
            let u = &mut *self.used;
            if read_volatile(&u.idx) == self.last_used_idx {
                return None;
            }

            let ring_bytes = (self.used as *mut u8).add(size_of::<VirtqUsed>());
            let idx = (self.last_used_idx as usize) % (self.qsz as usize);
            let slot_ptr = ring_bytes.add(idx * size_of::<VirtqUsedElem>()) as *const VirtqUsedElem;

            let elem = core::ptr::read_unaligned(slot_ptr);
            self.last_used_idx = self.last_used_idx.wrapping_add(1);
            Some(elem)
        }
    }

    pub fn notify(&mut self, qsel: u16) {
        unsafe {
            let off = (self.notify_off as u32).wrapping_mul(self.notify_mul) as usize;
            let ptr = (self.notify_base as *mut u8).add(off) as *mut u16;
            write_volatile(ptr, qsel);
            fence(SeqCst);
        }
    }
}
