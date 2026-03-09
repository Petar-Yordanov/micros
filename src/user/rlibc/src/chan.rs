use micros_abi::sysnr;
use micros_abi::types::{ChanCreateArgs, ChanRecvArgs, ChanSendArgs};

use crate::errno::{cvt, Errno};
use crate::syscall::syscall1;

#[inline(always)]
pub fn create(flags: u64) -> Result<u64, Errno> {
    let args = ChanCreateArgs { flags };
    let r = cvt(syscall1(sysnr::SYS_CHAN_CREATE, &args as *const _ as u64))?;
    Ok(r as u64)
}

#[inline(always)]
pub fn send(chan_id: u64, data: &[u8]) -> Result<usize, Errno> {
    let args = ChanSendArgs {
        chan_id,
        data_ptr: data.as_ptr() as u64,
        data_len: data.len() as u64,
    };
    let r = cvt(syscall1(sysnr::SYS_CHAN_SEND, &args as *const _ as u64))?;
    Ok(r as usize)
}

#[inline(always)]
pub fn recv(chan_id: u64, out: &mut [u8]) -> Result<usize, Errno> {
    let args = ChanRecvArgs {
        chan_id,
        out_ptr: out.as_mut_ptr() as u64,
        out_cap: out.len() as u64,
    };
    let r = cvt(syscall1(sysnr::SYS_CHAN_RECV, &args as *const _ as u64))?;
    Ok(r as usize)
}
