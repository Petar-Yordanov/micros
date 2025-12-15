#![allow(dead_code)]

use core::ops::Add;
use spin::Once;
use x86_64::{
    structures::{gdt::Descriptor, tss::TaskStateSegment},
    VirtAddr,
};

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

pub static TSS: Once<TaskStateSegment> = Once::new();

pub unsafe fn configure_rsp0_and_ists(rsp0_top: u64) {
    let _ = TSS.call_once(|| {
        let mut tss = TaskStateSegment::new();

        tss.privilege_stack_table[0] = VirtAddr::new(rsp0_top);
        tss.interrupt_stack_table[0] = top(&IST1_DF_STACK);
        tss.interrupt_stack_table[1] = top(&IST2_NMI_STACK);
        tss.interrupt_stack_table[2] = top(&IST3_PF_STACK);

        tss.iomap_base = core::mem::size_of::<x86_64::structures::tss::TaskStateSegment>() as u16;

        tss
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
