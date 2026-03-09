extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use libui::canvas::Canvas;
use libui::color::{PANEL_TEXT, TEXT, TEXT_DIM};
use libui::event::{MouseButton, UiEvent};
use libui::geom::{Point, Rect};
use libui::text::{draw_text, CHAR_H};
use libui::widgets::button::draw_button;
use libui::widgets::list_view::{draw_list_view, ListViewState, LIST_ROW_H};
use libui::widgets::panel::{draw_panel, inner_rect};
use libui::widgets::scroll::max_offset;
use libui::widgets::table_view::{
    draw_table_view, TableColumn, TableRow, TableViewState, TABLE_HEADER_H, TABLE_ROW_H,
};
use libui::widgets::text_area::{draw_text_area, TextAreaState};
use libui::widgets::text_field::{draw_text_field, handle_text_field_event, TextFieldState};
use libui::widgets::tree_view::{draw_tree_view, TreeRow, TreeViewState, TREE_ROW_H};

use crate::app::App;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FocusTarget {
    None,
    PathField,
    Console,
}

pub struct DemoApp {
    list: ListViewState,
    table: TableViewState,
    tree: TreeViewState,
    path: TextFieldState,
    console: TextAreaState,
    focus: FocusTarget,
    menu_button_hover: bool,
    menu_button_pressed: bool,

    root_expanded: bool,
    bin_expanded: bool,
    etc_expanded: bool,
    home_expanded: bool,
    guest_expanded: bool,

    tree_rows: Vec<TreeRow<'static>>,
    log_counter: u32,
}

impl DemoApp {
    pub fn new() -> Self {
        let mut this = Self {
            list: ListViewState::new(),
            table: TableViewState::new(),
            tree: TreeViewState::new(),
            path: TextFieldState::new("/bin"),
            console: TextAreaState::new(
                "MicrOS64 widget demo\n\
                 - text field\n\
                 - text area / console\n\
                 - list view\n\
                 - table view\n\
                 - tree view\n\
                 \n\
                 Click rows to select.\n\
                 Click + / - in tree to toggle.\n\
                 Mouse wheel scrolls hovered panel.\n",
            ),
            focus: FocusTarget::None,
            menu_button_hover: false,
            menu_button_pressed: false,

            root_expanded: true,
            bin_expanded: true,
            etc_expanded: false,
            home_expanded: true,
            guest_expanded: true,

            tree_rows: Vec::new(),
            log_counter: 1,
        };
        this.rebuild_tree();
        this
    }

    fn toolbar_rect_local() -> Rect {
        Rect::new(8, 8, 700, 28)
    }

    fn menu_button_rect_local() -> Rect {
        Rect::new(12, 12, 88, 20)
    }

    fn path_rect_local() -> Rect {
        Rect::new(108, 12, 240, 20)
    }

    fn tree_rect_local() -> Rect {
        Rect::new(8, 44, 210, 156)
    }

    fn list_rect_local() -> Rect {
        Rect::new(8, 208, 210, 144)
    }

    fn table_rect_local() -> Rect {
        Rect::new(226, 44, 270, 308)
    }

    fn console_rect_local() -> Rect {
        Rect::new(504, 44, 204, 308)
    }

    fn status_rect_local() -> Rect {
        Rect::new(8, 360, 700, 24)
    }

    fn tree_rows_as_slice(&self) -> &[TreeRow<'static>] {
        &self.tree_rows
    }

    fn is_expanded(&self, id: u32) -> bool {
        match id {
            1 => self.root_expanded,
            2 => self.bin_expanded,
            6 => self.etc_expanded,
            9 => self.home_expanded,
            10 => self.guest_expanded,
            _ => false,
        }
    }

    fn toggle_node(&mut self, id: u32) {
        match id {
            1 => self.root_expanded = !self.root_expanded,
            2 => self.bin_expanded = !self.bin_expanded,
            6 => self.etc_expanded = !self.etc_expanded,
            9 => self.home_expanded = !self.home_expanded,
            10 => self.guest_expanded = !self.guest_expanded,
            _ => {}
        }
        self.rebuild_tree();
    }

    fn rebuild_tree(&mut self) {
        self.tree_rows.clear();

        self.push_row(1, "/", 0, true);
        if self.root_expanded {
            self.push_row(2, "bin", 1, true);
            if self.bin_expanded {
                self.push_row(3, "apps", 2, false);
                self.push_row(4, "init.elf", 2, false);
                self.push_row(5, "wm.elf", 2, false);
            }

            self.push_row(6, "etc", 1, true);
            if self.etc_expanded {
                self.push_row(7, "hostname", 2, false);
                self.push_row(8, "motd", 2, false);
            }

            self.push_row(9, "home", 1, true);
            if self.home_expanded {
                self.push_row(10, "guest", 2, true);
                if self.guest_expanded {
                    self.push_row(11, "notes.txt", 3, false);
                    self.push_row(12, "save.dat", 3, false);
                }
            }

            self.push_row(13, "tmp", 1, false);
        }
    }

