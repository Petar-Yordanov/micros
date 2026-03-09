use crate::canvas::Canvas;
use crate::color::{BUTTON_HILITE, BUTTON_SHADOW, PANEL, PANEL_BORDER};
use crate::geom::Rect;

#[inline]
pub fn draw_panel(canvas: &mut Canvas, rect: Rect) {
    draw_panel_with(canvas, rect, PANEL, PANEL_BORDER);
}

#[inline]
pub fn draw_panel_with(canvas: &mut Canvas, rect: Rect, bg: u32, border: u32) {
    canvas.fill_rect(rect, bg);
    canvas.stroke_rect(rect, border);

    let inner = rect.inset(1, 1);
    if inner.w > 1 && inner.h > 1 {
        canvas.hline(inner.x, inner.y, inner.w, BUTTON_HILITE);
        canvas.vline(inner.x, inner.y, inner.h, BUTTON_HILITE);
        canvas.hline(inner.x, inner.bottom() - 1, inner.w, BUTTON_SHADOW);
        canvas.vline(inner.right() - 1, inner.y, inner.h, BUTTON_SHADOW);
    }
}

#[inline]
pub fn inner_rect(rect: Rect, pad: i32) -> Rect {
    rect.inset(pad, pad)
}
