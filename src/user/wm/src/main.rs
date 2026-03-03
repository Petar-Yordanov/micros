#![no_std]
#![no_main]

use core::fmt::Write;

use rlibc::log::log;
use rlibc::fb::{fb_info, fb_map};
use rlibc::input::next_event;

use micros_abi::types::{AbiInputEvent, FbInfo, ABI_IN_KIND_KEY, ABI_IN_KIND_REL, ABI_IN_KIND_SYN};

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    let _ = log("wm: panic\n");
    loop {}
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let _ = log("wm: starting\n");

    let mut fb = FbInfo::default();
    let r = fb_info(&mut fb);
    if r < 0 || fb.width == 0 || fb.height == 0 || fb.pitch == 0 {
        let _ = log("wm: fb_info failed/invalid\n");
        loop {}
    }

    let fb_user_va = fb_map();
    if fb_user_va < 0 {
        let _ = log("wm: fb_map failed (SYS_FB_MAP)\n");
        loop {}
    }

    let fb_ptr = fb_user_va as *mut u32;
    let w = fb.width as usize;
    let h = fb.height as usize;
    let pitch_pixels = (fb.pitch as usize) / 4;

    const BG: u32 = 0x00101010;
    const PANEL: u32 = 0x00202040;
    const TEXT: u32 = 0x00E0E0E0;
    const CUR: u32 = 0x00FFFFFF;

    fill_rect(fb_ptr, pitch_pixels, 0, 0, w, h, BG, w, h);
    let panel_w = core::cmp::min(260, w);
    let panel_h = core::cmp::min(140, h);
    fill_rect(fb_ptr, pitch_pixels, 0, 0, panel_w, panel_h, PANEL, w, h);

    let _ = log("wm: fb mapped; entering loop\n");

    unsafe {
        let n = core::cmp::min(w, h);
        for i in 0..n {
            *fb_ptr.add(i * pitch_pixels + i) = 0x00FF0000; // red
        }
    }

    let mut cx: i32 = (w / 2) as i32;
    let mut cy: i32 = (h / 2) as i32;
    let mut prev_cx = cx;
    let mut prev_cy = cy;

    let mut rel_dx: i32 = 0;
    let mut rel_dy: i32 = 0;

    let mut last_key_code: u16 = 0;
    let mut last_btn_code: u16 = 0;

    let mut pending_btn: Option<(u16, bool)> = None;

    let mut overlay_dirty = true;

    draw_cursor(fb_ptr, pitch_pixels, cx as usize, cy as usize, CUR, w, h);

    let mut ev = AbiInputEvent::default();
    loop {
        loop {
            let r = next_event(&mut ev);
            if r == -11 {
                break; // EAGAIN (TODO: Substitute with micros_abi errno values)
            }
            if r < 0 {
                let _ = log("wm: input_next_event error\n");
                break;
            }

            match ev.kind {
                ABI_IN_KIND_KEY => {
                    if (0x110..=0x11f).contains(&ev.code) {
                        pending_btn = Some((ev.code, ev.value != 0));
                    } else {
                        if ev.value == 1 || ev.value == 2 {
                            last_key_code = ev.code;
                            overlay_dirty = true;
                        }
                    }
                }

                ABI_IN_KIND_REL => {
                    match ev.code {
                        0x00 => rel_dx = rel_dx.saturating_add(ev.value),
                        0x01 => rel_dy = rel_dy.saturating_add(ev.value),
                        _ => {}
                    }
                }

                ABI_IN_KIND_SYN => {
                    if let Some((btn, pressed)) = pending_btn.take() {
                        if pressed {
                            last_btn_code = btn;
                            overlay_dirty = true;
                        }
                    }

                    if rel_dx != 0 || rel_dy != 0 {
                        cx = (cx + rel_dx).clamp(0, (w as i32).saturating_sub(1));
                        cy = (cy + rel_dy).clamp(0, (h as i32).saturating_sub(1));
                        rel_dx = 0;
                        rel_dy = 0;
                        overlay_dirty = true;
                    }
                }

                _ => {}
            }
        }

        if overlay_dirty {
            const MARGIN: usize = 8;
            const LINE_H: usize = 10;

            let box_w = (26 * 8) + (MARGIN * 2);
            let box_h = (LINE_H * 3) + (MARGIN * 2);

            let box_x = w.saturating_sub(box_w);
            let box_y = 0;

            fill_rect(fb_ptr, pitch_pixels, box_x, box_y, box_w, box_h, BG, w, h);

            let mut b1 = LineBuf::new();
            let mut b2 = LineBuf::new();
            let mut b3 = LineBuf::new();

            let _ = write!(&mut b1, "X={} Y={}", cx, cy);
            let _ = write!(&mut b2, "K={}", last_key_code);
            let _ = write!(&mut b3, "B={}", last_btn_code);

            let right_x = w.saturating_sub(MARGIN);

            draw_text_right(fb_ptr, pitch_pixels, w, h, right_x, 8, b1.as_str(), TEXT, BG);
            draw_text_right(
                fb_ptr,
                pitch_pixels,
                w,
                h,
                right_x,
                8 + LINE_H,
                b2.as_str(),
                TEXT,
                BG,
            );
            draw_text_right(
                fb_ptr,
                pitch_pixels,
                w,
                h,
                right_x,
                8 + (LINE_H * 2),
                b3.as_str(),
                TEXT,
                BG,
            );

            overlay_dirty = false;
        }

        // Move cursor if needed
        if cx != prev_cx || cy != prev_cy {
            erase_cursor(
                fb_ptr,
                pitch_pixels,
                prev_cx as usize,
                prev_cy as usize,
                BG,
                PANEL,
                panel_w,
                panel_h,
                w,
                h,
            );

            draw_cursor(fb_ptr, pitch_pixels, cx as usize, cy as usize, CUR, w, h);

            prev_cx = cx;
            prev_cy = cy;
        }

        spin_delay(25_000);
    }
}

