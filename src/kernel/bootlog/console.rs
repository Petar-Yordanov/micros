use core::ptr::copy;

use super::tags::{split_tag, tag_color};
pub(super) const BOOTLOG_MAX_LINES: usize = 160;
pub(super) const BOOTLOG_LINE_BYTES: usize = 112;
pub(super) const FONT_W: usize = 8;
pub(super) const LINE_H: usize = 12;

#[derive(Clone, Copy)]
pub(super) struct FbSurface {
    pub base: *mut u8,
    pub width: usize,
    pub height: usize,
    pub pitch: usize,
    pub bpp: usize,
}

#[derive(Clone, Copy)]
pub(super) struct BootLine {
    len: u8,
    bytes: [u8; BOOTLOG_LINE_BYTES],
}

impl BootLine {
    pub const fn empty() -> Self {
        Self {
            len: 0,
            bytes: [0; BOOTLOG_LINE_BYTES],
        }
    }

    pub fn set(&mut self, s: &str) {
        let src = s.as_bytes();
        let n = core::cmp::min(src.len(), BOOTLOG_LINE_BYTES);
        self.bytes[..n].copy_from_slice(&src[..n]);
        if n < BOOTLOG_LINE_BYTES {
            self.bytes[n..].fill(0);
        }
        self.len = n as u8;
    }

    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.bytes[..self.len as usize]).unwrap_or("?")
    }
}

pub(super) struct BootConsole {
    pub fb: Option<FbSurface>,
    pub lines: [BootLine; BOOTLOG_MAX_LINES],
    pub count: usize,
    pub progress_done: usize,
    pub progress_total: usize,
    pub bg: u32,
    pub panel_bg: u32,
    pub panel_border: u32,
    pub text: u32,
}

impl BootConsole {
    pub const fn new() -> Self {
        Self {
            fb: None,
            lines: [const { BootLine::empty() }; BOOTLOG_MAX_LINES],
            count: 0,
            progress_done: 0,
            progress_total: 1,
            bg: 0x000000,
            panel_bg: 0x0B0F14,
            panel_border: 0x25303B,
            text: 0xD8DEE9,
        }
    }

    pub unsafe fn try_init(&mut self) {
        if self.fb.is_some() {
            return;
        }

        let resp = match crate::platform::limine::framebuffer::FRAMEBUFFER_REQ.get_response() {
            Some(r) => r,
            None => return,
        };

        let mut fbs = resp.framebuffers();
        let fb = match fbs.next() {
            Some(fb) => fb,
            None => return,
        };

        let bpp = (fb.bpp() / 8) as usize;
        if bpp < 3 {
            return;
        }

        self.fb = Some(FbSurface {
            base: fb.addr().cast::<u8>(),
            width: fb.width() as usize,
            height: fb.height() as usize,
            pitch: fb.pitch() as usize,
            bpp,
        });

        self.draw_frame();
        self.draw_progress_bar();
    }

    pub fn set_progress(&mut self, done: usize, total: usize) {
        self.progress_done = done;
        self.progress_total = total;
        if self.fb.is_some() {
            self.draw_progress_bar();
        }
    }

    pub fn push_line(&mut self, s: &str) {
        if self.count < BOOTLOG_MAX_LINES {
            self.lines[self.count].set(s);
            self.count += 1;
        } else {
            let mut i = 1;
            while i < BOOTLOG_MAX_LINES {
                self.lines[i - 1] = self.lines[i];
                i += 1;
            }
            self.lines[BOOTLOG_MAX_LINES - 1].set(s);
        }

        if self.fb.is_none() {
            return;
        }

        let visible = self.visible_line_count();
        if self.count == 1 {
            self.draw_visible_line_at_row(0, 0);
        } else if self.count <= visible {
            self.draw_visible_line_at_row(self.count - 1, self.count - 1);
        } else {
            self.scroll_up_one_line();
            self.clear_bottom_line_strip();
            let row = visible - 1;
            let idx = self.count - 1;
            self.draw_visible_line_at_row(row, idx);
        }
    }

    pub fn visible_line_count(&self) -> usize {
        let (_, _, _, inner_h) = self.log_inner_rect();
        core::cmp::max(1, inner_h.saturating_sub(12) / LINE_H)
    }

    pub fn panel_rect(&self) -> (usize, usize, usize, usize) {
        let fb = self.fb.unwrap();
        let panel_w = core::cmp::min(fb.width.saturating_sub(80), 1120);
        let panel_h = core::cmp::min(fb.height.saturating_sub(80), 720);
        let panel_x = (fb.width.saturating_sub(panel_w)) / 2;
        let panel_y = (fb.height.saturating_sub(panel_h)) / 2;
        (panel_x, panel_y, panel_w, panel_h)
    }

    pub fn log_inner_rect(&self) -> (usize, usize, usize, usize) {
        let (panel_x, panel_y, panel_w, panel_h) = self.panel_rect();
        let inner_x = panel_x + 16;
        let inner_y = panel_y + 40;
        let inner_w = panel_w.saturating_sub(32);
        let inner_h = panel_h.saturating_sub(92);
        (inner_x, inner_y, inner_w, inner_h)
    }

