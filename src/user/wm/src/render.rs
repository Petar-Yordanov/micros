use font8x8::{UnicodeFonts, BASIC_FONTS};
use libui::canvas::Canvas;
use libui::color::{
    BG, BUTTON_BORDER, CURSOR_BORDER, CURSOR_FILL, PANEL_TEXT, START_BOTTOM, START_TEXT, START_TOP,
    TASKBAR_BOTTOM, TASKBAR_EDGE_BOTTOM, TASKBAR_EDGE_TOP, TASKBAR_TOP, TEXT,
    TITLEBAR_ACTIVE_BOTTOM, TITLEBAR_ACTIVE_TOP, TITLEBAR_INACTIVE_BOTTOM, TITLEBAR_INACTIVE_TOP,
    WHITE, WINDOW_BG, WINDOW_BORDER, WINDOW_BORDER_INNER,
};
use libui::event::CursorKind;
use libui::geom::Rect;
use libui::text::draw_text;
use libui::widgets::label::draw_label;
use rlibc::time::wall_time;

use crate::desktop::{
    Desktop, DESKTOP_ICON_H, DESKTOP_ICON_TILE, MENU_ITEM_H, START_W, SUBMENU_W, TASKBAR_H,
    TASK_BUTTON_W,
};
use crate::icon::decode_best_ico;
use crate::window::TITLEBAR_H;

const START_MENU_BG: u32 = 0x00FFFBF0;
const START_MENU_BORDER: u32 = 0x007F9DB9;
const START_MENU_HOVER: u32 = 0x003169C6;

const SHUTDOWN_TOP: u32 = 0x00F6C38D;
const SHUTDOWN_BOTTOM: u32 = 0x00C96B2E;
const SHUTDOWN_HOVER_TOP: u32 = 0x00FFD7A8;
const SHUTDOWN_HOVER_BOTTOM: u32 = 0x00DE7D3A;

const CLOSE_TOP: u32 = 0x00F7B38A;
const CLOSE_BOTTOM: u32 = 0x00D66A32;
const CLOSE_HOVER_TOP: u32 = 0x00FFD2B3;
const CLOSE_HOVER_BOTTOM: u32 = 0x00E57F42;

const MINMAX_TOP: u32 = 0x00C7DDF7;
const MINMAX_BOTTOM: u32 = 0x0087B4E6;
const MINMAX_HOVER_TOP: u32 = 0x00D9EAFF;
const MINMAX_HOVER_BOTTOM: u32 = 0x0095C0EF;

const ICON_HOVER: u32 = 0x0087B7F3;
const ICON_SELECTED: u32 = 0x003169C6;
const ICON_LABEL_BG: u32 = 0x003169C6;

const WALLPAPER_TITLE: u32 = 0x00E8E8E8;
const WALLPAPER_SUBTITLE: u32 = 0x00C8C8C8;
const WALLPAPER_SHADOW: u32 = 0x00000000;

const TASK_BTN_ACTIVE_TOP: u32 = 0x00F6FAFF;
const TASK_BTN_ACTIVE_BOTTOM: u32 = 0x00C8DCF6;
const TASK_BTN_HOVER_TOP: u32 = 0x00EAF4FF;
const TASK_BTN_HOVER_BOTTOM: u32 = 0x00B9D4F6;
const TASK_BTN_IDLE_TOP: u32 = 0x00D8E6F7;
const TASK_BTN_IDLE_BOTTOM: u32 = 0x0098BBE8;

const TASK_BTN_INNER_TOP: u32 = 0x00FFFFFF;
const TASK_BTN_TEXT_ACTIVE: u32 = 0x00000000;
const TASK_BTN_TEXT_IDLE: u32 = 0x000A2342;

const TASK_CLOSE_TOP: u32 = 0x00F8B28C;
const TASK_CLOSE_BOTTOM: u32 = 0x00D56A34;
const TASK_CLOSE_HOVER_TOP: u32 = 0x00FFD4B5;
const TASK_CLOSE_HOVER_BOTTOM: u32 = 0x00E5834D;

