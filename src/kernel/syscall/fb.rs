extern crate alloc;

use core::mem;

use micros_abi::errno;

use crate::kernel::mm::aspace::user_copy::copy_to_user;
use crate::kernel::mm::user::mapfb;

use micros_abi::types::FbInfo;

use crate::platform::limine::framebuffer::FRAMEBUFFER_REQ;
use crate::platform::limine::hhdm::HHDM_REQ;

pub(super) fn fb_info(out: &mut FbInfo) -> bool {
    let resp = match FRAMEBUFFER_REQ.get_response() {
        Some(r) => r,
        None => {
            crate::ksprintln!("[syscall] fb_info: no framebuffer response from Limine");
            return false;
        }
    };

    let mut fb_opt = None;
    for fb in resp.framebuffers() {
        fb_opt = Some(fb);
        break;
    }

    let fb = match fb_opt {
        Some(f) => f,
        None => {
            crate::ksprintln!("[syscall] fb_info: no framebuffers in response");
            return false;
        }
    };

    out.addr = fb.addr() as u64;
    out.width = fb.width() as u32;
    out.height = fb.height() as u32;
    out.pitch = fb.pitch() as u32;
    out.bpp = fb.bpp() as u32;

    true
}

pub(super) fn sys_fb_info(user_fb_ptr: u64) -> i64 {
    if user_fb_ptr == 0 {
        return -errno::EFAULT;
    }

    crate::ksprintln!("[syscall] sys_fb_info(user_ptr={:#x})", user_fb_ptr);

    let mut tmp = FbInfo {
        addr: 0,
        width: 0,
        height: 0,
        pitch: 0,
        bpp: 0,
    };

    if !fb_info(&mut tmp) {
        return -errno::ENODEV;
    }

    crate::ksprintln!("[syscall] sys_fb_info: fb={:?}", tmp);

    unsafe {
        if copy_to_user(
            user_fb_ptr as *mut u8,
            &tmp as *const _ as *const u8,
            mem::size_of::<FbInfo>(),
        )
        .is_err()
        {
            return -errno::EFAULT;
        }
    }

    crate::ksprintln!("[syscall] sys_fb_info: copy_to_user OK");
    0
}

pub(super) fn sys_fb_map() -> i64 {
    crate::ksprintln!("[syscall] sys_fb_map enter");

    let mut fb = FbInfo {
        addr: 0,
        width: 0,
        height: 0,
        pitch: 0,
        bpp: 0,
    };

    if !fb_info(&mut fb) {
        return -errno::ENODEV;
    }

    if fb.addr == 0 || fb.width == 0 || fb.height == 0 || fb.pitch == 0 {
        return -errno::ENODEV;
    }

    let len = (fb.pitch as u64) * (fb.height as u64);

    let hhdm = HHDM_REQ.get_response().map(|r| r.offset()).unwrap_or(0);

    let fb_phys = if hhdm != 0 && fb.addr >= hhdm {
        fb.addr - hhdm
    } else {
        fb.addr
    };

    crate::ksprintln!(
        "[syscall] sys_fb_map: fb.addr={:#x} hhdm={:#x} -> phys={:#x} len={:#x}",
        fb.addr,
        hhdm,
        fb_phys,
        len
    );

    match mapfb::map_framebuffer_user(fb_phys, len) {
        Ok(user_va) => user_va as i64,
        Err(e) => e,
    }
}
