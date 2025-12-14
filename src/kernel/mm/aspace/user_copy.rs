#[allow(unused)]
pub const USER_MAX: u64 = 0x0000_7FFF_FFFF_F000;

#[allow(unused)]
fn is_user_range(ptr: u64, len: usize) -> bool {
    if len == 0 {
        return true;
    }
    let end = match ptr.checked_add(len as u64) {
        Some(e) => e,
        None => return false,
    };
    ptr < end && end <= USER_MAX
}

#[allow(unused)]
pub unsafe fn copy_from_user(dst: *mut u8, src_user: *const u8, len: usize) -> Result<(), ()> {
    let addr = src_user as u64;
    if !is_user_range(addr, len) {
        return Err(());
    }
    core::ptr::copy_nonoverlapping(src_user, dst, len);
    Ok(())
}

#[allow(unused)]
pub unsafe fn copy_to_user(dst_user: *mut u8, src: *const u8, len: usize) -> Result<(), ()> {
    let addr = dst_user as u64;
    if !is_user_range(addr, len) {
        return Err(());
    }
    core::ptr::copy_nonoverlapping(src, dst_user, len);
    Ok(())
}
