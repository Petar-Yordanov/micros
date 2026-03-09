use crate::canvas::Canvas;
use crate::color::{
    BUTTON_BG, BUTTON_BORDER, BUTTON_DARK_SHADOW, BUTTON_HILITE, BUTTON_HOVER, BUTTON_LIGHT,
    BUTTON_PRESSED, BUTTON_SHADOW, TEXT,
};
use crate::geom::Rect;
use crate::text::{draw_text, measure_text, CHAR_H};

pub fn draw_button(
    canvas: &mut Canvas,
    rect: Rect,
    label: &str,
    hovered: bool,
    pressed: bool,
) {
    let face = if pressed {
        BUTTON_PRESSED
    } else if hovered {
        BUTTON_HOVER
    } else {
        BUTTON_BG
    };

    canvas.fill_rect(rect, face);
    canvas.stroke_rect(rect, BUTTON_BORDER);

    let inner = rect.inset(1, 1);
    if inner.w > 1 && inner.h > 1 {
        if pressed {
            canvas.hline(inner.x, inner.y, inner.w, BUTTON_DARK_SHADOW);
            canvas.vline(inner.x, inner.y, inner.h, BUTTON_DARK_SHADOW);
            canvas.hline(inner.x, inner.bottom() - 1, inner.w, BUTTON_HILITE);
            canvas.vline(inner.right() - 1, inner.y, inner.h, BUTTON_HILITE);
        } else {
            canvas.hline(inner.x, inner.y, inner.w, BUTTON_HILITE);
            canvas.vline(inner.x, inner.y, inner.h, BUTTON_HILITE);
            canvas.hline(inner.x, inner.bottom() - 1, inner.w, BUTTON_SHADOW);
            canvas.vline(inner.right() - 1, inner.y, inner.h, BUTTON_SHADOW);

            let inner2 = rect.inset(2, 2);
            if inner2.w > 1 && inner2.h > 1 {
                canvas.hline(inner2.x, inner2.y, inner2.w, BUTTON_LIGHT);
                canvas.vline(inner2.x, inner2.y, inner2.h, BUTTON_LIGHT);
            }
        }
    }

    let tw = measure_text(label);
    let mut tx = rect.x + ((rect.w - tw) / 2).max(0);
    let mut ty = rect.y + ((rect.h - CHAR_H) / 2).max(0);

    if pressed {
        tx += 1;
        ty += 1;
    }

    draw_text(canvas, tx, ty, TEXT, None, label);
}
