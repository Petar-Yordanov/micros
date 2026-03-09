use crate::canvas::Canvas;
use crate::color::{
    BUTTON_BORDER, PANEL_TEXT, SCROLL_ARROW_BG, SCROLL_THUMB, SCROLL_THUMB_HOVER, SCROLL_TRACK,
};
use crate::event::{MouseButton, UiEvent};
use crate::geom::Rect;
use crate::text::draw_text;

pub const SCROLLBAR_W: i32 = 14;
const SCROLL_BTN_H: i32 = 14;
const SCROLL_MIN_THUMB_H: i32 = 16;

#[derive(Clone, Copy, Debug)]
pub struct ScrollbarState {
    pub offset: i32,
    pub hovered: bool,
    pub dragging: bool,
    drag_anchor_y: i32,
    drag_start_offset: i32,
}

impl ScrollbarState {
    pub const fn new() -> Self {
        Self {
            offset: 0,
            hovered: false,
            dragging: false,
            drag_anchor_y: 0,
            drag_start_offset: 0,
        }
    }

    #[inline]
    pub fn clamp_offset(&mut self, viewport_h: i32, content_h: i32) {
        let max = max_offset(viewport_h, content_h);
        self.offset = self.offset.clamp(0, max);
    }

    #[inline]
    pub fn scroll_pixels(&mut self, viewport_h: i32, content_h: i32, dy: i32) {
        self.offset = self.offset.saturating_add(dy);
        self.clamp_offset(viewport_h, content_h);
    }

    #[inline]
    pub fn scroll_lines(&mut self, viewport_h: i32, content_h: i32, lines: i32, line_h: i32) {
        self.scroll_pixels(viewport_h, content_h, lines.saturating_mul(line_h));
    }
}

#[inline]
pub fn max_offset(viewport_h: i32, content_h: i32) -> i32 {
    (content_h - viewport_h).max(0)
}

#[inline]
pub fn bar_rect(host: Rect) -> Rect {
    Rect::new(host.right() - SCROLLBAR_W, host.y, SCROLLBAR_W, host.h)
}

fn up_button_rect(bar: Rect) -> Rect {
    Rect::new(bar.x, bar.y, bar.w, SCROLL_BTN_H)
}

fn down_button_rect(bar: Rect) -> Rect {
    Rect::new(bar.x, bar.bottom() - SCROLL_BTN_H, bar.w, SCROLL_BTN_H)
}

fn track_rect(bar: Rect) -> Rect {
    Rect::new(
        bar.x,
        bar.y + SCROLL_BTN_H,
        bar.w,
        bar.h - (SCROLL_BTN_H * 2),
    )
}

fn thumb_rect(bar: Rect, viewport_h: i32, content_h: i32, offset: i32) -> Rect {
    let track = track_rect(bar);
    if track.h <= 0 {
        return Rect::new(bar.x, bar.y, bar.w, 0);
    }

    if content_h <= viewport_h || content_h <= 0 {
        return Rect::new(track.x, track.y, track.w, track.h.max(0));
    }

    let mut h = (track.h.saturating_mul(viewport_h))
        .checked_div(content_h)
        .unwrap_or(track.h);
    if h < SCROLL_MIN_THUMB_H {
        h = SCROLL_MIN_THUMB_H;
    }
    if h > track.h {
        h = track.h;
    }

    let movable = (track.h - h).max(0);
    let max_off = max_offset(viewport_h, content_h);
    let y = if max_off == 0 {
        track.y
    } else {
        track.y + (movable.saturating_mul(offset.clamp(0, max_off)) / max_off)
    };

    Rect::new(track.x, y, track.w, h)
}

pub fn draw_v_scrollbar(
    canvas: &mut Canvas,
    host_rect: Rect,
    state: &ScrollbarState,
    viewport_h: i32,
    content_h: i32,
) {
    let bar = bar_rect(host_rect);
    canvas.fill_rect(bar, SCROLL_TRACK);
    canvas.stroke_rect(bar, BUTTON_BORDER);

    let up = up_button_rect(bar);
    let down = down_button_rect(bar);

    canvas.fill_rect(up, SCROLL_ARROW_BG);
    canvas.fill_rect(down, SCROLL_ARROW_BG);
    canvas.stroke_rect(up, BUTTON_BORDER);
    canvas.stroke_rect(down, BUTTON_BORDER);

    draw_text(canvas, up.x + 3, up.y + 3, PANEL_TEXT, None, "^");
    draw_text(canvas, down.x + 3, down.y + 3, PANEL_TEXT, None, "v");

    let thumb = thumb_rect(bar, viewport_h, content_h, state.offset);
    let thumb_color = if state.dragging || state.hovered {
        SCROLL_THUMB_HOVER
    } else {
        SCROLL_THUMB
    };
    canvas.fill_rect(thumb, thumb_color);
    canvas.stroke_rect(thumb, BUTTON_BORDER);
}

pub fn handle_v_scrollbar_event(
    host_rect: Rect,
    state: &mut ScrollbarState,
    viewport_h: i32,
    content_h: i32,
    ev: &UiEvent,
) -> bool {
    let bar = bar_rect(host_rect);
    let up = up_button_rect(bar);
    let down = down_button_rect(bar);
    let track = track_rect(bar);
    let thumb = thumb_rect(bar, viewport_h, content_h, state.offset);

    match *ev {
        UiEvent::MouseMove { pos } => {
            let new_hover = thumb.contains(pos) || bar.contains(pos);
            let mut changed = new_hover != state.hovered;
            state.hovered = new_hover;

            if state.dragging {
                let max_off = max_offset(viewport_h, content_h);
                let movable = (track.h - thumb.h).max(1);
                let dy = pos.y - state.drag_anchor_y;
                let delta = dy.saturating_mul(max_off) / movable;
                let new_offset = state
                    .drag_start_offset
                    .saturating_add(delta)
                    .clamp(0, max_off);
                if new_offset != state.offset {
                    state.offset = new_offset;
                    changed = true;
                }
            }

            changed
        }

        UiEvent::MouseDown {
            pos,
            button: MouseButton::Left,
        } => {
            if !bar.contains(pos) {
                return false;
            }

            if up.contains(pos) {
                state.scroll_pixels(viewport_h, content_h, -16);
                return true;
            }

            if down.contains(pos) {
                state.scroll_pixels(viewport_h, content_h, 16);
                return true;
            }

            if thumb.contains(pos) {
                state.dragging = true;
                state.drag_anchor_y = pos.y;
                state.drag_start_offset = state.offset;
                return true;
            }

            if track.contains(pos) {
                if pos.y < thumb.y {
                    state.scroll_pixels(viewport_h, content_h, -viewport_h.max(16));
                } else if pos.y >= thumb.bottom() {
                    state.scroll_pixels(viewport_h, content_h, viewport_h.max(16));
                }
                return true;
            }

            false
        }

        UiEvent::MouseUp {
            button: MouseButton::Left,
            ..
        } => {
            if state.dragging {
                state.dragging = false;
                return true;
            }
            false
        }

        UiEvent::MouseWheel { pos, delta } => {
            if host_rect.contains(pos) {
                let step = if delta > 0 {
                    -24
                } else if delta < 0 {
                    24
                } else {
                    0
                };
                if step != 0 {
                    state.scroll_pixels(viewport_h, content_h, step);
                    return true;
                }
            }
            false
        }

        _ => false,
    }
}
