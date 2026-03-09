extern crate alloc;

use micros_abi::sysnr;
use micros_abi::types::{ProcInfo, ProcListEntry, ProcSpawnArgs};

use crate::errno::{cvt, Errno};
use crate::syscall::{syscall0, syscall1, syscall2, syscall3};

pub type Pid = u32;

#[inline(always)]
pub fn getpid() -> Result<Pid, Errno> {
    let r = cvt(syscall0(sysnr::SYS_GETPID))?;
    Ok(r as Pid)
}

#[inline(always)]
pub fn exit(code: u64) -> ! {
    let _ = syscall1(sysnr::SYS_EXIT, code);
    loop {
        let _ = syscall0(sysnr::SYS_YIELD);
    }
}

#[inline(always)]
pub fn spawn(path: &str) -> Result<Pid, Errno> {
    let args = ProcSpawnArgs {
        path_ptr: path.as_ptr() as u64,
        path_len: path.len() as u64,
        argv_ptr: 0,
        argc: 0,
        flags: 0,
    };

    let r = cvt(syscall1(sysnr::SYS_PROC_SPAWN, &args as *const _ as u64))?;
    Ok(r as Pid)
}

#[inline(always)]
pub fn wait(pid: Pid, status_out_ptr: u64) -> Result<Pid, Errno> {
    let r = cvt(syscall2(sysnr::SYS_PROC_WAIT, pid as u64, status_out_ptr))?;
    Ok(r as Pid)
}

#[inline(always)]
pub fn kill(pid: Pid, signal_or_reason: u64) -> Result<(), Errno> {
    cvt(syscall2(sysnr::SYS_PROC_KILL, pid as u64, signal_or_reason)).map(|_| ())
}

#[inline(always)]
pub fn proc_count() -> Result<u64, Errno> {
    let mut total: u64 = 0;
    let _ = cvt(syscall3(
        sysnr::SYS_PROC_LIST,
        0,
        0,
        &mut total as *mut _ as u64,
    ))?;
    Ok(total)
}

#[inline(always)]
pub fn list(out: &mut [ProcListEntry]) -> Result<(usize, usize), Errno> {
    let mut total: u64 = 0;
    let written = cvt(syscall3(
        sysnr::SYS_PROC_LIST,
        out.as_mut_ptr() as u64,
        out.len() as u64,
        &mut total as *mut _ as u64,
    ))?;
    Ok((written as usize, total as usize))
}

#[inline(always)]
pub fn info(pid: Pid) -> Result<ProcInfo, Errno> {
    let mut out = ProcInfo::default();
    cvt(syscall2(
        sysnr::SYS_PROC_INFO,
        pid as u64,
        &mut out as *mut _ as u64,
    ))?;
    Ok(out)
}