struct LineBuf {
    buf: [u8; 96],
    len: usize,
}

impl LineBuf {
    const fn new() -> Self {
        Self { buf: [0u8; 96], len: 0 }
    }

    fn as_str(&self) -> &str {
        unsafe { core::str::from_utf8_unchecked(&self.buf[..self.len]) }
    }
}

impl core::fmt::Write for LineBuf {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let bytes = s.as_bytes();
        let cap = self.buf.len();
        let n = core::cmp::min(bytes.len(), cap.saturating_sub(self.len));
        self.buf[self.len..self.len + n].copy_from_slice(&bytes[..n]);
        self.len += n;
        Ok(())
    }
}

#[inline(always)]
fn spin_delay(mut n: u32) {
    while n > 0 {
        core::hint::spin_loop();
        n -= 1;
    }
}

#[inline(always)]
fn fill_rect(
    fb: *mut u32,
    pitch: usize,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    color: u32,
    screen_w: usize,
    screen_h: usize,
) {
    let x2 = (x + w).min(screen_w);
    let y2 = (y + h).min(screen_h);
    if x >= x2 || y >= y2 {
        return;
    }
    for yy in y..y2 {
        unsafe {
            let row = core::slice::from_raw_parts_mut(fb.add(yy * pitch + x), x2 - x);
            row.fill(color);
        }
    }
}

#[inline(always)]
fn draw_cursor(fb: *mut u32, pitch: usize, x: usize, y: usize, color: u32, w: usize, h: usize) {
    for yy in y..(y + 8).min(h) {
        for xx in x..(x + 8).min(w) {
            unsafe { *fb.add(yy * pitch + xx) = color; }
        }
    }
}

#[inline(always)]
fn erase_cursor(
    fb: *mut u32,
    pitch: usize,
    x: usize,
    y: usize,
    bg: u32,
    panel: u32,
    panel_w: usize,
    panel_h: usize,
    w: usize,
    h: usize,
) {
    for yy in y..(y + 8).min(h) {
        for xx in x..(x + 8).min(w) {
            let under = if xx < panel_w && yy < panel_h { panel } else { bg };
            unsafe { *fb.add(yy * pitch + xx) = under; }
        }
    }
}

fn glyph(ch: u8) -> [u8; 8] {
    match ch {
        b'0' => [0x3C,0x66,0x6E,0x76,0x66,0x66,0x3C,0x00],
        b'1' => [0x18,0x38,0x18,0x18,0x18,0x18,0x7E,0x00],
        b'2' => [0x3C,0x66,0x06,0x1C,0x30,0x60,0x7E,0x00],
        b'3' => [0x3C,0x66,0x06,0x1C,0x06,0x66,0x3C,0x00],
        b'4' => [0x0C,0x1C,0x3C,0x6C,0x7E,0x0C,0x0C,0x00],
        b'5' => [0x7E,0x60,0x7C,0x06,0x06,0x66,0x3C,0x00],
        b'6' => [0x1C,0x30,0x60,0x7C,0x66,0x66,0x3C,0x00],
        b'7' => [0x7E,0x06,0x0C,0x18,0x30,0x30,0x30,0x00],
        b'8' => [0x3C,0x66,0x66,0x3C,0x66,0x66,0x3C,0x00],
        b'9' => [0x3C,0x66,0x66,0x3E,0x06,0x0C,0x38,0x00],

        b'X' => [0x66,0x66,0x3C,0x18,0x3C,0x66,0x66,0x00],
        b'Y' => [0x66,0x66,0x3C,0x18,0x18,0x18,0x3C,0x00],
        b'K' => [0x66,0x6C,0x78,0x70,0x78,0x6C,0x66,0x00],
        b'B' => [0x7C,0x66,0x66,0x7C,0x66,0x66,0x7C,0x00],

        b'=' => [0x00,0x00,0x7E,0x00,0x7E,0x00,0x00,0x00],
        b'-' => [0x00,0x00,0x00,0x7E,0x00,0x00,0x00,0x00],
        b' ' => [0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00],
        _ => [0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00],
    }
}

#[inline(always)]
fn draw_char(fb: *mut u32, pitch: usize, x: usize, y: usize, fg: u32, bg: u32, ch: u8) {
    let g = glyph(ch);
    for row in 0..8usize {
        let bits = g[row];
        for col in 0..8usize {
            let on = ((bits >> (7 - col)) & 1) != 0;
            unsafe {
                *fb.add((y + row) * pitch + (x + col)) = if on { fg } else { bg };
            }
        }
    }
}

#[inline(always)]
fn draw_text_right(
    fb: *mut u32,
    pitch: usize,
    screen_w: usize,
    screen_h: usize,
    right_x: usize,
    y: usize,
    text: &str,
    fg: u32,
    bg: u32,
) {
    if y + 8 > screen_h {
        return;
    }
    let bytes = text.as_bytes();
    let px_w = bytes.len().saturating_mul(8);
    let start_x = right_x.saturating_sub(px_w);

    let mut x = start_x;
    for &b in bytes {
        if x + 8 > screen_w {
            break;
        }
        draw_char(fb, pitch, x, y, fg, bg, b);
        x += 8;
    }
}
