extern crate alloc;

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use libui::canvas::Canvas;
use libui::color::{PANEL_TEXT, TEXT, TEXT_DIM};
use libui::event::{MouseButton, UiEvent};
use libui::geom::{Point, Rect};
use libui::widgets::button::draw_button;
use libui::widgets::panel::{draw_panel, inner_rect};
use libui::widgets::table_view::{
    draw_table_view, handle_table_view_event, TableColumn, TableRow, TableViewState,
};

use micros_abi::types::ProcListEntry;

use crate::app::App;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ToolbarButton {
    None,
    Refresh,
    Kill,
}

struct ProcRow {
    pid: u32,
    pid_text: String,
    state_text: String,
    name: String,
    main_tid: u64,
}

pub struct TaskManagerApp {
    table: TableViewState,
    hover_button: ToolbarButton,
    pressed_button: ToolbarButton,

    rows: Vec<ProcRow>,
    selected_pid: Option<u32>,
    self_pid: Option<u32>,

    status: String,
}

impl TaskManagerApp {
    const MIN_BUF_ROWS: usize = 16;

    pub fn new() -> Self {
        let self_pid = rlibc::proc::getpid().ok();

        let mut this = Self {
            table: TableViewState::new(),
            hover_button: ToolbarButton::None,
            pressed_button: ToolbarButton::None,
            rows: Vec::new(),
            selected_pid: None,
            self_pid,
            status: String::from("Loading process list..."),
        };

        this.refresh_processes();
        this
    }

    fn toolbar_rect_local() -> Rect {
        Rect::new(8, 8, 604, 28)
    }

    fn refresh_button_rect_local() -> Rect {
        Rect::new(12, 12, 78, 20)
    }

    fn kill_button_rect_local() -> Rect {
        Rect::new(96, 12, 64, 20)
    }

    fn summary_rect_local() -> Rect {
        Rect::new(168, 12, 440, 20)
    }

    fn table_rect_local() -> Rect {
        Rect::new(8, 44, 604, 286)
    }

    fn status_rect_local() -> Rect {
        Rect::new(8, 338, 604, 28)
    }

    fn button_at(pos: Point) -> ToolbarButton {
        if Self::refresh_button_rect_local().contains(pos) {
            ToolbarButton::Refresh
        } else if Self::kill_button_rect_local().contains(pos) {
            ToolbarButton::Kill
        } else {
            ToolbarButton::None
        }
    }

    fn decode_name(entry: &ProcListEntry) -> String {
        let max_len = entry.name.len();
        let len = core::cmp::min(entry.name_len as usize, max_len);
        let bytes = &entry.name[..len];

        match core::str::from_utf8(bytes) {
            Ok(s) if !s.is_empty() => String::from(s),
            _ => String::from("(unnamed)"),
        }
    }

    //fn cursor(&self, local_pos: Point) -> CursorKind {
    //    if Self::button_at(local_pos) != ToolbarButton::None {
    //        CursorKind::Hand
    //    } else if Self::table_rect_local().contains(local_pos) {
    //        CursorKind::Hand
    //    } else {
    //        CursorKind::Arrow
    //    }
    //}

