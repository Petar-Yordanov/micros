use crate::arch::x86_64::interrupts::irq::{
    spurious_irq, timer_entry, virtio_blk_irq, virtio_gpu_irq, virtio_input_irq, virtio_net_irq,
    Irq,
};
use crate::arch::x86_64::interrupts::isr::{
    align, bound, breakpoint, cp_prot, debug, dev_na, df_handler, divide, gp, invalid_opcode,
    invalid_tss, mchk, nmi, overflow, pf_handler, sec, seg_np, simd, ss_fault, virt, x87,
};
use crate::arch::x86_64::interrupts::syscall_entry::syscall_entry;

use spin::Once;
use x86_64::structures::idt::InterruptDescriptorTable;
use x86_64::{PrivilegeLevel, VirtAddr};

static IDT: Once<InterruptDescriptorTable> = Once::new();

pub unsafe fn init(df_ist_index: u16) {
    let idt = IDT.call_once(|| {
        let mut idt = InterruptDescriptorTable::new();

        idt.breakpoint.set_handler_fn(breakpoint);

        idt.double_fault
            .set_handler_fn(df_handler)
            .set_stack_index(df_ist_index);

        idt.divide_error.set_handler_fn(divide);
        idt.debug.set_handler_fn(debug);

        idt.non_maskable_interrupt
            .set_handler_fn(nmi)
            .set_stack_index(df_ist_index);

        idt.overflow.set_handler_fn(overflow);
        idt.bound_range_exceeded.set_handler_fn(bound);
        idt.invalid_opcode.set_handler_fn(invalid_opcode);
        idt.device_not_available.set_handler_fn(dev_na);
        idt.invalid_tss.set_handler_fn(invalid_tss);
        idt.segment_not_present.set_handler_fn(seg_np);

        idt.stack_segment_fault.set_handler_fn(ss_fault);
        idt.general_protection_fault.set_handler_fn(gp);

        idt.page_fault.set_handler_fn(pf_handler);

        idt.x87_floating_point.set_handler_fn(x87);
        idt.alignment_check.set_handler_fn(align);
        idt.machine_check.set_handler_fn(mchk);
        idt.simd_floating_point.set_handler_fn(simd);
        idt.virtualization.set_handler_fn(virt);
        idt.security_exception.set_handler_fn(sec);
        idt.cp_protection_exception.set_handler_fn(cp_prot);

        idt[Irq::VirtioInput as u8].set_handler_fn(virtio_input_irq);
        idt[Irq::VirtioNet as u8].set_handler_fn(virtio_net_irq);
        idt[Irq::VirtioBlk as u8].set_handler_fn(virtio_blk_irq);
        idt[Irq::VirtioGpu as u8].set_handler_fn(virtio_gpu_irq);
        idt[Irq::Spurious as u8].set_handler_fn(spurious_irq);

        idt[Irq::Timer as u8].set_handler_addr(VirtAddr::new(timer_entry as *const () as u64));

        idt[0x80]
            .set_handler_addr(VirtAddr::new(syscall_entry as *const () as u64))
            .set_privilege_level(PrivilegeLevel::Ring3);

        unsafe {
            idt[0x80]
                .set_handler_addr(VirtAddr::new(syscall_entry as *const () as u64))
                .set_privilege_level(PrivilegeLevel::Ring3);
        }

        idt
    });

    idt.load();
}
