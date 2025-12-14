extern crate alloc;

use core::mem;

use crate::kernel::mm::aspace::user_copy::copy_to_user;
use crate::kernel::syscall::types::FbInfo;
use crate::platform::limine::framebuffer::FRAMEBUFFER_REQ;
use crate::sprintln;

pub fn fb_info(out: &mut FbInfo) -> bool {
    let resp = match FRAMEBUFFER_REQ.get_response() {
        Some(r) => r,
        None => {
            sprintln!("[syscall] fb_info: no framebuffer response from Limine");
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
            sprintln!("[syscall] fb_info: no framebuffers in response");
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

#[allow(unused)]
fn sys_fb_info(user_fb_ptr: u64) -> i64 {
    if user_fb_ptr == 0 {
        return -1;
    }

    let mut tmp = FbInfo {
        addr: 0,
        width: 0,
        height: 0,
        pitch: 0,
        bpp: 0,
    };

    if !fb_info(&mut tmp) {
        return -1;
    }

    unsafe {
        if copy_to_user(
            user_fb_ptr as *mut u8,
            &tmp as *const _ as *const u8,
            mem::size_of::<FbInfo>(),
        )
        .is_err()
        {
            return -1;
        }
    }

    0
}

//pub fn vfs_read(path: &str) -> Result<Vec<u8>, vfs::VfsError> {
//    vfs::vfs_read(path)
//}

//pub fn vfs_write(path: &str, data: &[u8]) -> Result<(), vfs::VfsError> {
//    vfs::vfs_write(path, data)
//}

//pub fn vfs_mkdir_p(path: &str) -> Result<(), vfs::VfsError> {
//    vfs::vfs_mkdir_p(path)
//}

//pub fn vfs_list(path: &str) -> Result<Vec<String>, vfs::VfsError> {
//    vfs::vfs_list(path)
//}

//pub fn heap_stats() -> crate::kernel::syscall::types::HeapStats {
//    crate::kernel::syscall::types::HeapStats {
//        total_pages: vmarena::total_pages(),
//        used_pages: vmarena::used_pages(),
//    }
//}

//pub fn dispatch(nr_no: u64, a0: u64, a1: u64, a2: u64, a3: u64) -> u64 {
//    match nr_no {
//        nr::SYS_FB_INFO => sys_fb_info(a0) as u64,
//        _ => {
//            sprintln!(
//                "[syscall] unknown nr={} (a0={:#x} a1={:#x} a2={:#x} a3={:#x})",
//                nr_no,
//                a0,
//                a1,
//                a2,
//                a3
//            );
//            !0u64
//        }
//    }
//}