    //fn cursor(&self, local_pos: Point) -> CursorKind {
    //    if Self::menu_button_rect_local().contains(local_pos) {
    //        CursorKind::Hand
    //    } else if Self::path_rect_local().contains(local_pos) {
    //        CursorKind::IBeam
    //    } else if Self::console_rect_local().contains(local_pos) {
    //        CursorKind::IBeam
    //    } else if Self::tree_rect_local().contains(local_pos)
    //        || Self::list_rect_local().contains(local_pos)
    //        || Self::table_rect_local().contains(local_pos)
    //    {
    //        CursorKind::Hand
    //    } else {
    //        CursorKind::Arrow
    //    }
    //}

    fn push_row(&mut self, id: u32, label: &'static str, depth: u8, has_children: bool) {
        self.tree_rows.push(TreeRow {
            id,
            label,
            depth,
            has_children,
            expanded: self.is_expanded(id),
        });
    }

    fn list_item(idx: usize) -> &'static str {
        match idx {
            0 => "README.md",
            1 => "boot.log",
            2 => "hello.txt",
            3 => "kernel.map",
            4 => "notes.txt",
            5 => "panic.txt",
            6 => "picture.bmp",
            7 => "screenshot.raw",
            8 => "shell.hist",
            9 => "sysinfo.txt",
            10 => "test_a.txt",
            11 => "test_b.txt",
            12 => "test_c.txt",
            13 => "todo.txt",
            14 => "users.txt",
            15 => "var.log",
            _ => "",
        }
    }

    fn list_len() -> usize {
        16
    }

    fn table_row<'a>(idx: usize) -> TableRow<'a> {
        match idx {
            0 => TableRow { cells: &["1", "RUN", "init"] },
            1 => TableRow { cells: &["2", "RUN", "wm"] },
            2 => TableRow { cells: &["3", "SLEEP", "clock"] },
            3 => TableRow { cells: &["4", "RUN", "demo"] },
            4 => TableRow { cells: &["5", "WAIT", "shell"] },
            5 => TableRow { cells: &["6", "RUN", "taskmgr"] },
            6 => TableRow { cells: &["7", "RUN", "explorer"] },
            7 => TableRow { cells: &["8", "SLEEP", "logger"] },
            8 => TableRow { cells: &["9", "RUN", "netd"] },
            9 => TableRow { cells: &["10", "WAIT", "inputd"] },
            10 => TableRow { cells: &["11", "RUN", "audio"] },
            11 => TableRow { cells: &["12", "RUN", "service"] },
            12 => TableRow { cells: &["13", "WAIT", "worker0"] },
            13 => TableRow { cells: &["14", "WAIT", "worker1"] },
            14 => TableRow { cells: &["15", "RUN", "worker2"] },
            _ => TableRow { cells: &["", "", ""] },
        }
    }

    fn table_len() -> usize {
        15
    }

    fn tree_content_height(&self) -> i32 {
        (self.tree_rows.len() as i32).saturating_mul(TREE_ROW_H)
    }

    fn list_content_height(&self) -> i32 {
        (Self::list_len() as i32).saturating_mul(LIST_ROW_H)
    }

    fn table_content_height(&self) -> i32 {
        (Self::table_len() as i32).saturating_mul(TABLE_ROW_H)
    }

    fn console_line_height(&self) -> i32 {
        CHAR_H + 2
    }

    fn console_line_count(&self) -> i32 {
        let mut n = 1i32;
        for b in self.console.text.as_bytes() {
            if *b == b'\n' {
                n += 1;
            }
        }
        n
    }

    fn console_content_height(&self) -> i32 {
        self.console_line_count()
            .saturating_mul(self.console_line_height())
    }

    fn clamp_tree_scroll(&mut self) {
        let viewport_h = Self::tree_rect_local().h - 2;
        self.tree.scroll.offset = self
            .tree
            .scroll
            .offset
            .clamp(0, max_offset(viewport_h, self.tree_content_height()));
    }

    fn clamp_list_scroll(&mut self) {
        let viewport_h = Self::list_rect_local().h - 2;
        self.list.scroll.offset = self
            .list
            .scroll
            .offset
            .clamp(0, max_offset(viewport_h, self.list_content_height()));
    }

    fn clamp_table_scroll(&mut self) {
        let viewport_h = Self::table_rect_local().h - TABLE_HEADER_H - 2;
        self.table.scroll.offset = self
            .table
            .scroll
            .offset
            .clamp(0, max_offset(viewport_h, self.table_content_height()));
    }

    fn clamp_console_scroll(&mut self) {
        let viewport_h = Self::console_rect_local().h - 8;
        self.console.scroll.offset = self
            .console
            .scroll
            .offset
            .clamp(0, max_offset(viewport_h, self.console_content_height()));
    }

    fn list_row_at(&self, pos: Point) -> Option<usize> {
        let rect = Self::list_rect_local();
        let content = Rect::new(rect.x + 1, rect.y + 1, rect.w - 16, rect.h - 2);
        if !content.contains(pos) {
            return None;
        }
        let y = pos.y - content.y + self.list.scroll.offset;
        if y < 0 {
            return None;
        }
        let idx = (y / LIST_ROW_H) as usize;
        if idx < Self::list_len() {
            Some(idx)
        } else {
            None
        }
    }

    fn table_row_at(&self, pos: Point) -> Option<usize> {
        let rect = Self::table_rect_local();
        let body = Rect::new(
            rect.x + 1,
            rect.y + 1 + TABLE_HEADER_H,
            rect.w - 16,
            rect.h - TABLE_HEADER_H - 2,
        );
        if !body.contains(pos) {
            return None;
        }
        let y = pos.y - body.y + self.table.scroll.offset;
        if y < 0 {
            return None;
        }
        let idx = (y / TABLE_ROW_H) as usize;
        if idx < Self::table_len() {
            Some(idx)
        } else {
            None
        }
    }

    fn tree_row_at(&self, pos: Point) -> Option<usize> {
        let rect = Self::tree_rect_local();
        let content = Rect::new(rect.x + 1, rect.y + 1, rect.w - 16, rect.h - 2);
        if !content.contains(pos) {
            return None;
        }
        let y = pos.y - content.y + self.tree.scroll.offset;
        if y < 0 {
            return None;
        }
        let idx = (y / TREE_ROW_H) as usize;
        if idx < self.tree_rows.len() {
            Some(idx)
        } else {
            None
        }
    }

    fn handle_tree_click(&mut self, pos: Point) -> bool {
        let Some(idx) = self.tree_row_at(pos) else {
            return false;
        };

        let row = self.tree_rows[idx];
        self.tree.selected_id = Some(row.id);

        let rect = Self::tree_rect_local();
        let content_y = rect.y + 1;
        let row_y = content_y + (idx as i32 * TREE_ROW_H) - self.tree.scroll.offset;
        let indent = row.depth as i32 * 14;
        let glyph_rect = Rect::new(rect.x + 1 + 4 + indent, row_y + 3, 10, 12);

        if row.has_children && glyph_rect.contains(pos) {
            self.toggle_node(row.id);
        }

        self.clamp_tree_scroll();
        true
    }

    fn handle_list_click(&mut self, pos: Point) -> bool {
        if let Some(idx) = self.list_row_at(pos) {
            self.list.selected = Some(idx);
            return true;
        }
        false
    }

    fn handle_table_click(&mut self, pos: Point) -> bool {
        if let Some(idx) = self.table_row_at(pos) {
            self.table.selected = Some(idx);
            return true;
        }
        false
    }

    fn scroll_step(delta: i32) -> i32 {
        if delta > 0 {
            -24
        } else if delta < 0 {
            24
        } else {
            0
        }
    }

    fn append_log_line(&mut self) {
        self.log_counter = self.log_counter.wrapping_add(1);
        let mut line = String::from("log line ");
        append_u32(&mut line, self.log_counter);
        self.console.append_line(&line);
        self.clamp_console_scroll();
    }

    fn append_typed_char(&mut self, code: u16) -> bool {
        if let Some(ch) = keycode_to_char(code) {
            self.console.text.push(ch);
            self.clamp_console_scroll();
            return true;
        }
        false
    }
}

