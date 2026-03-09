extern crate alloc;

use alloc::string::String;

use libui::canvas::Canvas;
use libui::color::{PANEL_TEXT, TEXT, TEXT_DIM};
use libui::event::{MouseButton, UiEvent};
use libui::geom::{Point, Rect};
use libui::widgets::button::draw_button;
use libui::widgets::panel::{draw_panel, inner_rect};
use libui::widgets::text_area::{draw_text_area, handle_text_area_event, TextAreaState};
use libui::text::draw_text;

use crate::app::App;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ToolbarButton {
    None,
    Save,
    Reload,
}

pub struct NotepadApp {
    path: String,
    text_area: TextAreaState,
    hover_button: ToolbarButton,
    pressed_button: ToolbarButton,
    status: String,
    dirty: bool,
}

impl NotepadApp {
    const MAX_FILE_BYTES: usize = 64 * 1024;

    pub fn new() -> Self {
        Self {
            path: String::from("(no file)"),
            text_area: TextAreaState::new(""),
            hover_button: ToolbarButton::None,
            pressed_button: ToolbarButton::None,
            status: String::from("Empty document"),
            dirty: false,
        }
    }

    pub fn open(path: &str) -> Self {
        let mut this = Self::new();
        this.load_path(path);
        this
    }

    //fn cursor(&self, local_pos: Point) -> CursorKind {
    //    match Self::button_at(local_pos) {
    //        ToolbarButton::Save | ToolbarButton::Reload => CursorKind::Hand,
    //        ToolbarButton::None => {
    //            if Self::text_rect_local().contains(local_pos) {
    //                CursorKind::IBeam
    //            } else {
    //                CursorKind::Arrow
    //            }
    //        }
    //    }
    //}

    fn toolbar_rect_local() -> Rect {
        Rect::new(8, 8, 620, 28)
    }

    fn save_button_rect_local() -> Rect {
        Rect::new(12, 12, 56, 20)
    }

    fn reload_button_rect_local() -> Rect {
        Rect::new(74, 12, 64, 20)
    }

    fn path_rect_local() -> Rect {
        Rect::new(146, 12, 482, 20)
    }

    fn text_rect_local() -> Rect {
        Rect::new(8, 44, 620, 316)
    }

    fn status_rect_local() -> Rect {
        Rect::new(8, 368, 620, 28)
    }

    fn button_at(pos: Point) -> ToolbarButton {
        if Self::save_button_rect_local().contains(pos) {
            ToolbarButton::Save
        } else if Self::reload_button_rect_local().contains(pos) {
            ToolbarButton::Reload
        } else {
            ToolbarButton::None
        }
    }

    fn load_path(&mut self, path: &str) {
        self.path.clear();
        self.path.push_str(path);
        self.text_area.scroll.offset = 0;

        match rlibc::vfs::read(path, Self::MAX_FILE_BYTES) {
            Ok(bytes) => match core::str::from_utf8(&bytes) {
                Ok(s) => {
                    self.text_area.text.clear();
                    self.text_area.text.push_str(s);
                    self.dirty = false;

                    self.status.clear();
                    self.status.push_str("Opened ");
                    self.status.push_str(path);
                }
                Err(_) => {
                    self.text_area.text.clear();
                    self.text_area.text.push_str("[File is not valid UTF-8 text]");
                    self.dirty = false;

                    self.status.clear();
                    self.status.push_str("Unable to decode as UTF-8: ");
                    self.status.push_str(path);
                }
            },
            Err(_) => {
                self.text_area.text.clear();
                self.text_area.text.push_str("[Failed to read file]");
                self.dirty = false;

                self.status.clear();
                self.status.push_str("Failed to open ");
                self.status.push_str(path);
            }
        }
    }

