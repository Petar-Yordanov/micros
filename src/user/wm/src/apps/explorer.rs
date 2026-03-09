extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use libui::canvas::Canvas;
use libui::color::{PANEL_TEXT, TEXT, TEXT_DIM};
use libui::event::{MouseButton, UiEvent};
use libui::geom::{Point, Rect};
use libui::widgets::button::draw_button;
use libui::widgets::list_view::{
    draw_list_view, handle_list_view_event, ListViewState, LIST_ROW_H,
};
use libui::widgets::panel::{draw_panel, inner_rect};

use crate::app::{request_launch, App, AppLaunch};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ToolbarButton {
    None,
    Up,
    Refresh,
}

struct ExplorerEntry {
    name: String,
    display: String,
    is_dir: bool,
}

pub struct ExplorerApp {
    current_path: String,
    entries: Vec<ExplorerEntry>,
    list: ListViewState,

    hover_button: ToolbarButton,
    pressed_button: ToolbarButton,

    status: String,

    last_click_selected: Option<usize>,
    last_click_ms: u64,
}

impl ExplorerApp {
    const LIST_MAX_BYTES: usize = 16 * 1024;
    const DOUBLE_CLICK_MS: u64 = 500;

    pub fn new() -> Self {
        let mut this = Self {
            current_path: String::from("/"),
            entries: Vec::new(),
            list: ListViewState::new(),
            hover_button: ToolbarButton::None,
            pressed_button: ToolbarButton::None,
            status: String::from("Loading /"),
            last_click_selected: None,
            last_click_ms: 0,
        };
        this.reload_current_dir();
        this
    }

    fn toolbar_rect_local() -> Rect {
        Rect::new(8, 8, 560, 28)
    }

    fn up_button_rect_local() -> Rect {
        Rect::new(12, 12, 56, 20)
    }

    fn refresh_button_rect_local() -> Rect {
        Rect::new(74, 12, 72, 20)
    }

    fn path_rect_local() -> Rect {
        Rect::new(154, 12, 410, 20)
    }

    fn list_rect_local() -> Rect {
        Rect::new(8, 44, 560, 264)
    }

    fn status_rect_local() -> Rect {
        Rect::new(8, 316, 560, 28)
    }

    fn button_at(pos: Point) -> ToolbarButton {
        if Self::up_button_rect_local().contains(pos) {
            ToolbarButton::Up
        } else if Self::refresh_button_rect_local().contains(pos) {
            ToolbarButton::Refresh
        } else {
            ToolbarButton::None
        }
    }

    //fn cursor(&self, local_pos: Point) -> CursorKind {
    //    if Self::button_at(local_pos) != ToolbarButton::None {
    //        CursorKind::Hand
    //    } else if Self::list_rect_local().contains(local_pos) {
    //        CursorKind::Hand
    //    } else {
    //        CursorKind::Arrow
    //    }
    //}

    fn now_ms() -> u64 {
        match rlibc::time::uptime() {
            Ok(ts) => ts.secs.saturating_mul(1000) + (ts.nanos as u64 / 1_000_000),
            Err(_) => 0,
        }
    }

    fn set_status(&mut self, msg: &str) {
        self.status.clear();
        self.status.push_str(msg);
    }

    fn set_status_path(&mut self, prefix: &str, path: &str) {
        self.status.clear();
        self.status.push_str(prefix);
        self.status.push_str(path);
    }

    fn normalize_path(path: &str) -> String {
        if path.is_empty() {
            return String::from("/");
        }

        let mut out = String::new();

        if !path.starts_with('/') {
            out.push('/');
        }

        let mut prev_was_slash = false;
        for ch in path.chars() {
            if ch == '/' {
                if !prev_was_slash {
                    out.push('/');
                    prev_was_slash = true;
                }
            } else {
                out.push(ch);
                prev_was_slash = false;
            }
        }

        while out.len() > 1 && out.ends_with('/') {
            out.pop();
        }

        if out.is_empty() {
            String::from("/")
        } else {
            out
        }
    }

    fn join_path(base: &str, name: &str) -> String {
        let base = Self::normalize_path(base);

        if base == "/" {
            let mut out = String::from("/");
            out.push_str(name);
            Self::normalize_path(&out)
        } else {
            let mut out = base;
            out.push('/');
            out.push_str(name);
            Self::normalize_path(&out)
        }
    }

    fn parent_path(path: &str) -> String {
        let norm = Self::normalize_path(path);
        if norm == "/" {
            return norm;
        }

        let bytes = norm.as_bytes();
        let mut last_sep = None;
        for i in (0..bytes.len()).rev() {
            if bytes[i] == b'/' {
                last_sep = Some(i);
                break;
            }
        }

        match last_sep {
            Some(0) | None => String::from("/"),
            Some(idx) => String::from(&norm[..idx]),
        }
    }

