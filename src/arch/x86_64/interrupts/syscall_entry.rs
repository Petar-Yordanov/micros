use crate::kernel::syscall::dispatch as kdispatch;
use core::sync::atomic::{AtomicU32, Ordering};

static SYSCALL_CNT: AtomicU32 = AtomicU32::new(0);

#[inline(never)]
extern "C" fn syscall_dispatch_rust(
    nr: u64,
    a0: u64,
    a1: u64,
    a2: u64,
    a3: u64,
    a4: u64,
    a5: u64,
) -> i64 {
    let n = SYSCALL_CNT.fetch_add(1, Ordering::Relaxed);
    if n < 30 {
        crate::ksprintln!(
            "[syscall] #{} nr={} a0={:#x} a1={:#x} a2={:#x} a3={:#x} a4={:#x} a5={:#x}",
            n,
            nr,
            a0,
            a1,
            a2,
            a3,
            a4,
            a5
        );
    }
    kdispatch::dispatch(nr, a0, a1, a2, a3, a4, a5)
}

#[inline(always)]
extern "C" fn syscall_enter_rust() {
    crate::kernel::sched::task::syscall_enter();
}

#[inline(always)]
extern "C" fn syscall_exit_rust() {
    crate::kernel::sched::task::syscall_exit();
}

#[unsafe(naked)]
pub extern "C" fn syscall_entry() {
    core::arch::naked_asm!(
        r#"
        // Save original syscall regs
        push r15
        push r14
        push r13
        push r12
        push r11
        push r10
        push r9
        push r8
        push rsi
        push rdi
        push rbp
        push rdx
        push rcx
        push rbx
        // DO NOT push rax yet; return value in rax later.
        push rax

        // Mark "in syscall" BEFORE enabling interrupts
        call {enter}
        sti

        // Load nr into rdi
        mov rdi, [rsp + 0]      // nr

        // Load a0..a4 into regs
        mov rsi, [rsp + 40]     // a0
        mov rdx, [rsp + 48]     // a1
        mov rcx, [rsp + 24]     // a2
        mov r8,  [rsp + 72]     // a3  (saved r10)
        mov r9,  [rsp + 56]     // a4  (saved r8)

        sub rsp, 8              // pad to keep alignment
        push qword ptr [rsp + 8 + 64]  // a5 (saved r9). Note: +8 because we sub rsp.

        call {dispatch}

        // Pop a5 + pad
        add rsp, 16

        // Leaving syscall
        call {exit}
        cli

        // Restore regs. Keep return value in RAX.
        // rax are saved on the stack; discard it.
        add rsp, 8              // drop saved rax

        pop rbx
        pop rcx
        pop rdx
        pop rbp
        pop rdi
        pop rsi
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
        enter = sym syscall_enter_rust,
        dispatch = sym syscall_dispatch_rust,
        exit = sym syscall_exit_rust,
    );
}
