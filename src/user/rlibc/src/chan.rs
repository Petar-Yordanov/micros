use micros_abi::sysnr;
use micros_abi::types::{ChanCreateArgs, ChanRecvArgs, ChanSendArgs, ChanNameArgs};

use crate::errno::{cvt, Errno};
use crate::syscall::{syscall1,syscall2};

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

#[inline]
pub fn publish(name:&str,chan_id:u64)->Result<(),Errno>{
    let args=ChanNameArgs{name_ptr:name.as_ptr() as u64,name_len:name.len() as u64,chan_id};
    cvt(syscall1(sysnr::SYS_CHAN_PUBLISH,&args as *const _ as u64)).map(|_|())
}
#[inline]
pub fn find(name:&str)->Result<u64,Errno>{
    let args=ChanNameArgs{name_ptr:name.as_ptr() as u64,name_len:name.len() as u64,chan_id:0};
    cvt(syscall1(sysnr::SYS_CHAN_FIND,&args as *const _ as u64)).map(|x|x as u64)
}

/// Receive one message or mark the current userspace task blocked until
/// a sender wakes it. EAGAIN is returned before the deferred IRQ switch;
/// simply retry in the application event loop. Do not busy-poll recv().
#[inline]
pub fn recv_wait(chan_id:u64,out:&mut [u8])->Result<usize,Errno>{
    let args=ChanRecvArgs{chan_id,out_ptr:out.as_mut_ptr() as u64,out_cap:out.len() as u64};
    let r=cvt(syscall1(sysnr::SYS_CHAN_RECV_WAIT,&args as *const _ as u64))?;
    Ok(r as usize)
}

/// Like recv_wait, but wakes after timeout_ms even without a message. Zero
/// means indefinite until a message arrives (no busy spin needed).
#[inline]
pub fn recv_wait_timeout(chan_id:u64,out:&mut [u8],timeout_ms:u64)->Result<usize,Errno>{
    let args=ChanRecvArgs{chan_id,out_ptr:out.as_mut_ptr() as u64,out_cap:out.len() as u64};
    let r=cvt(syscall2(sysnr::SYS_CHAN_RECV_WAIT,&args as *const _ as u64,timeout_ms))?;
    Ok(r as usize)
}