    fn is_text_file(name: &str) -> bool {
        name.ends_with(".txt")
    }

    fn probe_is_dir(&self, full_path: &str) -> bool {
        rlibc::vfs::list(full_path, Self::LIST_MAX_BYTES).is_ok()
    }

    fn reload_current_dir(&mut self) {
        self.entries.clear();
        self.list.selected = None;
        self.list.hovered = None;
        self.list.scroll.offset = 0;
        self.last_click_selected = None;
        self.last_click_ms = 0;

        let current = self.current_path.clone();
        match rlibc::vfs::list(&current, Self::LIST_MAX_BYTES) {
            Ok(mut names) => {
                names.retain(|s| !s.is_empty() && s.as_str() != "." && s.as_str() != "..");

                for name in names {
                    let full_path = Self::join_path(&current, &name);
                    let is_dir = self.probe_is_dir(&full_path);

                    let mut display = String::new();
                    display.push_str(&name);
                    if is_dir {
                        display.push('/');
                    }

                    self.entries.push(ExplorerEntry {
                        name,
                        display,
                        is_dir,
                    });
                }

                self.status.clear();
                self.status.push_str("Loaded ");
                self.status.push_str(&current);
                self.status.push_str(" (");
                append_usize(&mut self.status, self.entries.len());
                self.status.push_str(" items)");
            }
            Err(_) => {
                self.set_status_path("Failed to list ", &current);
            }
        }
    }

    fn go_up(&mut self) {
        let parent = Self::parent_path(&self.current_path);
        if parent != self.current_path {
            self.current_path = parent;
            self.reload_current_dir();
        } else {
            self.set_status("Already at root");
        }
    }

    fn activate_selected(&mut self) {
        let Some(idx) = self.list.selected else {
            self.set_status("No item selected");
            return;
        };

        if idx >= self.entries.len() {
            self.set_status("Selection out of range");
            return;
        }

        let full_path = Self::join_path(&self.current_path, &self.entries[idx].name);

        if self.entries[idx].is_dir {
            self.current_path = full_path;
            self.reload_current_dir();
        } else if Self::is_text_file(&self.entries[idx].name) {
            request_launch(AppLaunch::TextFile(full_path.clone()));
            self.status.clear();
            self.status.push_str("Opening ");
            self.status.push_str(&self.entries[idx].name);
        } else {
            self.status.clear();
            self.status.push_str("Unsupported file type: ");
            self.status.push_str(&self.entries[idx].name);
        }
    }

    fn move_selection(&mut self, delta: i32) {
        if self.entries.is_empty() {
            self.list.selected = None;
            return;
        }

        let len = self.entries.len() as i32;
        let current = self.list.selected.map(|v| v as i32).unwrap_or(0);
        let next = (current + delta).clamp(0, len - 1) as usize;
        self.list.selected = Some(next);
        self.ensure_selection_visible();
    }

    fn ensure_selection_visible(&mut self) {
        let Some(idx) = self.list.selected else {
            return;
        };

        let list = Self::list_rect_local();
        let viewport_h = list.h - 2;
        let row_top = idx as i32 * LIST_ROW_H;
        let row_bottom = row_top + LIST_ROW_H;

        if row_top < self.list.scroll.offset {
            self.list.scroll.offset = row_top;
        } else if row_bottom > self.list.scroll.offset + viewport_h {
            self.list.scroll.offset = row_bottom - viewport_h;
        }

        let content_h = (self.entries.len() as i32).saturating_mul(LIST_ROW_H);
        self.list.scroll.clamp_offset(viewport_h, content_h);
    }

    fn update_status_from_selection(&mut self) {
        if let Some(idx) = self.list.selected {
            if let Some(entry) = self.entries.get(idx) {
                self.status.clear();
                if entry.is_dir {
                    self.status.push_str("Directory: ");
                    self.status.push_str(&entry.name);
                    self.status.push_str(" (double-click to open)");
                } else {
                    self.status.push_str("File: ");
                    self.status.push_str(&entry.name);
                    if Self::is_text_file(&entry.name) {
                        self.status.push_str(" (double-click to open)");
                    }
                }
                return;
            }
        }

        let path = self.current_path.clone();
        self.set_status_path("Path: ", &path);
    }

    fn handle_list_double_click(&mut self) -> bool {
        let Some(idx) = self.list.selected else {
            return false;
        };

        let now = Self::now_ms();
        let is_double = self.last_click_selected == Some(idx)
            && now.saturating_sub(self.last_click_ms) <= Self::DOUBLE_CLICK_MS;

        self.last_click_selected = Some(idx);
        self.last_click_ms = now;

        if is_double {
            self.activate_selected();
            true
        } else {
            false
        }
    }
}

