use crate::kernel::mm::heap::freelist::Heap;
use crate::ksprintln;
use core::alloc::{GlobalAlloc, Layout};
use spin::Mutex;

struct LockedHeap(Mutex<Heap>);

unsafe impl Send for LockedHeap {}

unsafe impl GlobalAlloc for LockedHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.0.lock().alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.0.lock().dealloc(ptr, layout)
    }
}

#[global_allocator]
static GLOBAL: LockedHeap = LockedHeap(Mutex::new(Heap::new()));

#[alloc_error_handler]
fn alloc_error(_layout: Layout) -> ! {
    panic!("kernel heap OOM")
}

pub fn init(initial_pages: usize) -> Result<(), &'static str> {
    if initial_pages == 0 {
        return Err("initial_pages=0");
    }

    ksprintln!(
        "[HEAP] request {} pages (~{} MiB)",
        initial_pages,
        (initial_pages * 4096) / (1024 * 1024)
    );

    let base =
        crate::kernel::mm::virt::vmarena::alloc_n(initial_pages).ok_or("arena alloc failed")?;
    ksprintln!("[HEAP] arena gave base = {:#018x}", base.as_u64());

    let start = base.as_u64() as usize;
    let len = initial_pages * 4096;

    unsafe {
        let mut h = GLOBAL.0.lock();
        h.add_span(start, len);
    }

    ksprintln!(
        "[HEAP] seeded span @ {:#018x} len={} KiB",
        start,
        len / 1024
    );

    Ok(())
}