pub fn draw(
    desktop: &Desktop,
    fb: *mut u32,
    pitch_pixels: usize,
    width: usize,
    height: usize,
    clip: Option<Rect>,
) {
    let mut canvas = Canvas::new(fb, pitch_pixels, width, height);
    if let Some(r) = clip {
        canvas.set_clip(r);
    }

    if canvas.intersects(Rect::new(0, 0, width as i32, height as i32)) {
        draw_wallpaper(&mut canvas, width as i32, height as i32);
        draw_desktop_icons(&mut canvas, desktop, fb, pitch_pixels, width, height);
        draw_windows(&mut canvas, desktop);
        draw_taskbar(&mut canvas, desktop);
        draw_start_menu(&mut canvas, desktop);
        draw_cursor(
            &mut canvas,
            desktop.cursor.x,
            desktop.cursor.y,
            desktop.cursor_kind,
        );
    }
}

fn draw_wallpaper(canvas: &mut Canvas, screen_w: i32, screen_h: i32) {
    let title = "MicrOS";
    let subtitle = "64-bit Rust OS";

    let full = Rect::new(0, 0, screen_w, screen_h);
    if !canvas.intersects(full) {
        return;
    }

    canvas.fill_rect(full, 0x00000000);

    let title_scale = 6;
    let subtitle_scale = 2;

    let title_w = measure_text_scaled(title, title_scale);
    let subtitle_w = measure_text_scaled(subtitle, subtitle_scale);

    let band_h = 56;
    let band_y = (screen_h / 2) - (band_h / 2);
    let band = Rect::new(0, band_y, screen_w, band_h);

    if canvas.intersects(band) {
        canvas.fill_vertical_gradient(band, 0x00181818, 0x00080808);
    }

    let title_x = (screen_w - title_w) / 2;
    let title_y = band_y + 8;

    let subtitle_x = (screen_w - subtitle_w) / 2;
    let subtitle_y = title_y + 58;

    draw_text_scaled(
        canvas,
        title_x + 2,
        title_y + 2,
        WALLPAPER_SHADOW,
        title,
        title_scale,
    );
    draw_text_scaled(
        canvas,
        title_x,
        title_y,
        WALLPAPER_TITLE,
        title,
        title_scale,
    );

    draw_text_scaled(
        canvas,
        subtitle_x + 1,
        subtitle_y + 1,
        WALLPAPER_SHADOW,
        subtitle,
        subtitle_scale,
    );
    draw_text_scaled(
        canvas,
        subtitle_x,
        subtitle_y,
        WALLPAPER_SUBTITLE,
        subtitle,
        subtitle_scale,
    );
}

fn measure_text_scaled(text: &str, scale: i32) -> i32 {
    (text.chars().count() as i32) * 8 * scale
}

fn draw_text_scaled(canvas: &mut Canvas, x: i32, y: i32, color: u32, text: &str, scale: i32) {
    let mut xx = x;
    for ch in text.chars() {
        draw_char_scaled(canvas, xx, y, color, ch, scale);
        xx += 8 * scale;
    }
}

fn draw_char_scaled(canvas: &mut Canvas, x: i32, y: i32, color: u32, ch: char, scale: i32) {
    if scale <= 0 {
        return;
    }

    let Some(glyph): Option<[u8; 8]> = BASIC_FONTS.get(ch) else {
        return;
    };

    for (row, bits) in glyph.into_iter().enumerate() {
        for col in 0..8usize {
            if ((bits >> col) & 1) == 0 {
                continue;
            }

            let px = x + col as i32 * scale;
            let py = y + row as i32 * scale;
            canvas.fill_rect(Rect::new(px, py, scale, scale), color);
        }
    }
}

