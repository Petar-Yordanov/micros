extern crate alloc;

use super::util::copy_user_str;

pub(super) fn sys_exec(path_ptr: u64, path_len: u64) -> i64 {
    let path = match copy_user_str(path_ptr, path_len) {
        Ok(s) => s,
        Err(e) => return e,
    };

    crate::ksprintln!("[syscall] sys_exec {}", path);
    crate::kernel::exec::run_user_elf(&path, "user-exec")
}
