use crate::arch::x86_64::time::apic;
use crate::ksprintln;
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
        ksprintln!("[VIRTIO] Input interrupt");
        apic::eoi();
    }
}

pub extern "x86-interrupt" fn virtio_net_irq(_sf: InterruptStackFrame) {
    unsafe {
        ksprintln!("[VIRTIO] Net interrupt");
        apic::eoi();
    }
}

pub extern "x86-interrupt" fn virtio_blk_irq(_sf: InterruptStackFrame) {
    unsafe {
        ksprintln!("[VIRTIO] Blk interrupt");
        apic::eoi();
    }
}

pub extern "x86-interrupt" fn virtio_gpu_irq(_sf: InterruptStackFrame) {
    unsafe {
        ksprintln!("[VIRTIO] GPU interrupt");
        apic::eoi();
    }
}

pub extern "x86-interrupt" fn spurious_irq(_sf: InterruptStackFrame) {
    unsafe {
        ksprintln!("Spurious interrupt");
        apic::eoi();
    }
}

#[inline(always)]
extern "C" fn apic_eoi_rust() {
    unsafe { apic::eoi(); }
}

#[inline(never)]
extern "C" fn timer_preempt_rust(
    tf: *mut crate::kernel::sched::task::TrapFrame,
) -> *const crate::kernel::sched::task::TrapFrame {
    crate::kernel::sched::task::on_tick();

    if crate::kernel::sched::task::in_syscall() {
        return core::ptr::null();
    }

    unsafe { crate::kernel::sched::task::preempt_from_timer(&mut *tf) }
}

