use alloc::vec::Vec;

use libui::event::{CursorKind, MouseButton, UiEvent};
use libui::geom::{Point, Rect};

use crate::app::{
    take_launch_request, AppId, AppLaunch, DesktopIcon, StartMenuAction, StartMenuItem,
};
use crate::apps;
use crate::window::{Window, WindowHit};

pub const TASKBAR_H: i32 = 28;
pub const START_W: i32 = 64;
pub const TASK_BUTTON_W: i32 = 140;
pub const TASK_BUTTON_GAP: i32 = 4;
pub const TASK_BUTTON_CLOSE_W: i32 = 18;
pub const MENU_W: i32 = 180;
pub const MENU_ITEM_H: i32 = 24;
pub const SUBMENU_W: i32 = 140;

pub const DESKTOP_ICON_W: i32 = 64;
pub const DESKTOP_ICON_H: i32 = 72;
pub const DESKTOP_ICON_TILE: i32 = 40;
pub const DESKTOP_ICON_GAP_Y: i32 = 16;
pub const DESKTOP_ICON_GAP_X: i32 = 16;

#[derive(Clone, Copy)]
struct DragState {
    window_idx: usize,
    grab_offset_x: i32,
    grab_offset_y: i32,
}

#[derive(Clone, Copy)]
struct ButtonPressState {
    window_idx: usize,
}

pub struct Desktop {
    pub screen_w: i32,
    pub screen_h: i32,
    pub cursor: Point,
    pub windows: Vec<Window>,
    pub focused: Option<usize>,
    pub cursor_kind: CursorKind,

    pub start_menu_open: bool,
    pub menu_hover: Option<usize>,
    pub submenu_hover: Option<usize>,
    pub taskbar_hover_window: Option<usize>,
    pub taskbar_close_hover: Option<usize>,

    pub close_hover: Option<usize>,
    pub minimize_hover: Option<usize>,
    pub maximize_hover: Option<usize>,

    pub desktop_icon_hover: Option<usize>,
    pub desktop_icon_selected: Option<usize>,

    dirty_rect: Option<Rect>,
    next_window_id: u32,
    dragging: Option<DragState>,
    close_press: Option<ButtonPressState>,
    minimize_press: Option<ButtonPressState>,
    maximize_press: Option<ButtonPressState>,
    taskbar_close_press: Option<ButtonPressState>,
}

impl Desktop {
    pub fn new(screen_w: usize, screen_h: usize) -> Self {
        let screen_w = screen_w as i32;
        let screen_h = screen_h as i32;

        let mut this = Self {
            screen_w,
            screen_h,
            cursor: Point::new(screen_w / 2, screen_h / 2),
            windows: Vec::new(),
            focused: None,
            cursor_kind: CursorKind::Arrow,

            start_menu_open: false,
            menu_hover: None,
            submenu_hover: None,
            taskbar_hover_window: None,
            taskbar_close_hover: None,

            close_hover: None,
            minimize_hover: None,
            maximize_hover: None,

            desktop_icon_hover: None,
            desktop_icon_selected: None,

            dirty_rect: Some(Rect::new(0, 0, screen_w, screen_h)),
            next_window_id: 1,
            dragging: None,
            close_press: None,
            minimize_press: None,
            maximize_press: None,
            taskbar_close_press: None,
        };

        this.launch_app(AppLaunch::App(AppId::Hello));
        this
    }

    pub fn mark_dirty(&mut self) {
        self.dirty_rect = Some(Rect::new(0, 0, self.screen_w, self.screen_h));
    }

    pub fn mark_dirty_rect(&mut self, rect: Rect) {
        let clipped = clip_to_screen(rect, self.screen_w, self.screen_h);
        if clipped.w <= 0 || clipped.h <= 0 {
            return;
        }

        self.dirty_rect = Some(match self.dirty_rect {
            Some(old) => union_rect(old, clipped),
            None => clipped,
        });
    }

    pub fn take_dirty_rect(&mut self) -> Option<Rect> {
        self.dirty_rect.take()
    }

