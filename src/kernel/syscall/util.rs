extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use micros_abi::errno;

use crate::kernel::mm::aspace::user_copy::copy_from_user;

pub(super) fn copy_user_str(ptr: u64, len: u64) -> Result<String, i64> {
    if ptr == 0 {
        return Err(-errno::EFAULT);
    }
    if len == 0 || len > 4096 {
        return Err(-errno::EINVAL);
    }

    let mut buf = Vec::<u8>::with_capacity(len as usize);
    unsafe { buf.set_len(len as usize) };

    unsafe {
        if copy_from_user(buf.as_mut_ptr(), ptr as *const u8, buf.len()).is_err() {
            return Err(-errno::EFAULT);
        }
    }

    while buf.last().copied() == Some(0) {
        buf.pop();
    }

    if buf.iter().any(|&b| b == 0) {
        return Err(-errno::EINVAL);
    }

    core::str::from_utf8(&buf)
        .map(|s| String::from(s))
        .map_err(|_| -errno::EINVAL)
}

pub(super) fn copy_user_struct<T: Copy>(ptr: u64) -> Result<T, i64> {
    if ptr == 0 {
        return Err(-errno::EFAULT);
    }

    let mut tmp: T = unsafe { core::mem::zeroed() };
    unsafe {
        if copy_from_user(
            (&mut tmp as *mut T) as *mut u8,
            ptr as *const u8,
            core::mem::size_of::<T>(),
        )
        .is_err()
        {
            return Err(-errno::EFAULT);
        }
    }

    Ok(tmp)
}