    pub fn progress_rect(&self) -> (usize, usize, usize, usize) {
        let (panel_x, panel_y, panel_w, panel_h) = self.panel_rect();
        let bar_x = panel_x + 16;
        let bar_w = panel_w.saturating_sub(32);
        let bar_h = 22usize;
        let bar_y = panel_y + panel_h.saturating_sub(34);
        (bar_x, bar_y, bar_w, bar_h)
    }

    pub fn draw_frame(&mut self) {
        let Some(_) = self.fb else { return };

        self.clear_screen(self.bg);

        let (panel_x, panel_y, panel_w, panel_h) = self.panel_rect();

        self.fill_rect(panel_x, panel_y, panel_w, panel_h, self.panel_bg);
        self.stroke_rect(panel_x, panel_y, panel_w, panel_h, self.panel_border);

        let title = "MicrOS64 kernel boot";
        self.draw_text(panel_x + 16, panel_y + 14, title, 0xE5E9F0);

        let (inner_x, inner_y, inner_w, inner_h) = self.log_inner_rect();
        self.fill_rect(inner_x, inner_y, inner_w, inner_h, 0x070A0D);
        self.stroke_rect(inner_x, inner_y, inner_w, inner_h, 0x1B232C);

        let _ = panel_h;
    }

    pub fn draw_progress_bar(&mut self) {
        let Some(_) = self.fb else { return };

        let (x, y, w, h) = self.progress_rect();

        self.fill_rect(x, y, w, h, 0x070A0D);
        self.stroke_rect(x, y, w, h, 0x1B232C);

        let inner_x = x + 2;
        let inner_y = y + 2;
        let inner_w = w.saturating_sub(4);
        let inner_h = h.saturating_sub(4);

        self.fill_rect(inner_x, inner_y, inner_w, inner_h, 0x0B0F14);

        let pct = super::tags::progress_pct(self.progress_done, self.progress_total);

        let fill_w = (inner_w * pct) / 100;
        if fill_w > 0 {
            self.fill_rect(inner_x, inner_y, fill_w, inner_h, 0x2FA84F);
            if inner_h > 3 {
                self.fill_rect(inner_x, inner_y, fill_w, 2, 0x66D17D);
            }
        }

        let mut label_buf = [0u8; 4];
        let label =
            super::tags::progress_label(self.progress_done, self.progress_total, &mut label_buf);

        let text_x = x + (w.saturating_sub(label.len() * FONT_W)) / 2;
        let text_y = y + (h.saturating_sub(8)) / 2;
        self.draw_text(text_x, text_y, label, 0xF3F6FA);
    }

    pub fn draw_visible_line_at_row(&mut self, row: usize, line_idx: usize) {
        if line_idx >= self.count {
            return;
        }

        let (inner_x, inner_y, inner_w, _) = self.log_inner_rect();
        let y = inner_y + 8 + row * LINE_H;

        self.fill_rect(inner_x + 2, y, inner_w.saturating_sub(4), LINE_H, 0x070A0D);

        let line = self.lines[line_idx];
        self.draw_log_line(inner_x + 10, y, line.as_str());
    }

    pub fn clear_bottom_line_strip(&mut self) {
        let (inner_x, inner_y, inner_w, inner_h) = self.log_inner_rect();
        let visible = self.visible_line_count();
        let bottom_row = visible.saturating_sub(1);
        let y = inner_y + 8 + bottom_row * LINE_H;

        let clear_h = core::cmp::min(LINE_H, inner_y + inner_h.saturating_sub(2) - y);
        self.fill_rect(inner_x + 2, y, inner_w.saturating_sub(4), clear_h, 0x070A0D);
    }

    pub fn scroll_up_one_line(&mut self) {
        let Some(fb) = self.fb else { return };

        let (inner_x, inner_y, inner_w, inner_h) = self.log_inner_rect();
        let src_y = inner_y + 8 + LINE_H;
        let dst_y = inner_y + 8;
        let copy_h = inner_h.saturating_sub(10 + LINE_H);

        if copy_h == 0 || inner_w <= 4 {
            return;
        }

        let copy_w = inner_w.saturating_sub(4);
        let copy_bytes = copy_w * fb.bpp;

        let mut row = 0usize;
        while row < copy_h {
            let src_off = (src_y + row) * fb.pitch + (inner_x + 2) * fb.bpp;
            let dst_off = (dst_y + row) * fb.pitch + (inner_x + 2) * fb.bpp;
            unsafe {
                copy(fb.base.add(src_off), fb.base.add(dst_off), copy_bytes);
            }
            row += 1;
        }
    }

    pub fn draw_log_line(&mut self, x: usize, y: usize, s: &str) {
        let (tag, rest) = split_tag(s);
        let color = tag_color(tag);
        self.draw_text(x, y, tag, color);
        let tag_px = tag.len() * FONT_W;
        self.draw_text(x + tag_px + 8, y, rest, self.text);
    }

    pub fn set_progress_total(&mut self, total: usize) {
        self.progress_total = total;
        if self.fb.is_some() {
            self.draw_progress_bar();
        }
    }

    pub fn progress_total(&self) -> usize {
        self.progress_total
    }
}
