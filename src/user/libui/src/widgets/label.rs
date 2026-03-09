use crate::canvas::Canvas;
use crate::geom::Rect;
use crate::text::{draw_text, CHAR_H};

pub fn draw_label(canvas: &mut Canvas, rect: Rect, text: &str, fg: u32) {
    let y = rect.y + ((rect.h - CHAR_H) / 2).max(0);
    draw_text(canvas, rect.x, y, fg, None, text);
}
