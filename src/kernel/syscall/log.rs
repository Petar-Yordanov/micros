extern crate alloc;

use alloc::vec::Vec;

use micros_abi::errno;

use crate::kernel::mm::aspace::user_copy::copy_from_user;

pub(super) fn sys_log(user_ptr: u64, user_len: u64) -> i64 {
    if user_ptr == 0 {
        return -errno::EFAULT;
    }

    const MAX: usize = 4096;
    let len = user_len as usize;

    if len == 0 {
        return 0;
    }
    if len > MAX {
        return -errno::EINVAL;
    }

    let mut buf = Vec::<u8>::new();
    buf.resize(len, 0);

    unsafe {
        if copy_from_user(buf.as_mut_ptr(), user_ptr as *const u8, len).is_err() {
            return -errno::EFAULT;
        }
    }

    match core::str::from_utf8(&buf) {
        Ok(s) => crate::ksprintln!("[user] {}", s),
        Err(_) => crate::ksprintln!("[user] <non-utf8 {} bytes>", len),
    }

    0
}
