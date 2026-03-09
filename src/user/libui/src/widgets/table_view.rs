use crate::canvas::Canvas;
use crate::color::{
    BUTTON_BORDER, LIST_BG, LIST_ROW_HOVER, LIST_ROW_SELECTED, TABLE_GRID, TABLE_HEADER_BG, TEXT,
};
use crate::event::{MouseButton, UiEvent};
use crate::geom::Rect;
use crate::text::{draw_text, CHAR_H};
use crate::widgets::scroll::{
    draw_v_scrollbar, handle_v_scrollbar_event, max_offset, ScrollbarState, SCROLLBAR_W,
};

pub const TABLE_HEADER_H: i32 = 20;
pub const TABLE_ROW_H: i32 = 18;

#[derive(Clone, Copy)]
pub struct TableColumn<'a> {
    pub title: &'a str,
    pub width: i32,
}

#[derive(Clone, Copy)]
pub struct TableRow<'a> {
    pub cells: &'a [&'a str],
}

#[derive(Clone, Copy, Debug)]
pub struct TableViewState {
    pub selected: Option<usize>,
    pub hovered: Option<usize>,
    pub scroll: ScrollbarState,
}

impl TableViewState {
    pub const fn new() -> Self {
        Self {
            selected: None,
            hovered: None,
            scroll: ScrollbarState::new(),
        }
    }
}

fn header_rect(rect: Rect) -> Rect {
    Rect::new(
        rect.x + 1,
        rect.y + 1,
        rect.w - SCROLLBAR_W - 2,
        TABLE_HEADER_H,
    )
}

fn rows_rect(rect: Rect) -> Rect {
    Rect::new(
        rect.x + 1,
        rect.y + 1 + TABLE_HEADER_H,
        rect.w - SCROLLBAR_W - 2,
        rect.h - TABLE_HEADER_H - 2,
    )
}

fn row_from_local_y(local_y: i32, offset: i32, count: usize) -> Option<usize> {
    if local_y < 0 {
        return None;
    }
    let idx = ((local_y + offset) / TABLE_ROW_H) as usize;
    if idx < count {
        Some(idx)
    } else {
        None
    }
}

pub fn draw_table_view(
    canvas: &mut Canvas,
    rect: Rect,
    columns: &[TableColumn<'_>],
    rows: &[TableRow<'_>],
    state: &TableViewState,
) {
    canvas.fill_rect(rect, LIST_BG);
    canvas.stroke_rect(rect, BUTTON_BORDER);

    let hdr = header_rect(rect);
    let body = rows_rect(rect);

    canvas.fill_rect(hdr, TABLE_HEADER_BG);
    canvas.stroke_rect(hdr, TABLE_GRID);

    let mut x = hdr.x;
    for col in columns.iter() {
        let cell = Rect::new(x, hdr.y, col.width, hdr.h);
        canvas.stroke_rect(cell, TABLE_GRID);
        let ty = cell.y + ((cell.h - CHAR_H) / 2).max(0);
        draw_text(canvas, cell.x + 4, ty, TEXT, None, col.title);
        x += col.width;
    }

    let content_h = (rows.len() as i32).saturating_mul(TABLE_ROW_H);
    let off = state.scroll.offset.clamp(0, max_offset(body.h, content_h));
    let start = (off / TABLE_ROW_H).max(0) as usize;
    let end = ((off + body.h + TABLE_ROW_H - 1) / TABLE_ROW_H).max(0) as usize;

    for row_idx in start..rows.len().min(end) {
        let y = body.y + (row_idx as i32 * TABLE_ROW_H) - off;
        let row_rect = Rect::new(body.x, y, body.w, TABLE_ROW_H);

        if state.selected == Some(row_idx) {
            canvas.fill_rect(row_rect, LIST_ROW_SELECTED);
        } else if state.hovered == Some(row_idx) {
            canvas.fill_rect(row_rect, LIST_ROW_HOVER);
        }

        let mut cx = row_rect.x;
        for (col_idx, col) in columns.iter().enumerate() {
            let cell = Rect::new(cx, row_rect.y, col.width, row_rect.h);
            canvas.stroke_rect(cell, TABLE_GRID);
            let ty = cell.y + ((cell.h - CHAR_H) / 2).max(0);
            let text = rows[row_idx].cells.get(col_idx).copied().unwrap_or("");
            draw_text(canvas, cell.x + 4, ty, TEXT, None, text);
            cx += col.width;
        }
    }

    draw_v_scrollbar(canvas, rect, &state.scroll, body.h, content_h);
}

pub fn handle_table_view_event(
    rect: Rect,
    rows_len: usize,
    state: &mut TableViewState,
    ev: &UiEvent,
) -> bool {
    let body = rows_rect(rect);
    let content_h = (rows_len as i32).saturating_mul(TABLE_ROW_H);
    let mut changed = handle_v_scrollbar_event(rect, &mut state.scroll, body.h, content_h, ev);

    match *ev {
        UiEvent::MouseMove { pos } => {
            let old = state.hovered;
            if body.contains(pos) {
                state.hovered = row_from_local_y(pos.y - body.y, state.scroll.offset, rows_len);
            } else {
                state.hovered = None;
            }
            changed |= old != state.hovered;
        }

        UiEvent::MouseDown {
            pos,
            button: MouseButton::Left,
        } => {
            if body.contains(pos) {
                let idx = row_from_local_y(pos.y - body.y, state.scroll.offset, rows_len);
                if idx != state.selected {
                    state.selected = idx;
                    changed = true;
                }
            }
        }

        _ => {}
    }

    changed
}
