use micros_abi::sysnr;
use micros_abi::types::FbInfo;
use crate::syscall::{syscall0, syscall1};

#[inline(always)]
pub fn fb_info(out: &mut FbInfo) -> i64 {
    syscall1(sysnr::SYS_FB_INFO, out as *mut _ as u64)
}

#[inline(always)]
pub fn fb_map() -> i64 {
    syscall0(sysnr::SYS_FB_MAP)
}