impl App for ExplorerApp {
    fn title(&self) -> &'static str {
        "EXPLORER"
    }

    fn handle_event(&mut self, ev: &UiEvent) -> bool {
        let list_rect = Self::list_rect_local();
        let mut changed = false;

        changed |= handle_list_view_event(list_rect, self.entries.len(), &mut self.list, ev);

        match *ev {
            UiEvent::MouseMove { pos } => {
                let hover = Self::button_at(pos);
                if hover != self.hover_button {
                    self.hover_button = hover;
                    changed = true;
                }
            }

            UiEvent::MouseDown {
                pos,
                button: MouseButton::Left,
            } => {
                let btn = Self::button_at(pos);
                if btn != ToolbarButton::None {
                    if self.pressed_button != btn {
                        self.pressed_button = btn;
                        changed = true;
                    }
                } else if list_rect.contains(pos) {
                    self.update_status_from_selection();
                    changed = true;
                }
            }

            UiEvent::MouseUp {
                pos,
                button: MouseButton::Left,
            } => {
                let released_over = Self::button_at(pos);
                let pressed = self.pressed_button;

                if self.pressed_button != ToolbarButton::None {
                    self.pressed_button = ToolbarButton::None;
                    changed = true;
                }

                if pressed != ToolbarButton::None && pressed == released_over {
                    match pressed {
                        ToolbarButton::Up => self.go_up(),
                        ToolbarButton::Refresh => self.reload_current_dir(),
                        ToolbarButton::None => {}
                    }
                    changed = true;
                } else if list_rect.contains(pos) {
                    self.update_status_from_selection();
                    changed = true;
                    changed |= self.handle_list_double_click();
                }
            }

            UiEvent::MouseDown { .. } => {}
            UiEvent::MouseUp { .. } => {}
            UiEvent::MouseWheel { .. } => {}

            UiEvent::KeyDown { code } => match code {
                103 => {
                    self.move_selection(-1);
                    self.update_status_from_selection();
                    changed = true;
                }
                108 => {
                    self.move_selection(1);
                    self.update_status_from_selection();
                    changed = true;
                }
                28 => {
                    self.activate_selected();
                    changed = true;
                }
                14 => {
                    self.go_up();
                    changed = true;
                }
                _ => {}
            },

            UiEvent::KeyUp { .. } => {}
        }

        changed
    }

    fn render(&self, canvas: &mut Canvas, client_rect: Rect, _focused: bool) {
        let toolbar = Self::toolbar_rect_local().translate(client_rect.x, client_rect.y);
        let up_btn = Self::up_button_rect_local().translate(client_rect.x, client_rect.y);
        let refresh_btn = Self::refresh_button_rect_local().translate(client_rect.x, client_rect.y);
        let path_rect = Self::path_rect_local().translate(client_rect.x, client_rect.y);
        let list_rect = Self::list_rect_local().translate(client_rect.x, client_rect.y);
        let status_rect = Self::status_rect_local().translate(client_rect.x, client_rect.y);

        draw_panel(canvas, toolbar);

        draw_button(
            canvas,
            up_btn,
            "Up",
            self.hover_button == ToolbarButton::Up,
            self.pressed_button == ToolbarButton::Up,
        );
        draw_button(
            canvas,
            refresh_btn,
            "Refresh",
            self.hover_button == ToolbarButton::Refresh,
            self.pressed_button == ToolbarButton::Refresh,
        );

        draw_panel(canvas, path_rect);
        let path_inner = inner_rect(path_rect, 6);
        libui::text::draw_text(
            canvas,
            path_inner.x,
            path_inner.y + 3,
            PANEL_TEXT,
            None,
            &self.current_path,
        );

        let mut items: Vec<&str> = Vec::with_capacity(self.entries.len());
        for entry in self.entries.iter() {
            items.push(entry.display.as_str());
        }

        libui::text::draw_text(
            canvas,
            list_rect.x,
            list_rect.y - 12,
            TEXT_DIM,
            None,
            "Files and directories",
        );
        draw_list_view(canvas, list_rect, &items, &self.list);

        draw_panel(canvas, status_rect);
        let status_inner = inner_rect(status_rect, 6);
        libui::text::draw_text(
            canvas,
            status_inner.x,
            status_inner.y + 5,
            TEXT,
            None,
            &self.status,
        );
    }
}

fn append_usize(out: &mut String, mut value: usize) {
    if value == 0 {
        out.push('0');
        return;
    }

    let mut tmp = [0u8; 20];
    let mut n = 0usize;

    while value != 0 {
        tmp[n] = b'0' + (value % 10) as u8;
        value /= 10;
        n += 1;
    }

    while n != 0 {
        n -= 1;
        out.push(tmp[n] as char);
    }
}