    pub fn move_cursor(&mut self, dx: i32, dy: i32) {
        let old_pos = self.cursor;
        let old_kind = self.cursor_kind;

        let new_x = (self.cursor.x + dx).clamp(0, self.screen_w.saturating_sub(1));
        let new_y = (self.cursor.y + dy).clamp(0, self.screen_h.saturating_sub(1));

        if new_x != self.cursor.x || new_y != self.cursor.y {
            self.cursor.x = new_x;
            self.cursor.y = new_y;

            self.mark_dirty_rect(cursor_rect_at(old_pos, old_kind));
            self.mark_dirty_rect(cursor_rect_at(self.cursor, self.cursor_kind));
        }
    }

    fn update_cursor_kind(&mut self, pos: Point) {
        let old_kind = self.cursor_kind;
        let new_kind = self.resolve_cursor_kind(pos);

        if new_kind != old_kind {
            self.mark_dirty_rect(cursor_rect_at(pos, old_kind));
            self.cursor_kind = new_kind;
            self.mark_dirty_rect(cursor_rect_at(pos, new_kind));
        }
    }

    pub fn taskbar_rect(&self) -> Rect {
        Rect::new(0, self.screen_h - TASKBAR_H, self.screen_w, TASKBAR_H)
    }

    pub fn work_area_rect(&self) -> Rect {
        Rect::new(0, 0, self.screen_w, self.screen_h - TASKBAR_H)
    }

    pub fn start_button_rect(&self) -> Rect {
        let tb = self.taskbar_rect();
        Rect::new(tb.x + 4, tb.y + 2, START_W, tb.h - 4)
    }

    pub fn clock_rect(&self) -> Rect {
        let tb = self.taskbar_rect();
        Rect::new(tb.right() - 124, tb.y + 2, 120, tb.h - 4)
    }

    pub fn task_button_rect(&self, idx: usize) -> Rect {
        let start = self.start_button_rect();
        let x = start.right() + 8 + (idx as i32 * (TASK_BUTTON_W + TASK_BUTTON_GAP));
        Rect::new(x, self.taskbar_rect().y + 2, TASK_BUTTON_W, TASKBAR_H - 4)
    }

    pub fn task_button_close_rect(&self, idx: usize) -> Rect {
        let rect = self.task_button_rect(idx);
        Rect::new(
            rect.right() - TASK_BUTTON_CLOSE_W - 4,
            rect.y + ((rect.h - 16) / 2).max(0),
            TASK_BUTTON_CLOSE_W,
            16,
        )
    }

    pub fn task_button_main_rect(&self, idx: usize) -> Rect {
        let rect = self.task_button_rect(idx);
        let close = self.task_button_close_rect(idx);
        Rect::new(rect.x, rect.y, (close.x - rect.x - 2).max(0), rect.h)
    }

    fn start_menu_interactive_at(&self, pos: Point) -> bool {
        if !self.start_menu_open {
            return false;
        }

        for i in 0..apps::start_menu_items().len() {
            if self.start_menu_item_rect(i).contains(pos) {
                return true;
            }
        }

        if let Some(item_idx) = self.menu_hover {
            let submenu = apps::start_menu_items()[item_idx].submenu;
            for sub_idx in 0..submenu.len() {
                if self.submenu_item_rect(item_idx, sub_idx).contains(pos) {
                    return true;
                }
            }
        }

        false
    }