    fn proc_state_label(state: u32) -> &'static str {
        match state {
            0 => "New",
            1 => "Running",
            2 => "Zombie",
            _ => "Unknown",
        }
    }

    fn set_status(&mut self, msg: &str) {
        self.status.clear();
        self.status.push_str(msg);
    }

    fn refresh_processes(&mut self) {
        let keep_pid = self.selected_pid;

        let total = match rlibc::proc::proc_count() {
            Ok(v) => v as usize,
            Err(_) => {
                self.rows.clear();
                self.table.selected = None;
                self.selected_pid = None;
                self.set_status("Failed to query process count");
                return;
            }
        };

        let cap = core::cmp::max(total, Self::MIN_BUF_ROWS);
        let mut buf = vec![ProcListEntry::default(); cap];

        let (written, _reported_total) = match rlibc::proc::list(&mut buf) {
            Ok(v) => v,
            Err(_) => {
                self.rows.clear();
                self.table.selected = None;
                self.selected_pid = None;
                self.set_status("Failed to list processes");
                return;
            }
        };

        self.rows.clear();

        for entry in buf.iter().take(written) {
            let pid = entry.pid;
            let name = Self::decode_name(entry);
            let state_text = String::from(Self::proc_state_label(entry.state));

            let main_tid = match rlibc::proc::info(pid) {
                Ok(info) => info.main_tid,
                Err(_) => 0,
            };

            let mut pid_text = String::new();
            append_u32(&mut pid_text, pid);

            self.rows.push(ProcRow {
                pid,
                pid_text,
                state_text,
                name,
                main_tid,
            });
        }

        self.table.selected = None;
        self.selected_pid = None;

        if let Some(pid) = keep_pid {
            for (idx, row) in self.rows.iter().enumerate() {
                if row.pid == pid {
                    self.table.selected = Some(idx);
                    self.selected_pid = Some(pid);
                    break;
                }
            }
        }

        if self.rows.is_empty() {
            self.set_status("No processes");
        } else {
            self.update_status_from_selection();
        }
    }

    fn update_selection_from_table(&mut self) {
        self.selected_pid = self
            .table
            .selected
            .and_then(|idx| self.rows.get(idx).map(|row| row.pid));
    }

    fn update_status_from_selection(&mut self) {
        let Some(pid) = self.selected_pid else {
            self.set_status("No process selected");
            return;
        };

        let mut found_idx = None;
        for (idx, row) in self.rows.iter().enumerate() {
            if row.pid == pid {
                found_idx = Some(idx);
                break;
            }
        }

        let Some(idx) = found_idx else {
            self.set_status("Selected process no longer exists");
            return;
        };

        let row = &self.rows[idx];

        self.status.clear();
        self.status.push_str("PID ");
        append_u32(&mut self.status, row.pid);
        self.status.push_str(" | state ");
        self.status.push_str(&row.state_text);
        self.status.push_str(" | tid ");
        append_u64(&mut self.status, row.main_tid);
        self.status.push_str(" | name ");
        self.status.push_str(&row.name);

        if self.self_pid == Some(row.pid) {
            self.status.push_str(" | current WM process");
        }
    }

    fn try_kill_selected(&mut self) {
        let Some(pid) = self.selected_pid else {
            self.set_status("No process selected");
            return;
        };

        if self.self_pid == Some(pid) {
            self.set_status("Refusing to kill the current WM process");
            return;
        }

        match rlibc::proc::kill(pid, 0) {
            Ok(()) => {
                self.set_status("Kill requested");
                self.refresh_processes();
            }
            Err(_) => {
                self.set_status("Kill failed");
            }
        }
    }

    fn summary_text(&self) -> String {
        let mut s = String::from("Processes: ");
        append_usize(&mut s, self.rows.len());

        if let Some(pid) = self.self_pid {
            s.push_str(" | self pid: ");
            append_u32(&mut s, pid);
        }

        s
    }
}