fn draw_desktop_icons(
    canvas: &mut Canvas,
    desktop: &Desktop,
    fb: *mut u32,
    pitch_pixels: usize,
    screen_w: usize,
    screen_h: usize,
) {
    let icons = desktop.desktop_icons();

    for (idx, icon) in icons.iter().enumerate() {
        let rect = desktop.desktop_icon_rect(idx);
        if !canvas.intersects(rect) {
            continue;
        }

        let tile = Rect::new(
            rect.x + ((rect.w - DESKTOP_ICON_TILE) / 2).max(0),
            rect.y,
            DESKTOP_ICON_TILE,
            DESKTOP_ICON_TILE,
        );

        let tile_color = if desktop.desktop_icon_selected == Some(idx) {
            Some(ICON_SELECTED)
        } else if desktop.desktop_icon_hover == Some(idx) {
            Some(ICON_HOVER)
        } else {
            None
        };

        if let Some(color) = tile_color {
            canvas.fill_rect(tile, color);
            canvas.stroke_rect(tile, BUTTON_BORDER);
        }

        match decode_best_ico(icon.icon_ico_bytes, DESKTOP_ICON_TILE as usize) {
            Ok(decoded) => {
                let draw_x = tile.x + ((tile.w - decoded.width as i32) / 2).max(0);
                let draw_y = tile.y + ((tile.h - decoded.height as i32) / 2).max(0);
                blit_rgba_icon(
                    fb,
                    pitch_pixels,
                    screen_w,
                    screen_h,
                    draw_x,
                    draw_y,
                    decoded.width,
                    decoded.height,
                    &decoded.pixels_rgba,
                    Some(rect),
                );
            }
            Err(_) => {
                draw_text(
                    canvas,
                    tile.x + (DESKTOP_ICON_TILE / 2) - 4,
                    tile.y + 16,
                    WHITE,
                    None,
                    icon.placeholder_text,
                );
            }
        }

        let label_rect = Rect::new(
            rect.x,
            rect.y + DESKTOP_ICON_TILE + 4,
            rect.w,
            DESKTOP_ICON_H - DESKTOP_ICON_TILE - 4,
        );

        if desktop.desktop_icon_selected == Some(idx) {
            canvas.fill_rect(label_rect, ICON_LABEL_BG);
        }

        let tw = libui::text::measure_text(icon.label);
        let tx = label_rect.x + ((label_rect.w - tw) / 2).max(0);
        let ty = label_rect.y + ((label_rect.h - 8) / 2).max(0);
        draw_text(canvas, tx, ty, WHITE, None, icon.label);
    }
}