    fn resolve_cursor_kind(&self, pos: Point) -> CursorKind {
        if self.start_button_rect().contains(pos) {
            return CursorKind::Hand;
        }

        if self.taskbar_rect().contains(pos) {
            for i in 0..self.windows.len() {
                if self.task_button_rect(i).contains(pos) {
                    return CursorKind::Hand;
                }
            }
        }

        if self.start_menu_interactive_at(pos) {
            return CursorKind::Hand;
        }

        for i in 0..self.desktop_icons().len() {
            if self.desktop_icon_rect(i).contains(pos) {
                return CursorKind::Hand;
            }
        }

        if let Some(idx) = self.visible_frame_window_at(pos) {
            let win = &self.windows[idx];

            if win.close_button_rect().contains(pos)
                || win.minimize_button_rect().contains(pos)
                || win.maximize_button_rect().contains(pos)
            {
                return CursorKind::Hand;
            }

            if win.contains_client(pos) {
                let local = Self::client_local_point(win, pos);
                return win.app.cursor(local);
            }
        }

        CursorKind::Arrow
    }

    pub fn start_menu_rect(&self) -> Rect {
        let start = self.start_button_rect();
        let items_len = apps::start_menu_items().len() as i32;
        Rect::new(
            start.x,
            self.taskbar_rect().y - (MENU_ITEM_H * items_len) - 8,
            MENU_W,
            (MENU_ITEM_H * items_len) + 8,
        )
    }

    pub fn start_menu_item_rect(&self, idx: usize) -> Rect {
        let menu = self.start_menu_rect();
        Rect::new(
            menu.x + 4,
            menu.y + 4 + (idx as i32 * MENU_ITEM_H),
            menu.w - 8,
            MENU_ITEM_H,
        )
    }

    pub fn submenu_rect(&self, idx: usize) -> Rect {
        let item = self.start_menu_item_rect(idx);
        let entries = apps::start_menu_items()[idx].submenu.len() as i32;
        let h = (entries * MENU_ITEM_H) + 8;

        let mut y = item.y;
        let screen_bottom = self.taskbar_rect().y;

        if y + h > screen_bottom {
            y = (screen_bottom - h).max(0);
        }

        Rect::new(item.right() + 4, y, SUBMENU_W, h)
    }

    pub fn submenu_item_rect(&self, item_idx: usize, sub_idx: usize) -> Rect {
        let sub = self.submenu_rect(item_idx);
        Rect::new(
            sub.x + 4,
            sub.y + 4 + sub_idx as i32 * MENU_ITEM_H,
            sub.w - 8,
            MENU_ITEM_H,
        )
    }

    pub fn desktop_icon_rect(&self, idx: usize) -> Rect {
        let col = (idx as i32) % 2;
        let row = (idx as i32) / 2;

        Rect::new(
            16 + col * (DESKTOP_ICON_W + DESKTOP_ICON_GAP_X),
            16 + row * (DESKTOP_ICON_H + DESKTOP_ICON_GAP_Y),
            DESKTOP_ICON_W,
            DESKTOP_ICON_H,
        )
    }

