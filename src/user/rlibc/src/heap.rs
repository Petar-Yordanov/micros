#![allow(dead_code)]

use core::alloc::{GlobalAlloc, Layout};
use core::ptr::null_mut;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use micros_abi::sysnr;

use crate::syscall::syscall1;

pub struct BumpAlloc;

// Dedicated per-process address range, independently mapped by the kernel.
// Unlike the previous 16 MiB .bss array, unused heap consumes no frames and
// ELF startup does not zero 4096 pages for every application launch.
const HEAP_BASE: usize = 0x0000_3000_0000_0000;
const HEAP_MAX: usize = 64 * 1024 * 1024;
const COMMIT_CHUNK: usize = 64 * 1024;

static NEXT: AtomicUsize = AtomicUsize::new(0);
static COMMITTED: AtomicUsize = AtomicUsize::new(0);
static GROWING: AtomicBool = AtomicBool::new(false);

fn commit(required: usize) -> bool {
    if required <= COMMITTED.load(Ordering::Acquire) {
        return true;
    }
    // Only one thread may increase the backing pages. The kernel maps memory
    // in 4 KiB pages, while userspace grows in 64 KiB batches to reduce traps.
    while GROWING.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
        core::hint::spin_loop();
    }
    let ok = if required <= COMMITTED.load(Ordering::Acquire) {
        true
    } else {
        let want = match required.checked_add(COMMIT_CHUNK - 1) {
            Some(n) => (n / COMMIT_CHUNK) * COMMIT_CHUNK,
            None => usize::MAX,
        };
        if want > HEAP_MAX {
            false
        } else {
            let n = syscall1(sysnr::SYS_HEAP_COMMIT, want as u64);
            if n >= 0 && (n as usize) >= want {
                COMMITTED.store(n as usize, Ordering::Release);
                true
            } else {
                false
            }
        }
    };
    GROWING.store(false, Ordering::Release);
    ok
}

unsafe impl GlobalAlloc for BumpAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let align = layout.align().max(8);
        let size = layout.size().max(1);
        let mut cur = NEXT.load(Ordering::Relaxed);
        loop {
            let Some(aligned) = cur.checked_add(align - 1).map(|n| n & !(align - 1)) else {
                return null_mut();
            };
            let Some(next) = aligned.checked_add(size) else { return null_mut(); };
            if next > HEAP_MAX || !commit(next) { return null_mut(); }
            match NEXT.compare_exchange_weak(cur, next, Ordering::AcqRel, Ordering::Relaxed) {
                Ok(_) => return (HEAP_BASE + aligned) as *mut u8,
                Err(v) => cur = v,
            }
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // Preserves the original bump allocator semantics. A future allocator
        // should recycle freed blocks; this patch does not add that behavior.
    }
}