    fn save(&mut self) {
        if self.path == "(no file)" {
            self.status.clear();
            self.status.push_str("No file path");
            return;
        }

        match rlibc::vfs::write(&self.path, self.text_area.text.as_bytes()) {
            Ok(()) => {
                self.dirty = false;
                self.status.clear();
                self.status.push_str("Saved ");
                self.status.push_str(&self.path);
            }
            Err(_) => {
                self.status.clear();
                self.status.push_str("Save failed: ");
                self.status.push_str(&self.path);
            }
        }
    }

    fn reload(&mut self) {
        if self.path == "(no file)" {
            self.status.clear();
            self.status.push_str("No file path");
            return;
        }

        let path = self.path.clone();
        self.load_path(&path);
    }

    fn update_status_hint(&mut self) {
        self.status.clear();
        if self.dirty {
            self.status.push_str("Modified | ");
        }
        self.status.push_str("Editable text file");
    }
}

impl App for NotepadApp {
    fn title(&self) -> &'static str {
        "NOTEPAD"
    }

    fn handle_event(&mut self, ev: &UiEvent) -> bool {
        let text_rect = Self::text_rect_local();
        let mut changed = false;

        changed |= handle_text_area_event(text_rect, &mut self.text_area, ev);

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
                        ToolbarButton::Save => self.save(),
                        ToolbarButton::Reload => self.reload(),
                        ToolbarButton::None => {}
                    }
                    changed = true;
                }
            }

            UiEvent::KeyDown { code } => {
                match code {
                    63 => {
                        self.save();
                        changed = true;
                    }
                    _ => {
                        match code {
                            14 | 28 | 57 => {
                                self.dirty = true;
                                self.update_status_hint();
                            }
                            2..=13 | 16..=27 | 30..=53 => {
                                self.dirty = true;
                                self.update_status_hint();
                            }
                            _ => {}
                        }
                    }
                }
            }

            UiEvent::KeyUp { .. } => {}
            UiEvent::MouseWheel { .. } => {}
            UiEvent::MouseDown { .. } => {}
            UiEvent::MouseUp { .. } => {}
        }

        changed
    }

    fn render(&self, canvas: &mut Canvas, client_rect: Rect, _focused: bool) {
        let toolbar = Self::toolbar_rect_local().translate(client_rect.x, client_rect.y);
        let save_btn = Self::save_button_rect_local().translate(client_rect.x, client_rect.y);
        let reload_btn = Self::reload_button_rect_local().translate(client_rect.x, client_rect.y);
        let path_rect = Self::path_rect_local().translate(client_rect.x, client_rect.y);
        let text_rect = Self::text_rect_local().translate(client_rect.x, client_rect.y);
        let status_rect = Self::status_rect_local().translate(client_rect.x, client_rect.y);

        draw_panel(canvas, toolbar);

        draw_button(
            canvas,
            save_btn,
            "Save",
            self.hover_button == ToolbarButton::Save,
            self.pressed_button == ToolbarButton::Save,
        );

        draw_button(
            canvas,
            reload_btn,
            "Reload",
            self.hover_button == ToolbarButton::Reload,
            self.pressed_button == ToolbarButton::Reload,
        );

        draw_panel(canvas, path_rect);
        let path_inner = inner_rect(path_rect, 6);
        draw_text(
            canvas,
            path_inner.x,
            path_inner.y + 3,
            PANEL_TEXT,
            None,
            &self.path,
        );

        draw_text_area(canvas, text_rect, &self.text_area);

        draw_panel(canvas, status_rect);
        let status_inner = inner_rect(status_rect, 6);

        if self.dirty {
            draw_text(
                canvas,
                status_inner.x,
                status_inner.y + 5,
                TEXT_DIM,
                None,
                "*",
            );
            draw_text(
                canvas,
                status_inner.x + 10,
                status_inner.y + 5,
                TEXT,
                None,
                &self.status,
            );
        } else {
            draw_text(
                canvas,
                status_inner.x,
                status_inner.y + 5,
                TEXT,
                None,
                &self.status,
            );
        }
    }
}
