use micros_abi::sysnr;

use crate::errno::{cvt, Errno};
use crate::syscall::syscall2;

#[inline(always)]
pub fn exec(path: &str) -> Result<(), Errno> {
    cvt(syscall2(sysnr::SYS_EXEC, path.as_ptr() as u64, path.len() as u64)).map(|_| ())
}