    pub fn start_menu_items(&self) -> &'static [StartMenuItem] {
        apps::start_menu_items()
    }

    pub fn desktop_icons(&self) -> &'static [DesktopIcon] {
        apps::desktop_icons()
    }

    fn visible_frame_window_at(&self, p: Point) -> Option<usize> {
        for (idx, win) in self.windows.iter().enumerate().rev() {
            if !win.minimized && win.contains_frame(p) {
                return Some(idx);
            }
        }
        None
    }

    fn visible_client_window_at(&self, p: Point) -> Option<usize> {
        for (idx, win) in self.windows.iter().enumerate().rev() {
            if !win.minimized && win.contains_client(p) {
                return Some(idx);
            }
        }
        None
    }

    fn client_local_point(win: &Window, screen_pos: Point) -> Point {
        let r = win.client_rect();
        Point::new(screen_pos.x - r.x, screen_pos.y - r.y)
    }

    fn topmost_visible_window_index(&self) -> Option<usize> {
        for (idx, win) in self.windows.iter().enumerate().rev() {
            if !win.minimized {
                return Some(idx);
            }
        }
        None
    }

    fn refocus_after_visibility_change(&mut self) {
        self.focused = self.topmost_visible_window_index();
        self.mark_dirty();
    }

    fn focus_window(&mut self, idx: usize) {
        if idx >= self.windows.len() {
            return;
        }

        let mut win = self.windows.remove(idx);
        win.minimized = false;
        self.windows.push(win);
        self.focused = Some(self.windows.len() - 1);
        self.mark_dirty();
    }

    fn launch_app(&mut self, launch: AppLaunch) {
        let win_id = self.next_window_id;
        self.next_window_id = self.next_window_id.wrapping_add(1);

        let offset = (self.windows.len() as i32 * 24) % 160;
        let work = self.work_area_rect();

        let (wanted_w, wanted_h) = match &launch {
            AppLaunch::App(AppId::Demo) => (728, 430),
            AppLaunch::App(AppId::Hello) => (360, 220),
            AppLaunch::App(AppId::Explorer) => (600, 380),
            AppLaunch::App(AppId::TaskManager) => (620, 394),
            AppLaunch::App(AppId::Clock) => (440, 340),
            AppLaunch::App(AppId::Notepad) => (640, 420),
            AppLaunch::TextFile(_) => (640, 420),
        };

        let frame_w = wanted_w.min((work.w - 24).max(240));
        let frame_h = wanted_h.min((work.h - 24).max(180));

        let frame = Rect::new(
            (60 + offset).clamp(work.x, (work.right() - frame_w).max(work.x)),
            (56 + offset).clamp(work.y, (work.bottom() - frame_h).max(work.y)),
            frame_w,
            frame_h,
        );

        self.windows
            .push(Window::new(win_id, frame, apps::make_launch(launch)));
        self.focused = Some(self.windows.len() - 1);
        self.mark_dirty();
    }

    fn close_window(&mut self, idx: usize) {
        if idx >= self.windows.len() {
            return;
        }

        self.windows.remove(idx);
        self.close_hover = None;
        self.minimize_hover = None;
        self.maximize_hover = None;
        self.taskbar_hover_window = None;
        self.taskbar_close_hover = None;
        self.close_press = None;
        self.minimize_press = None;
        self.maximize_press = None;
        self.taskbar_close_press = None;
        self.dragging = None;
        self.refocus_after_visibility_change();
    }

    fn minimize_window(&mut self, idx: usize) {
        if idx >= self.windows.len() {
            return;
        }

        let frame = self.windows[idx].frame;
        self.windows[idx].minimized = true;
        self.dragging = None;
        self.mark_dirty_rect(frame);
        self.refocus_after_visibility_change();
    }

    fn toggle_maximize_window(&mut self, idx: usize) {
        if idx >= self.windows.len() {
            return;
        }

        let work = self.work_area_rect();
        let old_frame = self.windows[idx].frame;
        let win = &mut self.windows[idx];

        if win.maximized {
            if let Some(prev) = win.restore_frame.take() {
                win.frame = prev;
            }
            win.maximized = false;
        } else {
            win.restore_frame = Some(win.frame);
            win.frame = Rect::new(work.x, work.y, work.w, work.h);
            win.maximized = true;
            win.minimized = false;
        }

        self.mark_dirty_rect(union_rect(old_frame, self.windows[idx].frame));
    }

    fn menu_damage_rect(&self) -> Rect {
        let mut r = self.start_menu_rect();
        let items = apps::start_menu_items();
        for i in 0..items.len() {
            if !items[i].submenu.is_empty() {
                r = union_rect(r, self.submenu_rect(i));
            }
        }
        r
    }

    fn update_menu_hover(&mut self, pos: Point) {
        let old_hover = self.menu_hover;
        let old_sub = self.submenu_hover;

        let items = apps::start_menu_items();
        let mut new_hover = None;
        let mut new_sub = None;

        for i in 0..items.len() {
            if self.start_menu_item_rect(i).contains(pos) {
                new_hover = Some(i);
                break;
            }
        }

        if new_hover.is_none() {
            for i in 0..items.len() {
                let submenu = items[i].submenu;
                if submenu.is_empty() {
                    continue;
                }

                let sub_rect = self.submenu_rect(i);
                if sub_rect.contains(pos) {
                    new_hover = Some(i);

                    for sub_idx in 0..submenu.len() {
                        if self.submenu_item_rect(i, sub_idx).contains(pos) {
                            new_sub = Some(sub_idx);
                            break;
                        }
                    }
                    break;
                }
            }
        } else if let Some(item_idx) = new_hover {
            let submenu = items[item_idx].submenu;
            if !submenu.is_empty() {
                let sub_rect = self.submenu_rect(item_idx);
                if sub_rect.contains(pos) {
                    for sub_idx in 0..submenu.len() {
                        if self.submenu_item_rect(item_idx, sub_idx).contains(pos) {
                            new_sub = Some(sub_idx);
                            break;
                        }
                    }
                }
            }
        }

        self.menu_hover = new_hover;
        self.submenu_hover = new_sub;

        if old_hover != self.menu_hover || old_sub != self.submenu_hover {
            self.mark_dirty_rect(self.menu_damage_rect());
        }
    }

    fn update_taskbar_hover(&mut self, pos: Point) {
        let old_hover = self.taskbar_hover_window;
        let old_close_hover = self.taskbar_close_hover;

        self.taskbar_hover_window = None;
        self.taskbar_close_hover = None;

        for i in 0..self.windows.len() {
            if self.task_button_rect(i).contains(pos) {
                self.taskbar_hover_window = Some(i);
                if self.task_button_close_rect(i).contains(pos) {
                    self.taskbar_close_hover = Some(i);
                }
                break;
            }
        }

        if old_hover != self.taskbar_hover_window || old_close_hover != self.taskbar_close_hover {
            if let Some(i) = old_hover.or(old_close_hover) {
                self.mark_dirty_rect(self.task_button_rect(i));
            }
            if let Some(i) = self.taskbar_hover_window.or(self.taskbar_close_hover) {
                self.mark_dirty_rect(self.task_button_rect(i));
            }
        }
    }

    fn button_damage_rect_for(&self, idx: usize) -> Rect {
        let win = &self.windows[idx];
        let mut r = win.close_button_rect();
        r = union_rect(r, win.minimize_button_rect());
        r = union_rect(r, win.maximize_button_rect());
        r
    }

    fn update_window_button_hovers(&mut self, pos: Point) {
        let old_close = self.close_hover;
        let old_min = self.minimize_hover;
        let old_max = self.maximize_hover;

        self.close_hover = None;
        self.minimize_hover = None;
        self.maximize_hover = None;

        if let Some(idx) = self.visible_frame_window_at(pos) {
            let win = &self.windows[idx];
            if win.close_button_rect().contains(pos) {
                self.close_hover = Some(idx);
            } else if win.minimize_button_rect().contains(pos) {
                self.minimize_hover = Some(idx);
            } else if win.maximize_button_rect().contains(pos) {
                self.maximize_hover = Some(idx);
            }
        }

        if old_close != self.close_hover
            || old_min != self.minimize_hover
            || old_max != self.maximize_hover
        {
            if let Some(i) = old_close.or(old_min).or(old_max) {
                self.mark_dirty_rect(self.button_damage_rect_for(i));
            }
            if let Some(i) = self
                .close_hover
                .or(self.minimize_hover)
                .or(self.maximize_hover)
            {
                self.mark_dirty_rect(self.button_damage_rect_for(i));
            }
        }
    }

    fn update_desktop_icon_hover(&mut self, pos: Point) {
        let old = self.desktop_icon_hover;
        self.desktop_icon_hover = None;

        let icons = apps::desktop_icons();
        for i in 0..icons.len() {
            if self.desktop_icon_rect(i).contains(pos) {
                self.desktop_icon_hover = Some(i);
                break;
            }
        }

        if old != self.desktop_icon_hover {
            if let Some(i) = old {
                self.mark_dirty_rect(self.desktop_icon_rect(i));
            }
            if let Some(i) = self.desktop_icon_hover {
                self.mark_dirty_rect(self.desktop_icon_rect(i));
            }
        }
    }

    fn perform_start_action(&mut self, action: StartMenuAction) {
        match action {
            StartMenuAction::Launch(app_id) => {
                self.launch_app(AppLaunch::App(app_id));
            }
            StartMenuAction::Shutdown => {
                let _ = rlibc::log::log("wm: shutdown requested\n");
                match rlibc::power::power_off() {
                    Ok(()) => {}
                    Err(_) => {
                        let _ = rlibc::log::log("wm: shutdown failed\n");
                    }
                }
            }
        }

        self.start_menu_open = false;
        self.menu_hover = None;
        self.submenu_hover = None;
        self.mark_dirty_rect(self.menu_damage_rect());
    }

    pub fn handle_event(&mut self, ev: UiEvent) {
        let mut changed = false;

        match ev {
            UiEvent::MouseMove { pos } => {
                if let Some(drag) = self.dragging {
                    if drag.window_idx < self.windows.len() {
                        let old_frame = self.windows[drag.window_idx].frame;
                        let work = self.work_area_rect();
                        let new_x = pos.x - drag.grab_offset_x;
                        let new_y = pos.y - drag.grab_offset_y;

                        self.windows[drag.window_idx].frame.x =
                            new_x.clamp(work.x, work.right() - 80);
                        self.windows[drag.window_idx].frame.y =
                            new_y.clamp(work.y, work.bottom() - 40);

                        let new_frame = self.windows[drag.window_idx].frame;
                        self.cursor_kind = CursorKind::Arrow;
                        self.mark_dirty_rect(union_rect(old_frame, new_frame));
                    }
                } else {
                    self.update_menu_hover(pos);
                    self.update_taskbar_hover(pos);
                    self.update_window_button_hovers(pos);
                    self.update_desktop_icon_hover(pos);

                    let target = self.focused.or_else(|| self.visible_client_window_at(pos));
                    if let Some(idx) = target {
                        if idx < self.windows.len()
                            && !self.windows[idx].minimized
                            && self.windows[idx].contains_client(pos)
                        {
                            let local = Self::client_local_point(&self.windows[idx], pos);
                            changed |= self.windows[idx]
                                .app
                                .handle_event(&UiEvent::MouseMove { pos: local });
                        }
                    }

                    self.update_cursor_kind(pos);
                }
            }

            UiEvent::MouseDown {
                pos,
                button: MouseButton::Left,
            } => {
                if self.start_button_rect().contains(pos) {
                    self.start_menu_open = !self.start_menu_open;
                    self.menu_hover = None;
                    self.submenu_hover = None;
                    self.mark_dirty_rect(self.menu_damage_rect());
                    self.mark_dirty_rect(self.start_button_rect());
                } else if self.start_menu_open && self.start_menu_rect().contains(pos) {
                    self.update_menu_hover(pos);
                } else if self.start_menu_open {
                    let mut clicked_submenu = false;

                    if let Some(item_idx) = self.menu_hover {
                        let submenu = apps::start_menu_items()[item_idx].submenu;
                        for sub_idx in 0..submenu.len() {
                            if self.submenu_item_rect(item_idx, sub_idx).contains(pos) {
                                clicked_submenu = true;
                                break;
                            }
                        }
                    }

                    if !clicked_submenu {
                        self.start_menu_open = false;
                        self.menu_hover = None;
                        self.submenu_hover = None;
                        self.mark_dirty_rect(self.menu_damage_rect());
                    }
                } else if self.taskbar_rect().contains(pos) {
                    for i in 0..self.windows.len() {
                        if self.task_button_close_rect(i).contains(pos) {
                            self.taskbar_close_press = Some(ButtonPressState { window_idx: i });
                            self.mark_dirty_rect(self.task_button_rect(i));
                            return;
                        }

                        if self.task_button_main_rect(i).contains(pos) {
                            self.focus_window(i);
                            return;
                        }
                    }
                } else if let Some(idx) = self.visible_frame_window_at(pos) {
                    self.focus_window(idx);
                    let idx = self.focused.unwrap();

                    match self.windows[idx].hit_test(pos) {
                        WindowHit::CloseButton => {
                            self.close_press = Some(ButtonPressState { window_idx: idx });
                            self.mark_dirty_rect(self.button_damage_rect_for(idx));
                        }
                        WindowHit::MinimizeButton => {
                            self.minimize_press = Some(ButtonPressState { window_idx: idx });
                            self.mark_dirty_rect(self.button_damage_rect_for(idx));
                        }
                        WindowHit::MaximizeButton => {
                            self.maximize_press = Some(ButtonPressState { window_idx: idx });
                            self.mark_dirty_rect(self.button_damage_rect_for(idx));
                        }
                        WindowHit::Titlebar => {
                            if !self.windows[idx].maximized {
                                self.dragging = Some(DragState {
                                    window_idx: idx,
                                    grab_offset_x: pos.x - self.windows[idx].frame.x,
                                    grab_offset_y: pos.y - self.windows[idx].frame.y,
                                });
                                self.mark_dirty_rect(self.windows[idx].frame);
                            }
                        }
                        WindowHit::Client => {
                            let local = Self::client_local_point(&self.windows[idx], pos);
                            changed |= self.windows[idx].app.handle_event(&UiEvent::MouseDown {
                                pos: local,
                                button: MouseButton::Left,
                            });
                        }
                        WindowHit::None => {}
                    }
                } else {
                    let icons = apps::desktop_icons();
                    let mut clicked_icon = false;

                    for i in 0..icons.len() {
                        if self.desktop_icon_rect(i).contains(pos) {
                            let old = self.desktop_icon_selected;
                            self.desktop_icon_selected = Some(i);
                            if let Some(old_idx) = old {
                                self.mark_dirty_rect(self.desktop_icon_rect(old_idx));
                            }
                            self.mark_dirty_rect(self.desktop_icon_rect(i));
                            clicked_icon = true;
                            break;
                        }
                    }

                    if !clicked_icon && self.start_menu_open {
                        self.start_menu_open = false;
                        self.menu_hover = None;
                        self.submenu_hover = None;
                        self.mark_dirty_rect(self.menu_damage_rect());
                    }
                }
            }

            UiEvent::MouseUp {
                pos,
                button: MouseButton::Left,
            } => {
                if self.dragging.take().is_some() {
                    self.mark_dirty();
                }

                if let Some(press) = self.taskbar_close_press.take() {
                    if press.window_idx < self.windows.len()
                        && self.task_button_close_rect(press.window_idx).contains(pos)
                    {
                        self.close_window(press.window_idx);
                        return;
                    }
                    if press.window_idx < self.windows.len() {
                        self.mark_dirty_rect(self.task_button_rect(press.window_idx));
                    }
                }

                if let Some(press) = self.close_press.take() {
                    if press.window_idx < self.windows.len()
                        && self.windows[press.window_idx]
                            .close_button_rect()
                            .contains(pos)
                    {
                        self.close_window(press.window_idx);
                        return;
                    }
                    if press.window_idx < self.windows.len() {
                        self.mark_dirty_rect(self.button_damage_rect_for(press.window_idx));
                    }
                }

                if let Some(press) = self.minimize_press.take() {
                    if press.window_idx < self.windows.len()
                        && self.windows[press.window_idx]
                            .minimize_button_rect()
                            .contains(pos)
                    {
                        self.minimize_window(press.window_idx);
                        return;
                    }
                    if press.window_idx < self.windows.len() {
                        self.mark_dirty_rect(self.button_damage_rect_for(press.window_idx));
                    }
                }

                if let Some(press) = self.maximize_press.take() {
                    if press.window_idx < self.windows.len()
                        && self.windows[press.window_idx]
                            .maximize_button_rect()
                            .contains(pos)
                    {
                        self.toggle_maximize_window(press.window_idx);
                        return;
                    }
                    if press.window_idx < self.windows.len() {
                        self.mark_dirty_rect(self.button_damage_rect_for(press.window_idx));
                    }
                }

                if self.start_menu_open {
                    let items = apps::start_menu_items();

                    if let Some(item_idx) = self.menu_hover {
                        let item_rect = self.start_menu_item_rect(item_idx);
                        if item_rect.contains(pos) {
                            if let Some(action) = items[item_idx].action {
                                self.perform_start_action(action);
                                return;
                            }
                        }

                        let submenu = items[item_idx].submenu;
                        for sub_idx in 0..submenu.len() {
                            if self.submenu_item_rect(item_idx, sub_idx).contains(pos) {
                                self.perform_start_action(submenu[sub_idx].action);
                                return;
                            }
                        }
                    }
                }

                let icons = apps::desktop_icons();
                for i in 0..icons.len() {
                    if self.desktop_icon_rect(i).contains(pos)
                        && self.desktop_icon_selected == Some(i)
                    {
                        self.launch_app(AppLaunch::App(icons[i].app_id));
                        return;
                    }
                }

                if let Some(idx) = self.focused {
                    if idx < self.windows.len()
                        && !self.windows[idx].minimized
                        && self.windows[idx].contains_client(pos)
                    {
                        let local = Self::client_local_point(&self.windows[idx], pos);
                        changed |= self.windows[idx].app.handle_event(&UiEvent::MouseUp {
                            pos: local,
                            button: MouseButton::Left,
                        });
                    }
                }
            }

            UiEvent::MouseWheel { pos, delta } => {
                if let Some(idx) = self.visible_client_window_at(pos).or(self.focused) {
                    if idx < self.windows.len() && !self.windows[idx].minimized {
                        let local = if self.windows[idx].contains_client(pos) {
                            Self::client_local_point(&self.windows[idx], pos)
                        } else {
                            Point::new(0, 0)
                        };

                        changed |= self.windows[idx]
                            .app
                            .handle_event(&UiEvent::MouseWheel { pos: local, delta });
                    }
                }
            }

            UiEvent::MouseDown { .. } => {}
            UiEvent::MouseUp { .. } => {}

            UiEvent::KeyDown { code } => {
                if let Some(idx) = self.focused {
                    if idx < self.windows.len() && !self.windows[idx].minimized {
                        changed |= self.windows[idx]
                            .app
                            .handle_event(&UiEvent::KeyDown { code });
                    }
                }
            }

            UiEvent::KeyUp { code } => {
                if let Some(idx) = self.focused {
                    if idx < self.windows.len() && !self.windows[idx].minimized {
                        changed |= self.windows[idx].app.handle_event(&UiEvent::KeyUp { code });
                    }
                }
            }
        }

        while let Some(req) = take_launch_request() {
            self.launch_app(req);
            changed = true;
        }

        if changed {
            self.mark_dirty();
        }
    }
}

fn clip_to_screen(rect: Rect, screen_w: i32, screen_h: i32) -> Rect {
    let x0 = rect.x.max(0);
    let y0 = rect.y.max(0);
    let x1 = rect.right().min(screen_w);
    let y1 = rect.bottom().min(screen_h);

    if x1 <= x0 || y1 <= y0 {
        Rect::new(0, 0, 0, 0)
    } else {
        Rect::new(x0, y0, x1 - x0, y1 - y0)
    }
}

fn union_rect(a: Rect, b: Rect) -> Rect {
    let x0 = a.x.min(b.x);
    let y0 = a.y.min(b.y);
    let x1 = a.right().max(b.right());
    let y1 = a.bottom().max(b.bottom());
    Rect::new(x0, y0, x1 - x0, y1 - y0)
}

fn cursor_rect_at(pos: Point, kind: CursorKind) -> Rect {
    match kind {
        CursorKind::Arrow => Rect::new(pos.x, pos.y, 16, 24),
        CursorKind::Hand => Rect::new(pos.x, pos.y, 18, 22),
        CursorKind::IBeam => Rect::new(pos.x, pos.y, 11, 19),
    }
}
