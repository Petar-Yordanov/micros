use crate::kernel::mm::map::mapper as page;
use crate::kernel::mm::phys::frame;
use crate::kernel::mm::virt::vmarena;
use x86_64::VirtAddr;

pub fn alloc_kstack_top(pages: usize) -> VirtAddr {
    assert!(pages >= 1);
    let total = pages + 1;
    let base = vmarena::alloc_n(total).expect("kstack vmarena alloc");

    if let Ok(pf) = page::unmap(base) {
        frame::free(pf);
    }

    base + ((total as u64) * 4096u64)
}

#[allow(unused)]
pub fn free_kstack_top(top: VirtAddr, pages: usize) {
    let total = pages + 1;
    let base = top - ((total as u64) * 4096u64);

    for i in 1..total {
        let va = base + ((i as u64) * 4096u64);
        if let Ok(pf) = page::unmap(va) {
            frame::free(pf);
        }
    }

    vmarena::free_n(base, total);
}
