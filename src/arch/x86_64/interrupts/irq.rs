use crate::arch::x86_64::time::apic;
use crate::sprintln;
use x86_64::structures::idt::InterruptStackFrame;

#[repr(u8)]
pub enum Irq {
    Timer = 0x20,
    VirtioInput = 0x50,
    VirtioNet = 0x51,
    VirtioBlk = 0x52,
    VirtioGpu = 0x53,
    Spurious = 0xFF,
}

pub extern "x86-interrupt" fn virtio_input_irq(_sf: InterruptStackFrame) {
    unsafe {
        sprintln!("[VIRTIO] Input interrupt");
        apic::eoi();
    }
}

pub extern "x86-interrupt" fn virtio_net_irq(_sf: InterruptStackFrame) {
    unsafe {
        sprintln!("[VIRTIO] Net interrupt");
        apic::eoi();
    }
}

pub extern "x86-interrupt" fn virtio_blk_irq(_sf: InterruptStackFrame) {
    unsafe {
        sprintln!("[VIRTIO] Blk interrupt");
        apic::eoi();
    }
}

pub extern "x86-interrupt" fn virtio_gpu_irq(_sf: InterruptStackFrame) {
    unsafe {
        sprintln!("[VIRTIO] GPU interrupt");
        apic::eoi();
    }
}

pub extern "x86-interrupt" fn spurious_irq(_sf: InterruptStackFrame) {
    unsafe {
        sprintln!("Spurious interrupt");
        apic::eoi();
    }
}

pub extern "x86-interrupt" fn timer_irq(_stack: InterruptStackFrame) {
    use crate::kernel::sched::task;

    task::on_tick();

    if task::preempt_needed() {
        unsafe {
            apic::eoi();
        }
        task::schedule();
    } else {
        unsafe {
            apic::eoi();
        }
    }
}
