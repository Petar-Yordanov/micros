use core::ptr::write_volatile;

use font8x8::{UnicodeFonts, BASIC_FONTS};

use super::console::BootConsole;

impl BootConsole {
    pub fn clear_screen(&mut self, color: u32) {
        let Some(fb) = self.fb else { return };
        self.fill_rect(0, 0, fb.width, fb.height, color);
    }

    pub fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: u32) {
        let Some(fb) = self.fb else { return };
        let x2 = core::cmp::min(x + w, fb.width);
        let y2 = core::cmp::min(y + h, fb.height);

        if x >= x2 || y >= y2 {
            return;
        }

        if fb.bpp == 4 {
            let mut yy = y;
            while yy < y2 {
                let row_ptr = unsafe { fb.base.add(yy * fb.pitch) as *mut u32 };
                let mut xx = x;
                while xx < x2 {
                    unsafe {
                        write_volatile(row_ptr.add(xx), color);
                    }
                    xx += 1;
                }
                yy += 1;
            }
        } else {
            let mut yy = y;
            while yy < y2 {
                let mut xx = x;
                while xx < x2 {
                    self.put_px(xx, yy, color);
                    xx += 1;
                }
                yy += 1;
            }
        }
    }

    pub fn stroke_rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: u32) {
        if w < 2 || h < 2 {
            return;
        }
        self.fill_rect(x, y, w, 1, color);
        self.fill_rect(x, y + h - 1, w, 1, color);
        self.fill_rect(x, y, 1, h, color);
        self.fill_rect(x + w - 1, y, 1, h, color);
    }

    pub fn draw_text(&mut self, x: usize, y: usize, s: &str, color: u32) {
        let mut cx = x;
        for ch in s.chars() {
            self.draw_char(cx, y, ch, color);
            cx += super::console::FONT_W;
        }
    }

    pub fn draw_char(&mut self, x: usize, y: usize, ch: char, color: u32) {
        let Some(glyph) = BASIC_FONTS.get(ch) else {
            return;
        };

        let mut gy = 0usize;
        while gy < 8 {
            let row = glyph[gy];
            let mut gx = 0usize;
            while gx < 8 {
                if (row >> gx) & 1 != 0 {
                    self.put_px(x + gx, y + gy, color);
                }
                gx += 1;
            }
            gy += 1;
        }
    }

    pub fn put_px(&mut self, x: usize, y: usize, color: u32) {
        let Some(fb) = self.fb else { return };
        if x >= fb.width || y >= fb.height {
            return;
        }

        let off = y * fb.pitch + x * fb.bpp;
        unsafe {
            if fb.bpp == 4 {
                write_volatile(fb.base.add(off) as *mut u32, color);
            } else {
                let p = fb.base.add(off);
                write_volatile(p, (color & 0xFF) as u8);
                write_volatile(p.add(1), ((color >> 8) & 0xFF) as u8);
                write_volatile(p.add(2), ((color >> 16) & 0xFF) as u8);
            }
        }
    }
}
