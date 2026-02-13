use micros_abi::sysnr;

use crate::errno::{cvt, Errno};
use crate::syscall::syscall0;

#[inline(always)]
pub fn yield_now() -> Result<(), Errno> {
    cvt(syscall0(sysnr::SYS_YIELD)).map(|_| ())
}
