use x86_64::{PhysAddr, VirtAddr};

use crate::kernel::mm::map::mapper as page;
use crate::kernel::mm::map::mapper::Prot;
use micros_abi::errno;

const USER_FB_BASE: u64 = 0x0000_6000_0000_0000;

#[inline(always)]
fn align_down(x: u64, a: u64) -> u64 {
    x & !(a - 1)
}

#[inline(always)]
fn align_up(x: u64, a: u64) -> u64 {
    (x + (a - 1)) & !(a - 1)
}

#[inline(always)]
fn user_rw_prot() -> Prot {
    Prot::UserRW
}

pub fn map_framebuffer_user(fb_phys: u64, fb_len: u64) -> Result<u64, i64> {
    if fb_phys == 0 || fb_len == 0 {
        return Err(-errno::ENODEV);
    }

    let page_sz = 4096u64;

    let phys_start = align_down(fb_phys, page_sz);
    let phys_end = align_up(fb_phys + fb_len, page_sz);
    let map_len = phys_end - phys_start;
    let pages = (map_len / page_sz) as usize;

    let mut va = VirtAddr::new(USER_FB_BASE);
    let prot = user_rw_prot();

    for i in 0..pages {
        let pa = PhysAddr::new(phys_start + (i as u64) * page_sz);
        let pf = x86_64::structures::paging::PhysFrame::containing_address(pa);

        page::map_fixed(va, pf, prot).map_err(|_| -errno::ENOMEM)?;
        va += page_sz;
    }

    let delta = fb_phys - phys_start;
    Ok(USER_FB_BASE + delta)
}
