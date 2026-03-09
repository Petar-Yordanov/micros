use micros_abi::types::FbInfo;
use rlibc::fb::{fb_info, fb_map};

pub struct Framebuffer {
    pub ptr: *mut u32,
    pub width: usize,
    pub height: usize,
    pub pitch_pixels: usize,
}

pub fn map_framebuffer() -> Result<Framebuffer, &'static str> {
    let mut fb = FbInfo {
        addr: 0,
        width: 0,
        height: 0,
        pitch: 0,
        bpp: 0,
    };

    let r = fb_info(&mut fb);
    if r < 0 || fb.addr == 0 || fb.width == 0 || fb.height == 0 || fb.pitch == 0 {
        return Err("wm: fb_info failed");
    }

    let fb_user_va = fb_map();
    if fb_user_va < 0 {
        return Err("wm: fb_map failed");
    }

    Ok(Framebuffer {
        ptr: fb_user_va as *mut u32,
        width: fb.width as usize,
        height: fb.height as usize,
        pitch_pixels: (fb.pitch as usize) / 4,
    })
}
