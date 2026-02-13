extern crate alloc;

use alloc::{string::String, vec::Vec};

use micros_abi::sysnr;
use micros_abi::types::{VfsListArgs, VfsMountArgs, VfsMountFs, VfsReadArgs, VfsWriteArgs};

use crate::errno::{cvt, Errno};
use crate::syscall::syscall1;

pub fn mount(fs: VfsMountFs, mountpoint: &str, base_off_bytes: u64) -> Result<(), Errno> {
    let args = VfsMountArgs {
        fs: fs as u32,
        mountPtr: mountpoint.as_ptr() as u64,
        mountLen: mountpoint.len() as u64,
        baseOffBytes: base_off_bytes,
    };
    cvt(unsafe { syscall1(sysnr::SYS_VFS_MOUNT, &args as *const _ as u64) }).map(|_| ())
}

pub fn read(path: &str, max_bytes: usize) -> Result<Vec<u8>, Errno> {
    let mut buf = Vec::<u8>::with_capacity(max_bytes);
    unsafe { buf.set_len(max_bytes) };

    let args = VfsReadArgs {
        pathPtr: path.as_ptr() as u64,
        pathLen: path.len() as u64,
        off: 0,
        bufPtr: buf.as_mut_ptr() as u64,
        bufLen: max_bytes as u64,
    };

    let n = cvt(unsafe { syscall1(sysnr::SYS_VFS_READ, &args as *const _ as u64) })? as usize;
    buf.truncate(n);
    Ok(buf)
}

pub fn write(path: &str, data: &[u8]) -> Result<(), Errno> {
    let args = VfsWriteArgs {
        pathPtr: path.as_ptr() as u64,
        pathLen: path.len() as u64,
        off: 0,
        bufPtr: data.as_ptr() as u64,
        bufLen: data.len() as u64,
    };

    cvt(unsafe { syscall1(sysnr::SYS_VFS_WRITE, &args as *const _ as u64) }).map(|_| ())
}

pub fn mkdir_p(path: &str) -> Result<(), Errno> {
    cvt(unsafe {
        crate::syscall::syscall2(sysnr::SYS_VFS_MKDIR, path.as_ptr() as u64, path.len() as u64)
    })
    .map(|_| ())
}

pub fn list(path: &str, max_bytes: usize) -> Result<Vec<String>, Errno> {
    let mut out = Vec::<u8>::with_capacity(max_bytes);
    unsafe { out.set_len(max_bytes) };

    let args = VfsListArgs {
        pathPtr: path.as_ptr() as u64,
        pathLen: path.len() as u64,
        outPtr: out.as_mut_ptr() as u64,
        outLen: max_bytes as u64,
    };

    let n = cvt(unsafe { syscall1(sysnr::SYS_VFS_LIST, &args as *const _ as u64) })? as usize;
    out.truncate(n);

    let s = core::str::from_utf8(&out).map_err(|_| Errno(22))?;
    Ok(s.lines().map(|l| String::from(l)).collect())
}
