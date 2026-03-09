use micros_abi::sysnr;

use crate::errno::{cvt, Errno};
use crate::syscall::{syscall0, syscall1};

#[inline(always)]
pub fn yield_now() -> Result<(), Errno> {
    cvt(syscall0(sysnr::SYS_YIELD)).map(|_| ())
}

#[inline(always)]
pub fn sleep_ms(ms: u64) -> Result<(), Errno> {
    cvt(syscall1(sysnr::SYS_SLEEP_MS, ms)).map(|_| ())
}