impl App for DemoApp {
    fn title(&self) -> &'static str {
        "WIDGET DEMO"
    }

    fn handle_event(&mut self, ev: &UiEvent) -> bool {
        let menu_btn = Self::menu_button_rect_local();
        let path_rect = Self::path_rect_local();
        let tree_rect = Self::tree_rect_local();
        let list_rect = Self::list_rect_local();
        let table_rect = Self::table_rect_local();
        let console_rect = Self::console_rect_local();

        let mut changed = false;

        match *ev {
            UiEvent::MouseMove { pos } => {
                let hover = menu_btn.contains(pos);
                if hover != self.menu_button_hover {
                    self.menu_button_hover = hover;
                    changed = true;
                }
            }

            UiEvent::MouseDown {
                pos,
                button: MouseButton::Left,
            } => {
                if menu_btn.contains(pos) {
                    self.menu_button_pressed = true;
                    self.append_log_line();
                    return true;
                }

                changed |= handle_text_field_event(path_rect, &mut self.path, ev);
                if self.path.focused {
                    self.focus = FocusTarget::PathField;
                }

                if tree_rect.contains(pos) {
                    changed |= self.handle_tree_click(pos);
                }

                if list_rect.contains(pos) {
                    changed |= self.handle_list_click(pos);
                }

                if table_rect.contains(pos) {
                    changed |= self.handle_table_click(pos);
                }

                if console_rect.contains(pos) {
                    self.console.focused = true;
                    self.path.focused = false;
                    self.focus = FocusTarget::Console;
                    changed = true;
                } else if !path_rect.contains(pos) {
                    let old_path = self.path.focused;
                    let old_console = self.console.focused;
                    self.path.focused = false;
                    self.console.focused = false;
                    self.focus = FocusTarget::None;
                    changed |= old_path || old_console;
                }
            }

            UiEvent::MouseUp {
                button: MouseButton::Left,
                ..
            } => {
                if self.menu_button_pressed {
                    self.menu_button_pressed = false;
                    changed = true;
                }
            }

            UiEvent::MouseWheel { pos, delta } => {
                let step = Self::scroll_step(delta);

                if step != 0 {
                    if tree_rect.contains(pos) {
                        self.tree.scroll.offset = self.tree.scroll.offset.saturating_add(step);
                        self.clamp_tree_scroll();
                        changed = true;
                    } else if list_rect.contains(pos) {
                        self.list.scroll.offset = self.list.scroll.offset.saturating_add(step);
                        self.clamp_list_scroll();
                        changed = true;
                    } else if table_rect.contains(pos) {
                        self.table.scroll.offset = self.table.scroll.offset.saturating_add(step);
                        self.clamp_table_scroll();
                        changed = true;
                    } else if console_rect.contains(pos) {
                        self.console.scroll.offset = self.console.scroll.offset.saturating_add(step);
                        self.clamp_console_scroll();
                        changed = true;
                    }
                }
            }

            UiEvent::KeyDown { code } => match self.focus {
                FocusTarget::PathField => {
                    changed |= handle_text_field_event(path_rect, &mut self.path, ev);
                }

                FocusTarget::Console => match code {
                    14 => {
                        if !self.console.text.is_empty() {
                            self.console.text.pop();
                            self.clamp_console_scroll();
                            changed = true;
                        }
                    }
                    28 => {
                        self.console.text.push('\n');
                        self.clamp_console_scroll();
                        changed = true;
                    }
                    103 => {
                        self.console.scroll.offset = self
                            .console
                            .scroll
                            .offset
                            .saturating_sub(self.console_line_height());
                        self.clamp_console_scroll();
                        changed = true;
                    }
                    108 => {
                        self.console.scroll.offset = self
                            .console
                            .scroll
                            .offset
                            .saturating_add(self.console_line_height());
                        self.clamp_console_scroll();
                        changed = true;
                    }
                    _ => {
                        changed |= self.append_typed_char(code);
                    }
                },

                FocusTarget::None => {}
            },

            UiEvent::KeyUp { .. } => {}

            _ => {}
        }

        changed
    }

    fn render(&self, canvas: &mut Canvas, client_rect: Rect, _focused: bool) {
        let toolbar = Self::toolbar_rect_local().translate(client_rect.x, client_rect.y);
        let menu_btn = Self::menu_button_rect_local().translate(client_rect.x, client_rect.y);
        let path_rect = Self::path_rect_local().translate(client_rect.x, client_rect.y);
        let tree_rect = Self::tree_rect_local().translate(client_rect.x, client_rect.y);
        let list_rect = Self::list_rect_local().translate(client_rect.x, client_rect.y);
        let table_rect = Self::table_rect_local().translate(client_rect.x, client_rect.y);
        let console_rect = Self::console_rect_local().translate(client_rect.x, client_rect.y);
        let status = Self::status_rect_local().translate(client_rect.x, client_rect.y);

        let cols = [
            TableColumn { title: "PID", width: 48 },
            TableColumn { title: "State", width: 72 },
            TableColumn { title: "Name", width: 140 },
        ];

        let rows = [
            Self::table_row(0),
            Self::table_row(1),
            Self::table_row(2),
            Self::table_row(3),
            Self::table_row(4),
            Self::table_row(5),
            Self::table_row(6),
            Self::table_row(7),
            Self::table_row(8),
            Self::table_row(9),
            Self::table_row(10),
            Self::table_row(11),
            Self::table_row(12),
            Self::table_row(13),
            Self::table_row(14),
        ];

        let list_items = [
            Self::list_item(0),
            Self::list_item(1),
            Self::list_item(2),
            Self::list_item(3),
            Self::list_item(4),
            Self::list_item(5),
            Self::list_item(6),
            Self::list_item(7),
            Self::list_item(8),
            Self::list_item(9),
            Self::list_item(10),
            Self::list_item(11),
            Self::list_item(12),
            Self::list_item(13),
            Self::list_item(14),
            Self::list_item(15),
        ];

        draw_panel(canvas, toolbar);
        draw_button(
            canvas,
            menu_btn,
            "Add Log",
            self.menu_button_hover,
            self.menu_button_pressed,
        );
        draw_text(canvas, toolbar.x + 356, toolbar.y + 6, PANEL_TEXT, None, "Path:");
        draw_text_field(canvas, path_rect, &self.path);

        draw_text(canvas, tree_rect.x, tree_rect.y - 12, TEXT_DIM, None, "Tree View");
        draw_text(canvas, list_rect.x, list_rect.y - 12, TEXT_DIM, None, "List View");
        draw_text(canvas, table_rect.x, table_rect.y - 12, TEXT_DIM, None, "Table View");
        draw_text(
            canvas,
            console_rect.x,
            console_rect.y - 12,
            TEXT_DIM,
            None,
            "Text Area / Console",
        );

        draw_tree_view(canvas, tree_rect, self.tree_rows_as_slice(), &self.tree);
        draw_list_view(canvas, list_rect, &list_items, &self.list);
        draw_table_view(canvas, table_rect, &cols, &rows, &self.table);
        draw_text_area(canvas, console_rect, &self.console);

        draw_panel(canvas, status);
        let status_inner = inner_rect(status, 6);

        draw_text(canvas, status_inner.x, status_inner.y + 5, TEXT, None, "Selected file:");
        draw_text(
            canvas,
            status_inner.x + 112,
            status_inner.y + 5,
            PANEL_TEXT,
            None,
            self.list
                .selected
                .map(Self::list_item)
                .unwrap_or("(none)"),
        );

        draw_text(canvas, status_inner.x + 260, status_inner.y + 5, TEXT, None, "Task row:");
        draw_text(
            canvas,
            status_inner.x + 324,
            status_inner.y + 5,
            PANEL_TEXT,
            None,
            if self.table.selected.is_some() {
                "selected"
            } else {
                "(none)"
            },
        );

        draw_text(canvas, status_inner.x + 420, status_inner.y + 5, TEXT, None, "Tree:");
        draw_text(
            canvas,
            status_inner.x + 460,
            status_inner.y + 5,
            PANEL_TEXT,
            None,
            if self.tree.selected_id.is_some() {
                "selected"
            } else {
                "(none)"
            },
        );
    }
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

