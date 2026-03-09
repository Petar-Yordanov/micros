use crate::canvas::Canvas;
use crate::color::{MENU_BG, MENU_BORDER, MENU_HOVER, TEXT};
use crate::event::{MouseButton, UiEvent};
use crate::geom::Rect;
use crate::text::draw_text;
use crate::widgets::label::draw_label;

pub const MENU_ITEM_H: i32 = 22;
pub const MENU_W: i32 = 150;
pub const SUBMENU_W: i32 = 140;

#[derive(Clone, Copy)]
pub struct MenuSubItem<'a> {
    pub label: &'a str,
    pub action: u16,
}

#[derive(Clone, Copy)]
pub struct MenuItem<'a> {
    pub label: &'a str,
    pub submenu: &'a [MenuSubItem<'a>],
}

#[derive(Clone, Copy, Debug)]
pub struct MenuState {
    pub open: bool,
    pub hover: Option<usize>,
    pub sub_hover: Option<usize>,
}

impl MenuState {
    pub const fn new() -> Self {
        Self {
            open: false,
            hover: None,
            sub_hover: None,
        }
    }

    pub fn close(&mut self) {
        self.open = false;
        self.hover = None;
        self.sub_hover = None;
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MenuOutcome {
    pub changed: bool,
    pub action: Option<u16>,
}

fn menu_rect(anchor: Rect, items_len: usize) -> Rect {
    Rect::new(
        anchor.x,
        anchor.bottom() + 2,
        MENU_W,
        (items_len as i32 * MENU_ITEM_H) + 8,
    )
}

fn menu_item_rect(menu: Rect, idx: usize) -> Rect {
    Rect::new(
        menu.x + 4,
        menu.y + 4 + (idx as i32 * MENU_ITEM_H),
        menu.w - 8,
        MENU_ITEM_H,
    )
}

fn submenu_rect(menu: Rect, item_idx: usize, sub_len: usize) -> Rect {
    let item = menu_item_rect(menu, item_idx);
    Rect::new(
        item.right() + 4,
        item.y,
        SUBMENU_W,
        (sub_len as i32 * MENU_ITEM_H) + 8,
    )
}

fn submenu_item_rect(sub: Rect, sub_idx: usize) -> Rect {
    Rect::new(
        sub.x + 4,
        sub.y + 4 + (sub_idx as i32 * MENU_ITEM_H),
        sub.w - 8,
        MENU_ITEM_H,
    )
}

pub fn draw_popup_menu(
    canvas: &mut Canvas,
    anchor: Rect,
    state: &MenuState,
    items: &[MenuItem<'_>],
) {
    if !state.open {
        return;
    }

    let menu = menu_rect(anchor, items.len());
    canvas.fill_rect(menu, MENU_BG);
    canvas.stroke_rect(menu, MENU_BORDER);

    for (i, item) in items.iter().enumerate() {
        let r = menu_item_rect(menu, i);
        if state.hover == Some(i) {
            canvas.fill_rect(r, MENU_HOVER);
        }
        draw_label(
            canvas,
            Rect::new(r.x + 6, r.y, r.w - 20, r.h),
            item.label,
            TEXT,
        );
        draw_text(canvas, r.right() - 14, r.y + 7, TEXT, None, ">");
    }

    if let Some(i) = state.hover {
        let sub = submenu_rect(menu, i, items[i].submenu.len());
        canvas.fill_rect(sub, MENU_BG);
        canvas.stroke_rect(sub, MENU_BORDER);

        for (sub_idx, sub_item) in items[i].submenu.iter().enumerate() {
            let r = submenu_item_rect(sub, sub_idx);
            if state.sub_hover == Some(sub_idx) {
                canvas.fill_rect(r, MENU_HOVER);
            }
            draw_label(
                canvas,
                Rect::new(r.x + 6, r.y, r.w - 12, r.h),
                sub_item.label,
                TEXT,
            );
        }
    }
}

pub fn handle_popup_menu_event(
    anchor: Rect,
    state: &mut MenuState,
    items: &[MenuItem<'_>],
    ev: &UiEvent,
) -> MenuOutcome {
    let mut out = MenuOutcome {
        changed: false,
        action: None,
    };

    if !state.open {
        return out;
    }

    let menu = menu_rect(anchor, items.len());

    match *ev {
        UiEvent::MouseMove { pos } => {
            let old_hover = state.hover;
            let old_sub = state.sub_hover;

            state.hover = None;
            state.sub_hover = None;

            for i in 0..items.len() {
                if menu_item_rect(menu, i).contains(pos) {
                    state.hover = Some(i);
                    break;
                }
            }

            if let Some(i) = state.hover {
                let sub = submenu_rect(menu, i, items[i].submenu.len());
                if sub.contains(pos) {
                    for sub_idx in 0..items[i].submenu.len() {
                        if submenu_item_rect(sub, sub_idx).contains(pos) {
                            state.sub_hover = Some(sub_idx);
                            break;
                        }
                    }
                }
            }

            out.changed = old_hover != state.hover || old_sub != state.sub_hover;
        }

        UiEvent::MouseDown {
            pos,
            button: MouseButton::Left,
        } => {
            let mut inside = menu.contains(pos);
            if let Some(i) = state.hover {
                inside |= submenu_rect(menu, i, items[i].submenu.len()).contains(pos);
            }
            if !inside {
                state.close();
                out.changed = true;
            }
        }

        UiEvent::MouseUp {
            pos,
            button: MouseButton::Left,
        } => {
            if let Some(i) = state.hover {
                let sub = submenu_rect(menu, i, items[i].submenu.len());
                for sub_idx in 0..items[i].submenu.len() {
                    let r = submenu_item_rect(sub, sub_idx);
                    if r.contains(pos) {
                        out.action = Some(items[i].submenu[sub_idx].action);
                        state.close();
                        out.changed = true;
                        break;
                    }
                }
            }
        }

        _ => {}
    }

    out
}