fn draw_windows(canvas: &mut Canvas, desktop: &Desktop) {
    for (idx, win) in desktop.windows.iter().enumerate() {
        if win.minimized {
            continue;
        }

        if !canvas.intersects(win.frame) {
            continue;
        }

        let focused = desktop.focused == Some(idx);

        canvas.fill_rect(win.frame, WINDOW_BORDER);
        canvas.stroke_rect(win.frame, WINDOW_BORDER);

        let frame_inner = win.frame.inset(1, 1);
        canvas.stroke_rect(frame_inner, WINDOW_BORDER_INNER);

        let inner = win.frame.inset(2, 2);
        canvas.fill_rect(inner, WINDOW_BG);

        let titlebar = Rect::new(inner.x, inner.y, inner.w, TITLEBAR_H - 2);
        canvas.fill_vertical_gradient(
            titlebar,
            if focused {
                TITLEBAR_ACTIVE_TOP
            } else {
                TITLEBAR_INACTIVE_TOP
            },
            if focused {
                TITLEBAR_ACTIVE_BOTTOM
            } else {
                TITLEBAR_INACTIVE_BOTTOM
            },
        );
        canvas.hline(titlebar.x, titlebar.y, titlebar.w, WHITE);

        draw_label(
            canvas,
            Rect::new(titlebar.x + 10, titlebar.y + 1, titlebar.w - 82, titlebar.h),
            win.title,
            WHITE,
        );

        let min = win.minimize_button_rect();
        canvas.fill_vertical_gradient(
            min,
            if desktop.minimize_hover == Some(idx) {
                MINMAX_HOVER_TOP
            } else {
                MINMAX_TOP
            },
            if desktop.minimize_hover == Some(idx) {
                MINMAX_HOVER_BOTTOM
            } else {
                MINMAX_BOTTOM
            },
        );
        canvas.stroke_rect(min, BUTTON_BORDER);
        canvas.hline(min.x + 1, min.y + 1, min.w - 2, WHITE);
        canvas.vline(min.x + 1, min.y + 1, min.h - 2, WHITE);
        draw_text(canvas, min.x + 6, min.y + 5, TEXT, None, "_");

        let max = win.maximize_button_rect();
        canvas.fill_vertical_gradient(
            max,
            if desktop.maximize_hover == Some(idx) {
                MINMAX_HOVER_TOP
            } else {
                MINMAX_TOP
            },
            if desktop.maximize_hover == Some(idx) {
                MINMAX_HOVER_BOTTOM
            } else {
                MINMAX_BOTTOM
            },
        );
        canvas.stroke_rect(max, BUTTON_BORDER);
        canvas.hline(max.x + 1, max.y + 1, max.w - 2, WHITE);
        canvas.vline(max.x + 1, max.y + 1, max.h - 2, WHITE);
        draw_text(
            canvas,
            max.x + 5,
            max.y + 4,
            TEXT,
            None,
            if win.maximized { "o" } else { "+" },
        );

        let close = win.close_button_rect();
        canvas.fill_vertical_gradient(
            close,
            if desktop.close_hover == Some(idx) {
                CLOSE_HOVER_TOP
            } else {
                CLOSE_TOP
            },
            if desktop.close_hover == Some(idx) {
                CLOSE_HOVER_BOTTOM
            } else {
                CLOSE_BOTTOM
            },
        );
        canvas.stroke_rect(close, BUTTON_BORDER);
        canvas.hline(close.x + 1, close.y + 1, close.w - 2, WHITE);
        canvas.vline(close.x + 1, close.y + 1, close.h - 2, WHITE);
        draw_text(canvas, close.x + 5, close.y + 4, WHITE, None, "X");

        let client = win.client_rect();
        canvas.fill_rect(client, WINDOW_BG);
        win.app.render(canvas, client, focused);
    }
}

fn draw_taskbar(canvas: &mut Canvas, desktop: &Desktop) {
    let tb = desktop.taskbar_rect();
    if !canvas.intersects(tb) {
        return;
    }

    canvas.fill_vertical_gradient(tb, TASKBAR_TOP, TASKBAR_BOTTOM);
    canvas.hline(tb.x, tb.y, tb.w, TASKBAR_EDGE_TOP);
    canvas.hline(tb.x, tb.bottom() - 1, tb.w, TASKBAR_EDGE_BOTTOM);

    let start = desktop.start_button_rect();
    canvas.fill_vertical_gradient(start, START_TOP, START_BOTTOM);
    canvas.stroke_rect(start, BUTTON_BORDER);
    canvas.hline(start.x + 1, start.y + 1, start.w - 2, WHITE);
    canvas.vline(start.x + 1, start.y + 1, start.h - 2, WHITE);

    let start_label = "Start";
    let start_tw = libui::text::measure_text(start_label);
    let start_tx = start.x + ((start.w - start_tw) / 2).max(0);
    let start_ty = start.y + ((start.h - 8) / 2).max(0);
    draw_text(canvas, start_tx, start_ty, START_TEXT, None, start_label);

    for i in 0..desktop.windows.len() {
        let rect = desktop.task_button_rect(i);
        if !canvas.intersects(rect) {
            continue;
        }

        let hovered = desktop.taskbar_hover_window == Some(i);
        let active = desktop.focused == Some(i) && !desktop.windows[i].minimized;
        let close_hover = desktop.taskbar_close_hover == Some(i);
        draw_task_button(
            canvas,
            rect,
            desktop.windows[i].title,
            hovered,
            active,
            close_hover,
        );
    }

    let clock = desktop.clock_rect();
    if canvas.intersects(clock) {
        draw_clock(canvas, clock);
    }
}

