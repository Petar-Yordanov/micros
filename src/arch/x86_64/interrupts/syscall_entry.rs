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
    if n < 32 {
        crate::sprintln!(
            "[syscall] #{} nr={} a0={:#x} a1={:#x} a2={:#x} a3={:#x} a4={:#x} a5={:#x}",
            n, nr, a0, a1, a2, a3, a4, a5
        );
    }
    kdispatch::dispatch(nr, a0, a1, a2, a3, a4, a5)
}

#[unsafe(naked)]
pub extern "C" fn syscall_entry() {
    core::arch::naked_asm!(
        r#"
        // CPU pushed on CPL3->CPL0:
        //   SS, RSP, RFLAGS, CS, RIP

        // Save GPRs we might clobber.
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

        // 14 pushes = 112 bytes, keeps stack 16-byte aligned for calls.

        // Build args for syscall_dispatch_rust(nr, a0..a5)
        // User ABI for int80:
        //   rax = nr
        //   rdi,rsi,rdx,r10,r8,r9 = args

        mov rdi, rax            // nr (1st arg)

        // Restore original arg regs from our save area:
        // Layout at rsp (top):
        //   rbp rbx rcx rdx rsi rdi r8 r9 r10 r11 r12 r13 r14 r15
        mov rsi, [rsp + 0x28]   // a0 = saved rdi
        mov rdx, [rsp + 0x20]   // a1 = saved rsi
        mov rcx, [rsp + 0x18]   // a2 = saved rdx
        mov r8,  [rsp + 0x40]   // a3 = saved r10
        mov r9,  [rsp + 0x30]   // a4 = saved r8

        // 7th arg (a5 = saved r9) goes on stack for SysV.
        sub rsp, 8
        mov rax, [rsp + 0x40]   // after sub, old [rsp+0x38] becomes [rsp+0x40]
        mov [rsp], rax

        call {dispatch}

        add rsp, 8              // pop stack arg
        // return value already in rax (i64)

        // Restore regs
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
        dispatch = sym syscall_dispatch_rust,
    );
}
