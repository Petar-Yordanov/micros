use core::slice;
use micros_abi::types::FbInfo;
use rlibc::fb::{fb_info, fb_map};

pub fn get_fb() -> Option<(FbInfo, &'static mut [u8])> {
    let mut info = FbInfo {
        addr: 0,
        width: 0,
        height: 0,
        pitch: 0,
        bpp: 0,
    };

    if fb_info(&mut info) < 0 {
        return None;
    }

    let mapped = fb_map();
    if mapped < 0 {
        return None;
    }

    let ptr = mapped as usize as *mut u8;
    if ptr.is_null() {
        return None;
    }

    let len = (info.pitch as usize) * (info.height as usize);
    let buf = unsafe { slice::from_raw_parts_mut(ptr, len) };
    Some((info, buf))
}