impl App for TaskManagerApp {
    fn title(&self) -> &'static str {
        "TASK MANAGER"
    }

    fn handle_event(&mut self, ev: &UiEvent) -> bool {
        let table_rect = Self::table_rect_local();
        let mut changed = false;

        changed |= handle_table_view_event(table_rect, self.rows.len(), &mut self.table, ev);

        match *ev {
            UiEvent::MouseMove { pos } => {
                let hover = Self::button_at(pos);
                if hover != self.hover_button {
                    self.hover_button = hover;
                    changed = true;
                }

                let before = self.selected_pid;
                self.update_selection_from_table();
                if self.selected_pid != before {
                    self.update_status_from_selection();
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
                } else if table_rect.contains(pos) {
                    let before = self.selected_pid;
                    self.update_selection_from_table();
                    if self.selected_pid != before {
                        self.update_status_from_selection();
                        changed = true;
                    }
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
                        ToolbarButton::Refresh => self.refresh_processes(),
                        ToolbarButton::Kill => self.try_kill_selected(),
                        ToolbarButton::None => {}
                    }
                    changed = true;
                } else if table_rect.contains(pos) {
                    let before = self.selected_pid;
                    self.update_selection_from_table();
                    if self.selected_pid != before {
                        self.update_status_from_selection();
                        changed = true;
                    }
                }
            }

            UiEvent::KeyDown { code } => match code {
                63 => {
                    self.refresh_processes();
                    changed = true;
                }
                211 => {
                    self.try_kill_selected();
                    changed = true;
                }
                103 => {
                    if !self.rows.is_empty() {
                        let current = self.table.selected.unwrap_or(0);
                        let next = current.saturating_sub(1);
                        if self.table.selected != Some(next) {
                            self.table.selected = Some(next);
                            self.update_selection_from_table();
                            self.update_status_from_selection();
                            changed = true;
                        }
                    }
                }
                108 => {
                    if !self.rows.is_empty() {
                        let current = self.table.selected.unwrap_or(0);
                        let next = core::cmp::min(current + 1, self.rows.len() - 1);
                        if self.table.selected != Some(next) {
                            self.table.selected = Some(next);
                            self.update_selection_from_table();
                            self.update_status_from_selection();
                            changed = true;
                        }
                    }
                }
                _ => {}
            },

            UiEvent::MouseWheel { .. } => {}
            UiEvent::KeyUp { .. } => {}
            UiEvent::MouseDown { .. } => {}
            UiEvent::MouseUp { .. } => {}
        }

        changed
    }

    fn render(&self, canvas: &mut Canvas, client_rect: Rect, _focused: bool) {
        let toolbar = Self::toolbar_rect_local().translate(client_rect.x, client_rect.y);
        let refresh_btn = Self::refresh_button_rect_local().translate(client_rect.x, client_rect.y);
        let kill_btn = Self::kill_button_rect_local().translate(client_rect.x, client_rect.y);
        let summary_rect = Self::summary_rect_local().translate(client_rect.x, client_rect.y);
        let table_rect = Self::table_rect_local().translate(client_rect.x, client_rect.y);
        let status_rect = Self::status_rect_local().translate(client_rect.x, client_rect.y);

        let cols = [
            TableColumn { title: "PID", width: 64 },
            TableColumn { title: "State", width: 110 },
            TableColumn { title: "Name", width: 414 },
        ];

        let mut cell_rows: Vec<[&str; 3]> = Vec::with_capacity(self.rows.len());
        for row in self.rows.iter() {
            cell_rows.push([
                row.pid_text.as_str(),
                row.state_text.as_str(),
                row.name.as_str(),
            ]);
        }

        let mut table_rows: Vec<TableRow<'_>> = Vec::with_capacity(cell_rows.len());
        for cells in cell_rows.iter() {
            table_rows.push(TableRow { cells: &cells[..] });
        }

        draw_panel(canvas, toolbar);

        draw_button(
            canvas,
            refresh_btn,
            "Refresh",
            self.hover_button == ToolbarButton::Refresh,
            self.pressed_button == ToolbarButton::Refresh,
        );

        draw_button(
            canvas,
            kill_btn,
            "Kill",
            self.hover_button == ToolbarButton::Kill,
            self.pressed_button == ToolbarButton::Kill,
        );

        draw_panel(canvas, summary_rect);
        let summary_inner = inner_rect(summary_rect, 6);
        let summary = self.summary_text();
        libui::text::draw_text(
            canvas,
            summary_inner.x,
            summary_inner.y + 3,
            PANEL_TEXT,
            None,
            &summary,
        );

        libui::text::draw_text(
            canvas,
            table_rect.x,
            table_rect.y - 12,
            TEXT_DIM,
            None,
            "Kernel/user processes",
        );
        draw_table_view(canvas, table_rect, &cols, &table_rows, &self.table);

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

    //fn cursor(&self, local_pos: Point) -> CursorKind {
    //    self.cursor(local_pos)
    //}
}

fn append_u32(out: &mut String, mut value: u32) {
    if value == 0 {
        out.push('0');
        return;
    }

    let mut tmp = [0u8; 10];
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

fn append_u64(out: &mut String, mut value: u64) {
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
