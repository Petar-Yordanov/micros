use micros_abi::sysnr;
use micros_abi::types::TimeSpec;

use crate::errno::{cvt, Errno};
use crate::syscall::syscall1;

#[inline(always)]
pub fn wall_time() -> Result<TimeSpec, Errno> {
    let mut out = TimeSpec::default();
    cvt(syscall1(sysnr::SYS_TIME_WALL, &mut out as *mut _ as u64))?;
    Ok(out)
}

#[inline(always)]
pub fn uptime() -> Result<TimeSpec, Errno> {
    let mut out = TimeSpec::default();
    cvt(syscall1(sysnr::SYS_TIME_UPTIME, &mut out as *mut _ as u64))?;
    Ok(out)
}
