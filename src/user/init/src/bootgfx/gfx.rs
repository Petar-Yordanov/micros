use core::ptr::write_volatile;
use micros_abi::types::FbInfo;

use super::font::glyph_5x7;

pub struct Surface<'a> {
    pub info: FbInfo,
    pub buf: &'a mut [u8],
}

impl<'a> Surface<'a> {
    pub fn width(&self) -> usize {
        self.info.width as usize
    }

    pub fn height(&self) -> usize {
        self.info.height as usize
    }

    pub fn pitch(&self) -> usize {
        self.info.pitch as usize
    }

    pub fn bpp(&self) -> usize {
        (self.info.bpp as usize) / 8
    }

    pub fn clear(&mut self, color: u32) {
        self.fill_rect(0, 0, self.width(), self.height(), color);
    }

    pub fn put_px(&mut self, x: usize, y: usize, color: u32) {
        if x >= self.width() || y >= self.height() {
            return;
        }

        let bpp = self.bpp();
        if bpp < 3 {
            return;
        }

        let off = y * self.pitch() + x * bpp;
        if off + bpp > self.buf.len() {
            return;
        }

        unsafe {
            let p = self.buf.as_mut_ptr().add(off);
            write_volatile(p, (color & 0xFF) as u8);
            write_volatile(p.add(1), ((color >> 8) & 0xFF) as u8);
            write_volatile(p.add(2), ((color >> 16) & 0xFF) as u8);
            if bpp >= 4 {
                write_volatile(p.add(3), 0);
            }
        }
    }

    pub fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: u32) {
        let x2 = core::cmp::min(x + w, self.width());
        let y2 = core::cmp::min(y + h, self.height());

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

    pub fn stroke_rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: u32) {
        if w < 2 || h < 2 {
            return;
        }
        self.fill_rect(x, y, w, 1, color);
        self.fill_rect(x, y + h - 1, w, 1, color);
        self.fill_rect(x, y, 1, h, color);
        self.fill_rect(x + w - 1, y, 1, h, color);
    }

    pub fn draw_glyph_5x7(
        &mut self,
        x: usize,
        y: usize,
        glyph: [u8; 7],
        scale: usize,
        color: u32,
    ) {
        let mut row = 0usize;
        while row < 7 {
            let bits = glyph[row];
            let mut col = 0usize;
            while col < 5 {
                if ((bits >> (4 - col)) & 1) != 0 {
                    self.fill_rect(x + col * scale, y + row * scale, scale, scale, color);
                }
                col += 1;
            }
            row += 1;
        }
    }

    pub fn draw_text_5x7(&mut self, mut x: usize, y: usize, s: &str, scale: usize, color: u32) {
        for ch in s.chars() {
            if ch == ' ' {
                x += 4 * scale;
                continue;
            }

            if let Some(g) = glyph_5x7(ch) {
                self.draw_glyph_5x7(x, y, g, scale, color);
            }
            x += 6 * scale;
        }
    }
}
