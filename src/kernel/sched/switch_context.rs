#[unsafe(naked)]
#[no_mangle]
pub extern "C" fn switch_context(_old: *mut u8, _new: *const u8) {
    core::arch::naked_asm!(
        r#"
        // rdi = old (Context* as u8*)
        // rsi = new (Context* as u8*)

        // Save old
        mov     [rdi + 0x00], rsp
        mov     [rdi + 0x08], r15
        mov     [rdi + 0x10], r14
        mov     [rdi + 0x18], r13
        mov     [rdi + 0x20], r12
        mov     [rdi + 0x28], rbx
        mov     [rdi + 0x30], rbp

        lea     rax, [rip + 1f]
        mov     [rdi + 0x40], rax

        // Load new
        mov     rsp, [rsi + 0x00]
        mov     r15, [rsi + 0x08]
        mov     r14, [rsi + 0x10]
        mov     r13, [rsi + 0x18]
        mov     r12, [rsi + 0x20]
        mov     rbx, [rsi + 0x28]
        mov     rbp, [rsi + 0x30]
        mov     rdi, [rsi + 0x38]

        mov     rax, [rsi + 0x40]
        jmp     rax

    1:
        ret
    "#
    );
}