#[unsafe(naked)]
pub extern "C" fn timer_entry() {
    core::arch::naked_asm!(
        r#"
        // IRQ entry, CPU already pushed an iret frame.
        // All regs MUST be preserved, no matter what path is taken.

        // Save all GPRs (15 pushes = 120 bytes)
        push r15
        push r14
        push r13
        push r12
        push r11
        push r10
        push r9
        push r8
        push rdi
        push rsi
        push rdx
        push rcx
        push rbx
        push rbp
        push rax

        // Stack alignment + scratch TrapFrame
        //
        // SysV requires 16-byte alignment at call sites.
        // We reserve 168 bytes (160 TF + 8 pad) so that after 15 pushes,
        // the stack is aligned for our calls.
        //
        // Layout:
        //   [rsp+0..8)    = padding
        //   [rsp+8..168)  = TrapFrame (160 bytes)
        //   [rsp+168..]   = saved regs block (120 bytes) (top = rsp+168)
        //   [saved+120..] = iret frame (RIP,CS,RFLAGS,[RSP,SS])
        sub rsp, 168
        lea r10, [rsp + 8]          // r10 = tf*

        lea r9,  [rsp + 168]        // r9  = saved regs block base
        lea r11, [r9 + 120]         // r11 = iret frame base

        // Preserve these pointers across Rust calls (caller-saved regs can be clobbered)
        // Use callee-saved regs so they survive timer_preempt_rust/apic_eoi_rust.
        mov r14, r9                 // r14 = saved regs base
        mov r15, r11                // r15 = iret frame base

        // Layout of pushed regs at [saved + off] (saved = r14):
        //  0  rax
        //  8  rbp
        // 16  rbx
        // 24  rcx
        // 32  rdx
        // 40  rsi
        // 48  rdi
        // 56  r8
        // 64  r9
        // 72  r10
        // 80  r11
        // 88  r12
        // 96  r13
        // 104 r14
        // 112 r15

        // Fill TrapFrame GPRs (TrapFrame layout must match your Rust struct):
        //   0..112  = r15..rax (15*8 = 120 bytes? actually we store 15 regs as u64s)
        // Here you store 14 regs + rax (as you had). Keep exactly consistent with your TrapFrame.
        mov rax, [r14 + 112]        // r15
        mov [r10 + 0], rax
        mov rax, [r14 + 104]        // r14
        mov [r10 + 8], rax
        mov rax, [r14 + 96]         // r13
        mov [r10 + 16], rax
        mov rax, [r14 + 88]         // r12
        mov [r10 + 24], rax
        mov rax, [r14 + 80]         // r11
        mov [r10 + 32], rax
        mov rax, [r14 + 72]         // r10
        mov [r10 + 40], rax
        mov rax, [r14 + 64]         // r9
        mov [r10 + 48], rax
        mov rax, [r14 + 56]         // r8
        mov [r10 + 56], rax
        mov rax, [r14 + 40]         // rsi
        mov [r10 + 64], rax
        mov rax, [r14 + 48]         // rdi
        mov [r10 + 72], rax
        mov rax, [r14 + 8]          // rbp
        mov [r10 + 80], rax
        mov rax, [r14 + 32]         // rdx
        mov [r10 + 88], rax
        mov rax, [r14 + 24]         // rcx
        mov [r10 + 96], rax
        mov rax, [r14 + 16]         // rbx
        mov [r10 + 104], rax
        mov rax, [r14 + 0]          // rax
        mov [r10 + 112], rax

        // Copy iret frame into TrapFrame (iret base = r15):
        mov rax, [r15 + 0]          // RIP
        mov [r10 + 120], rax
        mov rax, [r15 + 8]          // CS
        mov [r10 + 128], rax
        mov rax, [r15 + 16]         // RFLAGS
        mov [r10 + 136], rax

        // Determine CPL from CS (cs&3)
        mov rax, [r15 + 8]
        and rax, 3
        cmp rax, 3
        jne 1f

        // From user: frame has RSP, SS
        mov rax, [r15 + 24]
        mov [r10 + 144], rax
        mov rax, [r15 + 32]
        mov [r10 + 152], rax
        jmp 2f

    1:
        // From kernel: no user RSP/SS in the iret frame
        xor rax, rax
        mov [r10 + 144], rax
        mov [r10 + 152], rax

    2:
        // Call Rust: timer_preempt_rust(tf) -> next_tf or NULL
        mov rdi, r10
        call {timer_preempt}

        // Preserve returned next_tf across the EOI call (EOI is a Rust call; rax is caller-saved)
        mov rbx, rax

        // Always EOI before returning
        call {apic_eoi}

        // Restore next_tf pointer
        mov rax, rbx

        // If rax == 0: no switch, keep current
        test rax, rax
        jz 9f

        // Only switch if next is user-mode (cs&3==3)
        mov rdx, [rax + 128]
        and rdx, 3
        cmp rdx, 3
        jne 9f

        // APPLY SWITCH

        // Overwrite iret frame for return-to-user (iret base = r15)
        mov rdx, [rax + 120]        // RIP
        mov [r15 + 0], rdx
        mov rdx, [rax + 128]        // CS
        mov [r15 + 8], rdx
        mov rdx, [rax + 136]        // RFLAGS
        mov [r15 + 16], rdx
        mov rdx, [rax + 144]        // user RSP
        mov [r15 + 24], rdx
        mov rdx, [rax + 152]        // user SS
        mov [r15 + 32], rdx

        // Overwrite saved-reg block so pops restore next task's regs (saved base = r14)
        mov rdx, [rax + 0]          // r15
        mov [r14 + 112], rdx
        mov rdx, [rax + 8]          // r14
        mov [r14 + 104], rdx
        mov rdx, [rax + 16]         // r13
        mov [r14 + 96], rdx
        mov rdx, [rax + 24]         // r12
        mov [r14 + 88], rdx
        mov rdx, [rax + 32]         // r11
        mov [r14 + 80], rdx
        mov rdx, [rax + 40]         // r10
        mov [r14 + 72], rdx
        mov rdx, [rax + 48]         // r9
        mov [r14 + 64], rdx
        mov rdx, [rax + 56]         // r8
        mov [r14 + 56], rdx
        mov rdx, [rax + 64]         // rsi
        mov [r14 + 40], rdx
        mov rdx, [rax + 72]         // rdi
        mov [r14 + 48], rdx
        mov rdx, [rax + 80]         // rbp
        mov [r14 + 8], rdx
        mov rdx, [rax + 88]         // rdx
        mov [r14 + 32], rdx
        mov rdx, [rax + 96]         // rcx
        mov [r14 + 24], rdx
        mov rdx, [rax + 104]        // rbx
        mov [r14 + 16], rdx
        mov rdx, [rax + 112]        // rax
        mov [r14 + 0], rdx

    9:
        // Free temporary TrapFrame space (168 bytes)
        add rsp, 168

        // Restore all regs (reverse of pushes)
        pop rax
        pop rbp
        pop rbx
        pop rcx
        pop rdx
        pop rsi
        pop rdi
        pop r8
        pop r9
        pop r10
        pop r11
        pop r12
        pop r13
        pop r14
        pop r15

        iretq
        "#,
        timer_preempt = sym timer_preempt_rust,
        apic_eoi      = sym apic_eoi_rust,
    );
}
