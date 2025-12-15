use crate::sprintln;
use x86_64::instructions::hlt;
use x86_64::registers::control::Cr2;
use x86_64::structures::idt::{InterruptStackFrame, PageFaultErrorCode};

pub extern "x86-interrupt" fn breakpoint(sf: InterruptStackFrame) {
    use core::ptr::read_volatile;

    let rip = sf.instruction_pointer.as_u64();
    let cs = sf.code_segment.0 as u64;
    let rfl = sf.cpu_flags;
    unsafe {
        let p = rip as *const u8;
        let b0 = read_volatile(p.add(0));
        let b1 = read_volatile(p.add(1));
        let b2 = read_volatile(p.add(2));
        let b3 = read_volatile(p.add(3));
        let b4 = read_volatile(p.add(4));
        let b5 = read_volatile(p.add(5));
        let b6 = read_volatile(p.add(6));
        let b7 = read_volatile(p.add(7));
        sprintln!("[EXC] breakpoint @ RIP={:#x} CS={:#x} RFLAGS={:?} bytes={:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x}",
            rip, cs, rfl, b0,b1,b2,b3,b4,b5,b6,b7);
    }
}

pub extern "x86-interrupt" fn df_handler(_s: InterruptStackFrame, _c: u64) -> ! {
    sprintln!("[EXC] df");
    loop {
        x86_64::instructions::hlt();
    }
}

pub extern "x86-interrupt" fn mchk(_s: InterruptStackFrame) -> ! {
    sprintln!("[EXC] machine_check");
    loop {
        x86_64::instructions::hlt();
    }
}

pub extern "x86-interrupt" fn divide(_s: InterruptStackFrame) {
    sprintln!("[EXC] divide");
    loop {
        x86_64::instructions::hlt();
    }
}

pub extern "x86-interrupt" fn debug(_s: InterruptStackFrame) {
    sprintln!("[EXC] debug");
    loop {
        x86_64::instructions::hlt();
    }
}

pub extern "x86-interrupt" fn nmi(_s: InterruptStackFrame) {
    sprintln!("[EXC] nmi");
    loop {
        x86_64::instructions::hlt();
    }
}

pub extern "x86-interrupt" fn overflow(_s: InterruptStackFrame) {
    sprintln!("[EXC] overflow");
    loop {
        x86_64::instructions::hlt();
    }
}

pub extern "x86-interrupt" fn bound(_s: InterruptStackFrame) {
    sprintln!("[EXC] bound");
    loop {
        x86_64::instructions::hlt();
    }
}

pub extern "x86-interrupt" fn invalid_opcode(_s: InterruptStackFrame) {
    sprintln!("[EXC] invalid_opcode");
    loop {
        hlt();
    }
}

pub extern "x86-interrupt" fn dev_na(_s: InterruptStackFrame) {
    sprintln!("[EXC] dev_na");
    loop {
        x86_64::instructions::hlt();
    }
}

pub extern "x86-interrupt" fn invalid_tss(_s: InterruptStackFrame, _c: u64) {
    sprintln!("[EXC] invalid_tss");
    loop {
        x86_64::instructions::hlt();
    }
}

pub extern "x86-interrupt" fn seg_np(_s: InterruptStackFrame, _c: u64) {
    sprintln!("[EXC] seg_np");
    loop {
        x86_64::instructions::hlt();
    }
}

pub extern "x86-interrupt" fn ss_fault(_s: InterruptStackFrame, _c: u64) {
    sprintln!("[EXC] ss_fault");
    loop {
        x86_64::instructions::hlt();
    }
}

pub extern "x86-interrupt" fn gp(_s: InterruptStackFrame, _c: u64) {
    sprintln!("[EXC] gp");
    loop {
        x86_64::instructions::hlt();
    }
}

pub extern "x86-interrupt" fn x87(_s: InterruptStackFrame) {
    sprintln!("[EXC] x87");
    loop {
        x86_64::instructions::hlt();
    }
}

pub extern "x86-interrupt" fn align(_s: InterruptStackFrame, _c: u64) {
    sprintln!("[EXC] align");
    loop {
        x86_64::instructions::hlt();
    }
}

pub extern "x86-interrupt" fn simd(_s: InterruptStackFrame) {
    sprintln!("[EXC] simd");
    loop {
        x86_64::instructions::hlt();
    }
}

pub extern "x86-interrupt" fn virt(_s: InterruptStackFrame) {
    sprintln!("[EXC] virt");
    loop {
        x86_64::instructions::hlt();
    }
}

pub extern "x86-interrupt" fn sec(_s: InterruptStackFrame, _c: u64) {
    sprintln!("[EXC] sec");
    loop {
        x86_64::instructions::hlt();
    }
}

pub extern "x86-interrupt" fn cp_prot(_s: InterruptStackFrame, _c: u64) {
    sprintln!("[EXC] control_protection");
    loop {
        x86_64::instructions::hlt();
    }
}

pub extern "x86-interrupt" fn pf_handler(sf: InterruptStackFrame, code: PageFaultErrorCode) {
    let addr = Cr2::read();

    let cur = crate::kernel::sched::task::current_ptr();
    unsafe {
        let (tid, ktop, rsp) = if cur.is_null() {
            (0, 0, sf.stack_pointer.as_u64())
        } else {
            (
                (*cur).tid,
                (*cur).kstack_top.as_u64(),
                sf.stack_pointer.as_u64(),
            )
        };

        sprintln!(
            "[#PF] addr={:#018x} code={:?} RIP={:#018x} RSP={:#018x} tid={} kstack_top={:#018x}",
            addr.expect("REASON").as_u64(),
            code,
            sf.instruction_pointer.as_u64(),
            rsp,
            tid,
            ktop,
        );
    }

    loop {
        x86_64::instructions::hlt();
    }
}
