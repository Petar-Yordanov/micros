use crate::canvas::Canvas;
use font8x8::{UnicodeFonts, BASIC_FONTS};

pub const CHAR_W: i32 = 8;
pub const CHAR_H: i32 = 8;

pub fn measure_text(text: &str) -> i32 {
    (text.chars().count() as i32).saturating_mul(CHAR_W)
}

pub fn draw_char(canvas: &mut Canvas, x: i32, y: i32, fg: u32, bg: Option<u32>, ch: char) {
    let Some(glyph): Option<[u8; 8]> = BASIC_FONTS.get(ch) else {
        return;
    };

    for (row, bits) in glyph.into_iter().enumerate() {
        for col in 0..8usize {
            let on = ((bits >> col) & 1) != 0;
            let px = x + col as i32;
            let py = y + row as i32;

            if on {
                canvas.put_pixel(px, py, fg);
            } else if let Some(bg) = bg {
                canvas.put_pixel(px, py, bg);
            }
        }
    }
}

pub fn draw_text(canvas: &mut Canvas, x: i32, y: i32, fg: u32, bg: Option<u32>, text: &str) {
    let mut xx = x;
    for ch in text.chars() {
        draw_char(canvas, xx, y, fg, bg, ch);
        xx += CHAR_W;
    }
}