fn draw_task_button(
    canvas: &mut Canvas,
    rect: Rect,
    title: &str,
    hovered: bool,
    active: bool,
    close_hover: bool,
) {
    let (top, bottom, text_color) = if active {
        (
            TASK_BTN_ACTIVE_TOP,
            TASK_BTN_ACTIVE_BOTTOM,
            TASK_BTN_TEXT_ACTIVE,
        )
    } else if hovered {
        (
            TASK_BTN_HOVER_TOP,
            TASK_BTN_HOVER_BOTTOM,
            TASK_BTN_TEXT_ACTIVE,
        )
    } else {
        (TASK_BTN_IDLE_TOP, TASK_BTN_IDLE_BOTTOM, TASK_BTN_TEXT_IDLE)
    };

    canvas.fill_vertical_gradient(rect, top, bottom);
    canvas.stroke_rect(rect, BUTTON_BORDER);
    canvas.hline(rect.x + 1, rect.y + 1, rect.w - 2, TASK_BTN_INNER_TOP);
    canvas.vline(rect.x + 1, rect.y + 1, rect.h - 2, TASK_BTN_INNER_TOP);

    let close = Rect::new(
        rect.right() - 22,
        rect.y + ((rect.h - 16) / 2).max(0),
        18,
        16,
    );

    canvas.fill_vertical_gradient(
        close,
        if close_hover {
            TASK_CLOSE_HOVER_TOP
        } else {
            TASK_CLOSE_TOP
        },
        if close_hover {
            TASK_CLOSE_HOVER_BOTTOM
        } else {
            TASK_CLOSE_BOTTOM
        },
    );
    canvas.stroke_rect(close, BUTTON_BORDER);
    canvas.hline(close.x + 1, close.y + 1, close.w - 2, WHITE);
    canvas.vline(close.x + 1, close.y + 1, close.h - 2, WHITE);
    draw_text(canvas, close.x + 5, close.y + 4, WHITE, None, "X");

    let text_x = rect.x + 10;
    let text_y = rect.y + ((rect.h - 8) / 2).max(0);
    draw_text(canvas, text_x, text_y, text_color, None, title);
}

fn draw_clock(canvas: &mut Canvas, clock: Rect) {
    canvas.fill_vertical_gradient(clock, 0x0099CAF9, 0x0068A5E4);
    canvas.stroke_rect(clock, BUTTON_BORDER);
    canvas.hline(clock.x + 1, clock.y + 1, clock.w - 2, WHITE);
    canvas.vline(clock.x + 1, clock.y + 1, clock.h - 2, WHITE);

    let formatted = match wall_time() {
        Ok(ts) => format_wall_datetime(ts.secs),
        Err(_) => FormattedWallDateTime {
            time: *b"--:--",
            date: {
                let mut buf = [0u8; 16];
                let s = b"-- --- ----";
                buf[..s.len()].copy_from_slice(s);
                buf
            },
            date_len: 10,
        },
    };

    let time_str = core::str::from_utf8(&formatted.time).unwrap_or("--:--");
    let date_str =
        core::str::from_utf8(&formatted.date[..formatted.date_len]).unwrap_or("-- --- ----");

    let time_w = libui::text::measure_text(time_str);
    let date_w = libui::text::measure_text(date_str);

    let time_x = clock.x + ((clock.w - time_w) / 2).max(0);
    let date_x = clock.x + ((clock.w - date_w) / 2).max(0);

    draw_text(canvas, time_x, clock.y + 3, PANEL_TEXT, None, time_str);
    draw_text(canvas, date_x, clock.y + 13, PANEL_TEXT, None, date_str);
}

