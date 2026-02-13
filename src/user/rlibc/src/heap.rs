#![allow(dead_code)]

use core::alloc::{GlobalAlloc, Layout};
use core::ptr::null_mut;
use core::sync::atomic::{AtomicUsize, Ordering};

pub struct BumpAlloc;

const HEAP_SIZE: usize = 1024 * 1024; // 1 MiB
#[link_section = ".bss.heap"]
static mut HEAP: [u8; HEAP_SIZE] = [0; HEAP_SIZE];

static NEXT: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for BumpAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let align = layout.align().max(8);
        let size = layout.size();

        let mut cur = NEXT.load(Ordering::Relaxed);
        loop {
            let aligned = (cur + (align - 1)) & !(align - 1);
            let next = aligned.saturating_add(size);
            if next > HEAP_SIZE {
                return null_mut();
            }

            match NEXT.compare_exchange(cur, next, Ordering::SeqCst, Ordering::Relaxed) {
                Ok(_) => {
                    let base = unsafe { core::ptr::addr_of_mut!(HEAP) as *mut u8 };
                    return unsafe { base.add(aligned) };
                }
                Err(v) => cur = v,
            }
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // no-op bump allocator
    }
}
