use micros_abi::sysnr;
use crate::syscall::syscall2;

#[inline(always)]
pub fn log(s: &str) -> i64 {
    syscall2(sysnr::SYS_LOG, s.as_ptr() as u64, s.len() as u64)
}