fn draw_start_menu(canvas: &mut Canvas, desktop: &Desktop) {
    if !desktop.start_menu_open {
        return;
    }

    let menu = desktop.start_menu_rect();
    if !canvas.intersects(menu)
        && !canvas.intersects(Rect::new(menu.right(), menu.y, SUBMENU_W + 8, menu.h + 64))
    {
        return;
    }

    canvas.fill_rect(menu, START_MENU_BG);
    canvas.stroke_rect(menu, START_MENU_BORDER);

    let items = desktop.start_menu_items();

    for (i, item) in items.iter().enumerate() {
        let rect = desktop.start_menu_item_rect(i);
        let hovered = desktop.menu_hover == Some(i);

        if item.action.is_some() && item.submenu.is_empty() {
            let btn = rect.inset(2, 2);
            canvas.fill_vertical_gradient(
                btn,
                if hovered {
                    SHUTDOWN_HOVER_TOP
                } else {
                    SHUTDOWN_TOP
                },
                if hovered {
                    SHUTDOWN_HOVER_BOTTOM
                } else {
                    SHUTDOWN_BOTTOM
                },
            );
            canvas.stroke_rect(btn, BUTTON_BORDER);
            canvas.hline(btn.x + 1, btn.y + 1, btn.w - 2, WHITE);
            canvas.vline(btn.x + 1, btn.y + 1, btn.h - 2, WHITE);

            let tw = libui::text::measure_text(item.label);
            let tx = btn.x + ((btn.w - tw) / 2).max(0);
            let ty = btn.y + ((btn.h - 8) / 2).max(0);
            draw_text(canvas, tx, ty, WHITE, None, item.label);
        } else {
            if hovered {
                canvas.fill_rect(rect, START_MENU_HOVER);
            }

            draw_label(
                canvas,
                Rect::new(rect.x + 8, rect.y, rect.w - 20, rect.h),
                item.label,
                if hovered { WHITE } else { TEXT },
            );

            if !item.submenu.is_empty() {
                draw_text(
                    canvas,
                    rect.right() - 14,
                    rect.y + 8,
                    if hovered { WHITE } else { TEXT },
                    None,
                    ">",
                );
            }
        }
    }

    if let Some(item_idx) = desktop.menu_hover {
        let submenu = items[item_idx].submenu;
        if !submenu.is_empty() {
            let sub = desktop.submenu_rect(item_idx);
            canvas.fill_rect(sub, START_MENU_BG);
            canvas.stroke_rect(sub, START_MENU_BORDER);

            for (sub_idx, subitem) in submenu.iter().enumerate() {
                let rect = desktop.submenu_item_rect(item_idx, sub_idx);
                let hovered = desktop.submenu_hover == Some(sub_idx);

                if hovered {
                    canvas.fill_rect(rect, START_MENU_HOVER);
                }

                draw_label(
                    canvas,
                    Rect::new(rect.x + 8, rect.y, rect.w - 16, rect.h),
                    subitem.label,
                    if hovered { WHITE } else { TEXT },
                );
            }
        }
    }

    let _ = (
        START_W,
        TASK_BUTTON_W,
        TASKBAR_H,
        MENU_ITEM_H,
        SUBMENU_W,
        BG,
    );
}

