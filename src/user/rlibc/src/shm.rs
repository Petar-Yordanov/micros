use micros_abi::sysnr;
use micros_abi::types::{ShmCreateArgs, ShmMapArgs};

use crate::errno::{cvt, Errno};
use crate::syscall::syscall1;

#[inline(always)]
pub fn create(size: u64, flags: u64) -> Result<u64, Errno> {
    let args = ShmCreateArgs { size, flags };
    let r = cvt(syscall1(sysnr::SYS_SHM_CREATE, &args as *const _ as u64))?;
    Ok(r as u64)
}

#[inline(always)]
pub fn map(shm_id: u64, desired_va: u64, flags: u64) -> Result<u64, Errno> {
    let args = ShmMapArgs {
        shm_id,
        desired_va,
        flags,
    };
    let r = cvt(syscall1(sysnr::SYS_SHM_MAP, &args as *const _ as u64))?;
    Ok(r as u64)
}
