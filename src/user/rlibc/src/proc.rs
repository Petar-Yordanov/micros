extern crate alloc;

use micros_abi::sysnr;
use micros_abi::types::{ProcInfo, ProcSpawnArgs};

use crate::errno::{cvt, Errno};
use crate::syscall::{syscall0, syscall1, syscall2};

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
        pathPtr: path.as_ptr() as u64,
        pathLen: path.len() as u64,
        argvPtr: 0,
        argc: 0,
        flags: 0,
    };

    let r = cvt(unsafe { syscall1(sysnr::SYS_PROC_SPAWN, &args as *const _ as u64) })?;
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
pub fn list(out: &mut [ProcInfo]) -> Result<usize, Errno> {
    let r = cvt(syscall2(
        sysnr::SYS_PROC_LIST,
        out.as_mut_ptr() as u64,
        out.len() as u64,
    ))?;
    Ok(r as usize)
}
