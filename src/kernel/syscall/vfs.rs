extern crate alloc;

use alloc::vec::Vec;
use core::cmp::min;

use micros_abi::errno;

use crate::kernel::mm::aspace::user_copy::{copy_from_user, copy_to_user};
use crate::kernel::fs::vfs::error::VfsError;
use crate::kernel::fs::vfs::mount as vfs_mount;
use crate::kernel::fs::vfs::ops as vfs_ops;

use micros_abi::types::{VfsListArgs, VfsMountArgs, VfsMountFs, VfsReadArgs, VfsWriteArgs};

use super::util::{copy_user_str, copy_user_struct};

pub(super) fn sys_vfs_read(args_ptr: u64) -> i64 {
    let args: VfsReadArgs = match copy_user_struct(args_ptr) {
        Ok(v) => v,
        Err(e) => return e,
    };

    let path = match copy_user_str(args.path_ptr, args.path_len) {
        Ok(s) => s,
        Err(e) => return e,
    };

    if args.buf_ptr == 0 {
        return -errno::EFAULT;
    }

    if args.off != 0 {
        return -errno::EINVAL;
    }

    let data = match vfs_ops::vfs_read(&path) {
        Ok(v) => v,
        Err(e) => return vfs_err_to_errno(e),
    };

    let to_copy = min(args.buf_len as usize, data.len());
    if to_copy == 0 {
        return 0;
    }

    unsafe {
        if copy_to_user(args.buf_ptr as *mut u8, data.as_ptr(), to_copy).is_err() {
            return -errno::EFAULT;
        }
    }

    to_copy as i64
}

pub(super) fn sys_vfs_write(args_ptr: u64) -> i64 {
    let args: VfsWriteArgs = match copy_user_struct(args_ptr) {
        Ok(v) => v,
        Err(e) => return e,
    };

    let path = match copy_user_str(args.path_ptr, args.path_len) {
        Ok(s) => s,
        Err(e) => return e,
    };

    if args.buf_ptr == 0 {
        return -errno::EFAULT;
    }

    if args.off != 0 {
        return -errno::EINVAL;
    }

    if args.buf_len > (1024 * 1024) {
        return -errno::EINVAL;
    }

    let mut data = Vec::<u8>::with_capacity(args.buf_len as usize);
    unsafe { data.set_len(args.buf_len as usize) };

    unsafe {
        if copy_from_user(data.as_mut_ptr(), args.buf_ptr as *const u8, data.len()).is_err() {
            return -errno::EFAULT;
        }
    }

    match vfs_ops::vfs_write(&path, &data) {
        Ok(()) => 0,
        Err(e) => vfs_err_to_errno(e),
    }
}

pub(super) fn sys_vfs_list(args_ptr: u64) -> i64 {
    let args: VfsListArgs = match copy_user_struct(args_ptr) {
        Ok(v) => v,
        Err(e) => return e,
    };

    let path = match copy_user_str(args.path_ptr, args.path_len) {
        Ok(s) => s,
        Err(e) => return e,
    };

    if args.out_ptr == 0 {
        return -errno::EFAULT;
    }

    let entries = match vfs_ops::vfs_list(&path) {
        Ok(v) => v,
        Err(e) => return vfs_err_to_errno(e),
    };

    let mut out = Vec::<u8>::new();
    for s in entries {
        out.extend_from_slice(s.as_bytes());
        out.push(b'\n');
    }

    if out.len() > args.out_len as usize {
        return -errno::EINVAL;
    }

    if !out.is_empty() {
        unsafe {
            if copy_to_user(args.out_ptr as *mut u8, out.as_ptr(), out.len()).is_err() {
                return -errno::EFAULT;
            }
        }
    }

    out.len() as i64
}

pub(super) fn sys_vfs_mkdir(path_ptr: u64, path_len: u64) -> i64 {
    use x86_64::instructions::interrupts;

    crate::ksprintln!("[sys_vfs_mkdir] enter path_ptr={:#x} path_len={}", path_ptr, path_len);

    let path = match copy_user_str(path_ptr, path_len) {
        Ok(s) => s,
        Err(e) => return e,
    };

    crate::ksprintln!("[sys_vfs_mkdir] path='{}'", path);
    crate::ksprintln!("[sys_vfs_mkdir] before vfs_mkdir_p");

    let r = interrupts::without_interrupts(|| vfs_ops::vfs_mkdir_p(&path));

    crate::ksprintln!("[sys_vfs_mkdir] after vfs_mkdir_p -> {:?}", r);

    match r {
        Ok(()) => 0,
        Err(e) => vfs_err_to_errno(e),
    }
}

pub(super) fn sys_vfs_mount(args_ptr: u64) -> i64 {
    use x86_64::instructions::interrupts;

    let args: VfsMountArgs = match copy_user_struct(args_ptr) {
        Ok(v) => v,
        Err(e) => return e,
    };

    let mp = match copy_user_str(args.mount_ptr, args.mount_len) {
        Ok(s) => s,
        Err(e) => return e,
    };

    let pid = crate::kernel::sched::proc::current_pid();
    if pid != Some(1) && pid != Some(0) {
        return -errno::EINVAL;
    }

    let fs = args.fs;
    let off = args.base_off_bytes;

    let r = interrupts::without_interrupts(|| match fs {
        x if x == (VfsMountFs::Fat32 as u32) => vfs_mount::mount_fat32(&mp, off),
        x if x == (VfsMountFs::Ext2 as u32) => vfs_mount::mount_ext2(&mp, off),
        _ => Err(VfsError::BadPath),
    });

    match r {
        Ok(()) => 0,
        Err(e) => vfs_err_to_errno(e),
    }
}

fn vfs_err_to_errno(e: VfsError) -> i64 {
    match e {
        VfsError::NotFound => -errno::ENOENT,
        VfsError::BadPath => -errno::EINVAL,
        VfsError::Name => -errno::EINVAL,
        VfsError::NotMounted => -errno::ENODEV,
        VfsError::Unsupported => -errno::ENOSYS,
        VfsError::Full => -errno::ENOSPC,
        VfsError::Io => -errno::EIO,
        VfsError::FsSpecific => -errno::EINVAL,
    }
}
