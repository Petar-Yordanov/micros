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

/// Unmap one bounded batch of pages. Returns `Ok(false)` while still
/// in progress; the caller must retry before destroying or remapping it.
#[inline]
pub fn unmap_step(shm_id: u64) -> Result<bool, Errno> {
    match cvt(syscall1(sysnr::SYS_SHM_UNMAP, shm_id)) {
        Ok(_) => Ok(true),
        Err(Errno(n)) if n as i64 == micros_abi::errno::EAGAIN => Ok(false),
        Err(e) => Err(e),
    }
}

/// Complete the unmap for callers without an event loop (application-side).
/// WM must use `unmap_step` instead so closing a window never blocks drawing.
#[inline]
pub fn unmap(shm_id: u64) -> Result<(), Errno> {
    loop {
        if unmap_step(shm_id)? { return Ok(()); }
        let _ = crate::sched::yield_now();
    }
}
