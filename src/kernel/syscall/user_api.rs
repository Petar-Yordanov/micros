#[inline]
#[allow(unused)]
pub unsafe fn syscall0(nr: u64) -> u64 {
    let ret: u64;
    core::arch::asm!(
        "int 0x80",
        in("rax") nr,
        lateout("rax") ret,
        options(nostack),
    );
    ret
}

#[inline]
#[allow(unused)]
pub unsafe fn syscall1(nr: u64, a0: u64) -> u64 {
    let ret: u64;
    core::arch::asm!(
        "int 0x80",
        in("rax") nr,
        in("rdi") a0,
        lateout("rax") ret,
        options(nostack),
    );
    ret
}
