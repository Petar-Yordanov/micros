#![allow(dead_code)]

use core::cell::UnsafeCell;
use core::ops::Add;

use spin::Once;
use x86_64::structures::{gdt::Descriptor, tss::TaskStateSegment};
use x86_64::VirtAddr;

pub struct SyncUnsafeCell<T>(UnsafeCell<T>);
unsafe impl<T> Sync for SyncUnsafeCell<T> {}

const STACK_SIZE: usize = 16 * 4096;

#[repr(align(16))]
struct Aligned([u8; STACK_SIZE]);

static KERNEL_RSP0_STACK: Aligned = Aligned([0; STACK_SIZE]);
static IST1_DF_STACK: Aligned = Aligned([0; STACK_SIZE]);
static IST2_NMI_STACK: Aligned = Aligned([0; STACK_SIZE]);
static IST3_PF_STACK: Aligned = Aligned([0; STACK_SIZE]);

#[inline(always)]
unsafe fn top(s: &Aligned) -> VirtAddr {
    VirtAddr::from_ptr(s.0.as_ptr()).add(STACK_SIZE as u64)
}

pub static TSS: Once<SyncUnsafeCell<TaskStateSegment>> = Once::new();

pub unsafe fn configure_rsp0_and_ists(rsp0_top: u64) {
    let _ = TSS.call_once(|| {
        let mut tss = TaskStateSegment::new();
        tss.privilege_stack_table[0] = VirtAddr::new(rsp0_top);
        tss.interrupt_stack_table[0] = top(&IST1_DF_STACK);
        tss.interrupt_stack_table[1] = top(&IST2_NMI_STACK);
        tss.interrupt_stack_table[2] = top(&IST3_PF_STACK);
        tss.iomap_base = core::mem::size_of::<TaskStateSegment>() as u16;
        SyncUnsafeCell(UnsafeCell::new(tss))
    });
}

pub unsafe fn bootstrap_rsp0_top() -> u64 {
    top(&KERNEL_RSP0_STACK).as_u64()
}

pub fn descriptor_for(ptr: *const TaskStateSegment) -> (u64, u64) {
    let desc = unsafe { Descriptor::tss_segment_unchecked(ptr) };
    match desc {
        Descriptor::SystemSegment(low, high) => (low, high),
        Descriptor::UserSegment(_) => unreachable!("tss_segment must be SystemSegment"),
    }
}

static mut LAST_RSP0: u64 = 0;

pub fn set_rsp0_top(rsp0_top: u64) {
    let cell = TSS.get().expect("TSS not initialized");
    unsafe {
        if LAST_RSP0 != rsp0_top {
            LAST_RSP0 = rsp0_top;
            //crate::ksprintln!("[tss] rsp0 - {:#x}", rsp0_top);
        }
        (*cell.0.get()).privilege_stack_table[0] = VirtAddr::new(rsp0_top);
    }
}

pub fn set_rsp0(top: VirtAddr) {
    let cell = TSS.get().expect("TSS not initialized");
    unsafe {
        (*cell.0.get()).privilege_stack_table[0] = top;
    }
}

pub fn tss_ref() -> &'static TaskStateSegment {
    let cell = TSS.get().expect("TSS not initialized");
    unsafe { &*cell.0.get() }
}
