use micros_abi::sysnr;

use crate::errno::{cvt, Errno};
use crate::syscall::syscall1;

pub const POWER_ACTION_OFF: u64 = 0;
pub const POWER_ACTION_REBOOT: u64 = 1;

#[inline(always)]
pub fn power(action: u64) -> Result<(), Errno> {
    cvt(syscall1(sysnr::SYS_POWER, action)).map(|_| ())
}

#[inline(always)]
pub fn power_off() -> Result<(), Errno> {
    power(POWER_ACTION_OFF)
}

#[inline(always)]
pub fn reboot() -> Result<(), Errno> {
    power(POWER_ACTION_REBOOT)
}
