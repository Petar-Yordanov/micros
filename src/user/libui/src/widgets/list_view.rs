use crate::canvas::Canvas;
use crate::color::{BUTTON_BORDER, LIST_BG, LIST_ROW_HOVER, LIST_ROW_SELECTED, TEXT};
use crate::event::{MouseButton, UiEvent};
use crate::geom::Rect;
use crate::text::{draw_text, CHAR_H};
use crate::widgets::scroll::{
    draw_v_scrollbar, handle_v_scrollbar_event, max_offset, ScrollbarState, SCROLLBAR_W,
};

pub const LIST_ROW_H: i32 = 20;

#[derive(Clone, Copy, Debug)]
pub struct ListViewState {
    pub selected: Option<usize>,
    pub hovered: Option<usize>,
    pub scroll: ScrollbarState,
}

impl ListViewState {
    pub const fn new() -> Self {
        Self {
            selected: None,
            hovered: None,
            scroll: ScrollbarState::new(),
        }
    }
}

fn content_rect(rect: Rect) -> Rect {
    Rect::new(rect.x + 1, rect.y + 1, rect.w - SCROLLBAR_W - 2, rect.h - 2)
}

fn row_from_local_y(local_y: i32, offset: i32, count: usize) -> Option<usize> {
    if local_y < 0 {
        return None;
    }
    let idx = ((local_y + offset) / LIST_ROW_H) as usize;
    if idx < count {
        Some(idx)
    } else {
        None
    }
}

pub fn draw_list_view(canvas: &mut Canvas, rect: Rect, items: &[&str], state: &ListViewState) {
    canvas.fill_rect(rect, LIST_BG);
    canvas.stroke_rect(rect, BUTTON_BORDER);

    let content = content_rect(rect);
    let content_h = (items.len() as i32).saturating_mul(LIST_ROW_H);
    let off = state.scroll.offset.clamp(0, max_offset(content.h, content_h));

    let start = (off / LIST_ROW_H).max(0) as usize;
    let end = ((off + content.h + LIST_ROW_H - 1) / LIST_ROW_H).max(0) as usize;

    for idx in start..items.len().min(end) {
        let y = content.y + (idx as i32 * LIST_ROW_H) - off;
        let row = Rect::new(content.x, y, content.w, LIST_ROW_H);

        if state.selected == Some(idx) {
            canvas.fill_rect(row, LIST_ROW_SELECTED);
        } else if state.hovered == Some(idx) {
            canvas.fill_rect(row, LIST_ROW_HOVER);
        }

        let ty = row.y + ((row.h - CHAR_H) / 2).max(0);
        draw_text(canvas, row.x + 6, ty, TEXT, None, items[idx]);
    }

    draw_v_scrollbar(canvas, rect, &state.scroll, content.h, content_h);
}

pub fn handle_list_view_event(
    rect: Rect,
    items_len: usize,
    state: &mut ListViewState,
    ev: &UiEvent,
) -> bool {
    let content = content_rect(rect);
    let content_h = (items_len as i32).saturating_mul(LIST_ROW_H);
    let mut changed =
        handle_v_scrollbar_event(rect, &mut state.scroll, content.h, content_h, ev);

    match *ev {
        UiEvent::MouseMove { pos } => {
            let old = state.hovered;
            if content.contains(pos) {
                state.hovered = row_from_local_y(pos.y - content.y, state.scroll.offset, items_len);
            } else {
                state.hovered = None;
            }
            changed |= old != state.hovered;
        }

        UiEvent::MouseDown {
            pos,
            button: MouseButton::Left,
        } => {
            if content.contains(pos) {
                let idx = row_from_local_y(pos.y - content.y, state.scroll.offset, items_len);
                if idx != state.selected {
                    state.selected = idx;
                    changed = true;
                }
            }
        }

        UiEvent::MouseWheel { pos, .. } => {
            if rect.contains(pos) {
                state.scroll.clamp_offset(content.h, content_h);
            }
        }

        _ => {}
    }

    changed
}
