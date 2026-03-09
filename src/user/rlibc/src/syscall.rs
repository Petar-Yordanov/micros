#![allow(dead_code)]
use core::arch::asm;

#[inline(always)]
unsafe fn raw_syscall6(nr: u64, a0: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64) -> i64 {
    let ret: i64;
    asm!(
        "int 0x80",
        inlateout("rax") (nr as i64) => ret,
        in("rdi") a0,
        in("rsi") a1,
        in("rdx") a2,
        in("r10") a3,
        in("r8")  a4,
        in("r9")  a5,
        options(nostack),
    );
    ret
}

#[inline(always)]
pub fn syscall0(nr: u64) -> i64 {
    unsafe { raw_syscall6(nr, 0, 0, 0, 0, 0, 0) }
}
#[inline(always)]
pub fn syscall1(nr: u64, a0: u64) -> i64 {
    unsafe { raw_syscall6(nr, a0, 0, 0, 0, 0, 0) }
}
#[inline(always)]
pub fn syscall2(nr: u64, a0: u64, a1: u64) -> i64 {
    unsafe { raw_syscall6(nr, a0, a1, 0, 0, 0, 0) }
}
#[inline(always)]
pub fn syscall3(nr: u64, a0: u64, a1: u64, a2: u64) -> i64 {
    unsafe { raw_syscall6(nr, a0, a1, a2, 0, 0, 0) }
}
#[inline(always)]
pub fn syscall4(nr: u64, a0: u64, a1: u64, a2: u64, a3: u64) -> i64 {
    unsafe { raw_syscall6(nr, a0, a1, a2, a3, 0, 0) }
}
#[inline(always)]
pub fn syscall5(nr: u64, a0: u64, a1: u64, a2: u64, a3: u64, a4: u64) -> i64 {
    unsafe { raw_syscall6(nr, a0, a1, a2, a3, a4, 0) }
}
#[inline(always)]
pub fn syscall6(nr: u64, a0: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64) -> i64 {
    unsafe { raw_syscall6(nr, a0, a1, a2, a3, a4, a5) }
}
