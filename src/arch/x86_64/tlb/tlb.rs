use x86_64::VirtAddr;

pub fn flush_all() {
    x86_64::instructions::tlb::flush_all();
}

pub fn flush_va(va: VirtAddr) {
    x86_64::instructions::tlb::flush(va);
}