fn blit_rgba_icon(
    fb: *mut u32,
    pitch_pixels: usize,
    screen_w: usize,
    screen_h: usize,
    x: i32,
    y: i32,
    icon_w: usize,
    icon_h: usize,
    rgba: &[u8],
    clip_hint: Option<Rect>,
) {
    let clip = clip_hint.unwrap_or(Rect::new(0, 0, screen_w as i32, screen_h as i32));

    for iy in 0..icon_h {
        let dy = y + iy as i32;
        if dy < 0 || dy >= screen_h as i32 || dy < clip.y || dy >= clip.bottom() {
            continue;
        }

        for ix in 0..icon_w {
            let dx = x + ix as i32;
            if dx < 0 || dx >= screen_w as i32 || dx < clip.x || dx >= clip.right() {
                continue;
            }

            let src = (iy * icon_w + ix) * 4;
            if src + 4 > rgba.len() {
                return;
            }

            let sr = rgba[src] as u32;
            let sg = rgba[src + 1] as u32;
            let sb = rgba[src + 2] as u32;
            let sa = rgba[src + 3] as u32;

            if sa == 0 {
                continue;
            }

            unsafe {
                let p = fb.add(dy as usize * pitch_pixels + dx as usize);
                let dst = *p;
                let dr = (dst >> 16) & 0xff;
                let dg = (dst >> 8) & 0xff;
                let db = dst & 0xff;

                let out = if sa >= 255 {
                    (sr << 16) | (sg << 8) | sb
                } else {
                    let inv = 255 - sa;
                    let rr = (sr * sa + dr * inv) / 255;
                    let rg = (sg * sa + dg * inv) / 255;
                    let rb = (sb * sa + db * inv) / 255;
                    (rr << 16) | (rg << 8) | rb
                };

                *p = out;
            }
        }
    }
}

struct FormattedWallDateTime {
    time: [u8; 5],
    date: [u8; 16],
    date_len: usize,
}

fn format_wall_datetime(epoch_secs: u64) -> FormattedWallDateTime {
    let dt = unix_to_datetime(epoch_secs);

    let mut time = *b"00:00";
    write_2(&mut time, 0, dt.hour as u32);
    time[2] = b':';
    write_2(&mut time, 3, dt.min as u32);

    let mut date = [0u8; 16];
    let mut n = 0usize;

    n += write_u32_no_pad(&mut date[n..], dt.day as u32);
    date[n] = b' ';
    n += 1;

    let month = month_short_name(dt.month);
    date[n..n + month.len()].copy_from_slice(month.as_bytes());
    n += month.len();

    date[n] = b' ';
    n += 1;

    write_4(&mut date, n, dt.year);
    n += 4;

    FormattedWallDateTime {
        time,
        date,
        date_len: n,
    }
}

fn month_short_name(month: u8) -> &'static str {
    match month {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        12 => "Dec",
        _ => "???",
    }
}

fn write_u32_no_pad(buf: &mut [u8], value: u32) -> usize {
    if value >= 100 {
        buf[0] = b'0' + ((value / 100) % 10) as u8;
        buf[1] = b'0' + ((value / 10) % 10) as u8;
        buf[2] = b'0' + (value % 10) as u8;
        3
    } else if value >= 10 {
        buf[0] = b'0' + ((value / 10) % 10) as u8;
        buf[1] = b'0' + (value % 10) as u8;
        2
    } else {
        buf[0] = b'0' + value as u8;
        1
    }
}

#[derive(Clone, Copy)]
struct DateTimeParts {
    year: u32,
    month: u8,
    day: u8,
    hour: u8,
    min: u8,
}