fn keycode_to_char(code: u16) -> Option<char> {
    match code {
        2 => Some('1'),
        3 => Some('2'),
        4 => Some('3'),
        5 => Some('4'),
        6 => Some('5'),
        7 => Some('6'),
        8 => Some('7'),
        9 => Some('8'),
        10 => Some('9'),
        11 => Some('0'),
        12 => Some('-'),
        13 => Some('='),
        16 => Some('q'),
        17 => Some('w'),
        18 => Some('e'),
        19 => Some('r'),
        20 => Some('t'),
        21 => Some('y'),
        22 => Some('u'),
        23 => Some('i'),
        24 => Some('o'),
        25 => Some('p'),
        26 => Some('['),
        27 => Some(']'),
        30 => Some('a'),
        31 => Some('s'),
        32 => Some('d'),
        33 => Some('f'),
        34 => Some('g'),
        35 => Some('h'),
        36 => Some('j'),
        37 => Some('k'),
        38 => Some('l'),
        39 => Some(';'),
        40 => Some('\''),
        41 => Some('`'),
        43 => Some('\\'),
        44 => Some('z'),
        45 => Some('x'),
        46 => Some('c'),
        47 => Some('v'),
        48 => Some('b'),
        49 => Some('n'),
        50 => Some('m'),
        51 => Some(','),
        52 => Some('.'),
        53 => Some('/'),
        57 => Some(' '),
        _ => None,
    }
}
