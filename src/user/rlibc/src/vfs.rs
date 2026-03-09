extern crate alloc;

use alloc::{string::String, vec::Vec};

use micros_abi::sysnr;
use micros_abi::types::{VfsListArgs, VfsMountArgs, VfsMountFs, VfsReadArgs, VfsWriteArgs};

use crate::errno::{cvt, Errno};
use crate::syscall::{syscall1, syscall2};

pub fn mount(fs: VfsMountFs, mountpoint: &str, base_off_bytes: u64) -> Result<(), Errno> {
    let args = VfsMountArgs {
        fs: fs as u32,
        mount_ptr: mountpoint.as_ptr() as u64,
        mount_len: mountpoint.len() as u64,
        base_off_bytes,
    };
    cvt(syscall1(sysnr::SYS_VFS_MOUNT, &args as *const _ as u64)).map(|_| ())
}

#[inline(always)]
pub fn mount_root_auto() -> Result<(), Errno> {
    mount(VfsMountFs::Ext2, "/", 0).or_else(|_| mount(VfsMountFs::Fat32, "/", 0))
}

pub fn read(path: &str, max_bytes: usize) -> Result<Vec<u8>, Errno> {
    let mut buf = Vec::<u8>::with_capacity(max_bytes);
    unsafe { buf.set_len(max_bytes) };

    let args = VfsReadArgs {
        path_ptr: path.as_ptr() as u64,
        path_len: path.len() as u64,
        off: 0,
        buf_ptr: buf.as_mut_ptr() as u64,
        buf_len: max_bytes as u64,
    };

    let n = cvt(syscall1(sysnr::SYS_VFS_READ, &args as *const _ as u64))? as usize;
    buf.truncate(n);
    Ok(buf)
}

pub fn write(path: &str, data: &[u8]) -> Result<(), Errno> {
    let args = VfsWriteArgs {
        path_ptr: path.as_ptr() as u64,
        path_len: path.len() as u64,
        off: 0,
        buf_ptr: data.as_ptr() as u64,
        buf_len: data.len() as u64,
    };

    cvt(syscall1(sysnr::SYS_VFS_WRITE, &args as *const _ as u64)).map(|_| ())
}

pub fn mkdir_p(path: &str) -> Result<(), Errno> {
    cvt(syscall2(
        sysnr::SYS_VFS_MKDIR,
        path.as_ptr() as u64,
        path.len() as u64,
    ))
    .map(|_| ())
}

pub fn list(path: &str, max_bytes: usize) -> Result<Vec<String>, Errno> {
    let mut out = Vec::<u8>::with_capacity(max_bytes);
    unsafe { out.set_len(max_bytes) };

    let args = VfsListArgs {
        path_ptr: path.as_ptr() as u64,
        path_len: path.len() as u64,
        out_ptr: out.as_mut_ptr() as u64,
        out_len: max_bytes as u64,
    };

    let n = cvt(syscall1(sysnr::SYS_VFS_LIST, &args as *const _ as u64))? as usize;
    out.truncate(n);

    let s = core::str::from_utf8(&out).map_err(|_| Errno(22))?;
    Ok(s.lines().map(String::from).collect())
}
