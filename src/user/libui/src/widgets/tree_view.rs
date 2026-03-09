use crate::canvas::Canvas;
use crate::color::{
    BUTTON_BORDER, LIST_BG, LIST_ROW_HOVER, LIST_ROW_SELECTED, TEXT, TREE_GLYPH,
};
use crate::event::{MouseButton, UiEvent};
use crate::geom::Rect;
use crate::text::{draw_text, CHAR_H};
use crate::widgets::scroll::{
    draw_v_scrollbar, handle_v_scrollbar_event, max_offset, ScrollbarState, SCROLLBAR_W,
};

pub const TREE_ROW_H: i32 = 18;
const TREE_INDENT_W: i32 = 14;
const TREE_GLYPH_W: i32 = 10;

#[derive(Clone, Copy)]
pub struct TreeRow<'a> {
    pub id: u32,
    pub label: &'a str,
    pub depth: u8,
    pub has_children: bool,
    pub expanded: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct TreeViewState {
    pub selected_id: Option<u32>,
    pub hovered_row: Option<usize>,
    pub scroll: ScrollbarState,
}

impl TreeViewState {
    pub const fn new() -> Self {
        Self {
            selected_id: None,
            hovered_row: None,
            scroll: ScrollbarState::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TreeViewAction {
    Select(u32),
    Toggle(u32),
}

fn content_rect(rect: Rect) -> Rect {
    Rect::new(rect.x + 1, rect.y + 1, rect.w - SCROLLBAR_W - 2, rect.h - 2)
}

fn row_from_local_y(local_y: i32, offset: i32, count: usize) -> Option<usize> {
    if local_y < 0 {
        return None;
    }
    let idx = ((local_y + offset) / TREE_ROW_H) as usize;
    if idx < count {
        Some(idx)
    } else {
        None
    }
}

pub fn draw_tree_view(
    canvas: &mut Canvas,
    rect: Rect,
    rows: &[TreeRow<'_>],
    state: &TreeViewState,
) {
    canvas.fill_rect(rect, LIST_BG);
    canvas.stroke_rect(rect, BUTTON_BORDER);

    let content = content_rect(rect);
    let content_h = (rows.len() as i32).saturating_mul(TREE_ROW_H);
    let off = state.scroll.offset.clamp(0, max_offset(content.h, content_h));

    let start = (off / TREE_ROW_H).max(0) as usize;
    let end = ((off + content.h + TREE_ROW_H - 1) / TREE_ROW_H).max(0) as usize;

    for idx in start..rows.len().min(end) {
        let y = content.y + (idx as i32 * TREE_ROW_H) - off;
        let row = Rect::new(content.x, y, content.w, TREE_ROW_H);

        if state.selected_id == Some(rows[idx].id) {
            canvas.fill_rect(row, LIST_ROW_SELECTED);
        } else if state.hovered_row == Some(idx) {
            canvas.fill_rect(row, LIST_ROW_HOVER);
        }

        let indent = rows[idx].depth as i32 * TREE_INDENT_W;
        let glyph_x = row.x + 4 + indent;
        let glyph_y = row.y + ((row.h - CHAR_H) / 2).max(0);

        if rows[idx].has_children {
            draw_text(
                canvas,
                glyph_x,
                glyph_y,
                TREE_GLYPH,
                None,
                if rows[idx].expanded { "-" } else { "+" },
            );
        }

        draw_text(
            canvas,
            glyph_x + TREE_GLYPH_W,
            glyph_y,
            TEXT,
            None,
            rows[idx].label,
        );
    }

    draw_v_scrollbar(canvas, rect, &state.scroll, content.h, content_h);
}

pub fn handle_tree_view_event(
    rect: Rect,
    rows: &[TreeRow<'_>],
    state: &mut TreeViewState,
    ev: &UiEvent,
) -> (bool, Option<TreeViewAction>) {
    let content = content_rect(rect);
    let content_h = (rows.len() as i32).saturating_mul(TREE_ROW_H);
    let mut changed =
        handle_v_scrollbar_event(rect, &mut state.scroll, content.h, content_h, ev);
    let mut action = None;

    match *ev {
        UiEvent::MouseMove { pos } => {
            let old = state.hovered_row;
            if content.contains(pos) {
                state.hovered_row = row_from_local_y(pos.y - content.y, state.scroll.offset, rows.len());
            } else {
                state.hovered_row = None;
            }
            changed |= old != state.hovered_row;
        }

        UiEvent::MouseDown {
            pos,
            button: MouseButton::Left,
        } => {
            if content.contains(pos) {
                if let Some(idx) =
                    row_from_local_y(pos.y - content.y, state.scroll.offset, rows.len())
                {
                    let row = rows[idx];
                    let row_y = content.y + (idx as i32 * TREE_ROW_H) - state.scroll.offset;
                    let indent = row.depth as i32 * TREE_INDENT_W;
                    let glyph = Rect::new(content.x + 4 + indent, row_y + 3, TREE_GLYPH_W, 12);

                    if row.has_children && glyph.contains(pos) {
                        action = Some(TreeViewAction::Toggle(row.id));
                        changed = true;
                    } else {
                        if state.selected_id != Some(row.id) {
                            state.selected_id = Some(row.id);
                            changed = true;
                        }
                        action = Some(TreeViewAction::Select(row.id));
                    }
                }
            }
        }

        _ => {}
    }

    (changed, action)
}