fn unix_to_datetime(epoch_secs: u64) -> DateTimeParts {
    let mut days = epoch_secs / 86_400;
    let secs_of_day = epoch_secs % 86_400;

    let hour = (secs_of_day / 3600) as u8;
    let min = ((secs_of_day % 3600) / 60) as u8;

    let mut year: u32 = 1970;
    loop {
        let ydays = if is_leap(year) { 366 } else { 365 };
        if days >= ydays {
            days -= ydays;
            year += 1;
        } else {
            break;
        }
    }

    let month_days = [
        31u32,
        if is_leap(year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];

    let mut month: u8 = 1;
    for md in month_days {
        if days >= md as u64 {
            days -= md as u64;
            month += 1;
        } else {
            break;
        }
    }

    let day = (days + 1) as u8;

    DateTimeParts {
        year,
        month,
        day,
        hour,
        min,
    }
}

fn is_leap(year: u32) -> bool {
    (year % 4 == 0) && ((year % 100 != 0) || (year % 400 == 0))
}

fn write_2(buf: &mut [u8], off: usize, value: u32) {
    buf[off] = b'0' + ((value / 10) % 10) as u8;
    buf[off + 1] = b'0' + (value % 10) as u8;
}

fn write_4(buf: &mut [u8], off: usize, value: u32) {
    buf[off] = b'0' + ((value / 1000) % 10) as u8;
    buf[off + 1] = b'0' + ((value / 100) % 10) as u8;
    buf[off + 2] = b'0' + ((value / 10) % 10) as u8;
    buf[off + 3] = b'0' + (value % 10) as u8;
}

fn draw_cursor(canvas: &mut Canvas, x: i32, y: i32, kind: CursorKind) {
    match kind {
        CursorKind::Arrow => draw_arrow_cursor(canvas, x, y),
        CursorKind::Hand => draw_hand_cursor(canvas, x, y),
        CursorKind::IBeam => draw_ibeam_cursor(canvas, x, y),
    }
}

fn draw_cursor_bitmap(canvas: &mut Canvas, x: i32, y: i32, rows: &[&str]) {
    for (dy, row) in rows.iter().enumerate() {
        for (dx, b) in row.as_bytes().iter().copied().enumerate() {
            match b {
                b'X' => canvas.put_pixel(x + dx as i32, y + dy as i32, CURSOR_BORDER),
                b'.' => canvas.put_pixel(x + dx as i32, y + dy as i32, CURSOR_FILL),
                _ => {}
            }
        }
    }
}

fn draw_arrow_cursor(canvas: &mut Canvas, x: i32, y: i32) {
    const ARROW: &[&str] = &[
        "XX              ",
        "X.X             ",
        "X..X            ",
        "X...X           ",
        "X....X          ",
        "X.....X         ",
        "X......X        ",
        "X.......X       ",
        "X........X      ",
        "X.........X     ",
        "X..........X    ",
        "X.....XXXXXXX   ",
        "X....X          ",
        "X...X           ",
        "X..X            ",
        "XX              ",
        "                ",
        "                ",
        "                ",
        "                ",
        "                ",
        "                ",
    ];
    draw_cursor_bitmap(canvas, x, y, ARROW);
}

fn draw_hand_cursor(canvas: &mut Canvas, x: i32, y: i32) {
    const HAND: &[&str] = &[
        "   XX           ",
        "   X.X          ",
        "   X.X          ",
        "   X.X          ",
        "   X.X          ",
        "   X.X          ",
        "   X.X          ",
        "  XX.XXXXXXXX   ",
        " X...........X  ",
        " X...........X  ",
        " X...........X  ",
        " X...........X  ",
        " X...........X  ",
        "  X........XXX  ",
        "  X.......X     ",
        "  X......X      ",
        "   X....X       ",
        "   XXXXXX       ",
        "                ",
    ];
    draw_cursor_bitmap(canvas, x, y, HAND);
}

fn draw_ibeam_cursor(canvas: &mut Canvas, x: i32, y: i32) {
    const IBEAM: &[&str] = &[
        " XXXXXXX ",
        " XX...XX ",
        " XXXXXXX ",
        "    X    ",
        "    X    ",
        "    X    ",
        "    X    ",
        "    X    ",
        "    X    ",
        "    X    ",
        "    X    ",
        "    X    ",
        "    X    ",
        "    X    ",
        "    X    ",
        "    X    ",
        " XXXXXXX ",
        " XX...XX ",
        " XXXXXXX ",
    ];
    draw_cursor_bitmap(canvas, x, y, IBEAM);
}
