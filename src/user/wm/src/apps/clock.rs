use libui::canvas::Canvas;
use libui::color::{BUTTON_BORDER, PANEL, TEXT_DIM};
use libui::event::{CursorKind, UiEvent};
use libui::geom::{Point, Rect};
use libui::text::{draw_text, measure_text};
use crate::alloc::string::ToString;
use crate::app::App;

const FACE_BG: u32 = 0x00F2F2F2;
const FACE_BORDER: u32 = 0x00282828;
const TICK_COLOR: u32 = 0x00181818;
const HAND_COLOR: u32 = 0x00181818;
const HUB_COLOR: u32 = 0x00202020;

pub struct ClockApp;

impl ClockApp {
    pub fn new() -> Self {
        Self
    }
}

impl App for ClockApp {
    fn title(&self) -> &'static str {
        "CLOCK"
    }

    fn handle_event(&mut self, _ev: &UiEvent) -> bool {
        false
    }

    fn cursor(&self, _local_pos: Point) -> CursorKind {
        CursorKind::Arrow
    }

    fn render(&self, canvas: &mut Canvas, client_rect: Rect, _focused: bool) {
        canvas.fill_rect(client_rect, PANEL);

        let outer = client_rect.inset(8, 8);
        canvas.stroke_rect(outer, BUTTON_BORDER);

        let header = Rect::new(outer.x + 8, outer.y + 6, outer.w - 16, 14);
        let date_box = Rect::new(outer.x + 24, outer.bottom() - 34, outer.w - 48, 22);

        draw_text(canvas, header.x, header.y, TEXT_DIM, None, "Current local time");

        let date_str = match rlibc::time::wall_time() {
            Ok(ts) => {
                let dt = unix_to_datetime(ts.secs);
                format_date(dt.day as u32, dt.month, dt.year)
            }
            Err(_) => "Unknown date".to_string(),
        };

        let face_top = header.bottom() + 8;
        let face_bottom = date_box.y - 10;
        let face_h = (face_bottom - face_top).max(80);
        let face_w = outer.w - 20;
        let face_size = core::cmp::min(face_w, face_h);
        let radius = (face_size / 2).max(24) - 2;
        let cx = outer.x + outer.w / 2;
        let cy = face_top + face_h / 2;

        draw_clock_face(canvas, cx, cy, radius);

        if let Ok(ts) = rlibc::time::wall_time() {
            let dt = unix_to_datetime(ts.secs);
            draw_clock_hands(canvas, cx, cy, radius, dt.hour, dt.min);
        }

        let date_w = measure_text(&date_str);
        let date_x = date_box.x + ((date_box.w - date_w) / 2).max(0);
        let date_y = date_box.y + ((date_box.h - 8) / 2).max(0);

        canvas.fill_rect(date_box, 0x00E8E6D8);
        canvas.stroke_rect(date_box, BUTTON_BORDER);
        draw_text(canvas, date_x, date_y, 0x00000000, None, &date_str);
    }
}

fn draw_clock_face(canvas: &mut Canvas, cx: i32, cy: i32, radius: i32) {
    fill_circle(canvas, cx, cy, radius, FACE_BG);

    draw_ring(canvas, cx, cy, radius, 3, FACE_BORDER);

    for i in 0..60 {
        let angle = minute_angle_rad(i);
        let (sin_a, cos_a) = sin_cos(angle);

        let outer_r = radius - 6;
        let inner_r = if i % 5 == 0 { radius - 22 } else { radius - 12 };

        let x0 = cx + round_to_i32(cos_a * inner_r as f64);
        let y0 = cy + round_to_i32(sin_a * inner_r as f64);
        let x1 = cx + round_to_i32(cos_a * outer_r as f64);
        let y1 = cy + round_to_i32(sin_a * outer_r as f64);

        if i % 5 == 0 {
            draw_thick_line_2(canvas, x0, y0, x1, y1, TICK_COLOR);
        } else {
            draw_line(canvas, x0, y0, x1, y1, TICK_COLOR);
        }
    }

    let label_r = radius - 38;
    draw_face_label(canvas, cx, cy, label_r, 12, "12");
    draw_face_label(canvas, cx, cy, label_r, 3, "3");
    draw_face_label(canvas, cx, cy, label_r, 6, "6");
    draw_face_label(canvas, cx, cy, label_r, 9, "9");
}

fn draw_face_label(canvas: &mut Canvas, cx: i32, cy: i32, r: i32, hour: i32, text: &str) {
    let deg = (hour as f64 * 30.0) - 90.0;
    let angle = deg * core::f64::consts::PI / 180.0;
    let (sin_a, cos_a) = sin_cos(angle);

    let tx = cx + round_to_i32(cos_a * r as f64);
    let ty = cy + round_to_i32(sin_a * r as f64) - 4;

    draw_text_centered(canvas, tx, ty, text, TICK_COLOR);
}

fn draw_clock_hands(canvas: &mut Canvas, cx: i32, cy: i32, radius: i32, hour: u8, min: u8) {
    let minute_angle = minute_hand_angle_rad(min);
    let (min_sin, min_cos) = sin_cos(minute_angle);

    let hour_angle = hour_hand_angle_rad(hour, min);
    let (hour_sin, hour_cos) = sin_cos(hour_angle);

    let min_len = radius - 30;
    let hour_len = radius - 52;

    let min_x = cx + round_to_i32(min_cos * min_len as f64);
    let min_y = cy + round_to_i32(min_sin * min_len as f64);

    let hour_x = cx + round_to_i32(hour_cos * hour_len as f64);
    let hour_y = cy + round_to_i32(hour_sin * hour_len as f64);

    draw_thick_line_3(canvas, cx, cy, hour_x, hour_y, HAND_COLOR);
    draw_thick_line_3(canvas, cx, cy, min_x, min_y, HAND_COLOR);

    fill_circle(canvas, cx, cy, 6, HUB_COLOR);
    draw_ring(canvas, cx, cy, 6, 2, HUB_COLOR);
}

fn draw_text_centered(canvas: &mut Canvas, cx: i32, cy: i32, text: &str, color: u32) {
    let w = measure_text(text);
    draw_text(canvas, cx - w / 2, cy, color, None, text);
}

fn draw_thick_line_2(canvas: &mut Canvas, x0: i32, y0: i32, x1: i32, y1: i32, color: u32) {
    draw_line(canvas, x0, y0, x1, y1, color);
    draw_line(canvas, x0 + 1, y0, x1 + 1, y1, color);
}

fn draw_thick_line_3(canvas: &mut Canvas, x0: i32, y0: i32, x1: i32, y1: i32, color: u32) {
    draw_line(canvas, x0, y0, x1, y1, color);
    draw_line(canvas, x0 + 1, y0, x1 + 1, y1, color);
    draw_line(canvas, x0, y0 + 1, x1, y1 + 1, color);
}

fn draw_line(canvas: &mut Canvas, mut x0: i32, mut y0: i32, x1: i32, y1: i32, color: u32) {
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    loop {
        canvas.put_pixel(x0, y0, color);

        if x0 == x1 && y0 == y1 {
            break;
        }

        let e2 = err * 2;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

fn draw_ring(canvas: &mut Canvas, cx: i32, cy: i32, r: i32, thickness: i32, color: u32) {
    for t in 0..thickness {
        draw_circle_outline(canvas, cx, cy, r - t, color);
    }
}

fn draw_circle_outline(canvas: &mut Canvas, cx: i32, cy: i32, r: i32, color: u32) {
    if r <= 0 {
        return;
    }

    let mut x = r;
    let mut y = 0;
    let mut err = 1 - x;

    while x >= y {
        plot_circle_points(canvas, cx, cy, x, y, color);
        y += 1;
        if err < 0 {
            err += 2 * y + 1;
        } else {
            x -= 1;
            err += 2 * (y - x) + 1;
        }
    }
}

fn fill_circle(canvas: &mut Canvas, cx: i32, cy: i32, r: i32, color: u32) {
    if r <= 0 {
        return;
    }

    let rr = r * r;
    for dy in -r..=r {
        let yy = dy * dy;
        let rem = rr - yy;
        if rem < 0 {
            continue;
        }
        let dx = isqrt(rem);
        for xx in -dx..=dx {
            canvas.put_pixel(cx + xx, cy + dy, color);
        }
    }
}

fn plot_circle_points(canvas: &mut Canvas, cx: i32, cy: i32, x: i32, y: i32, color: u32) {
    canvas.put_pixel(cx + x, cy + y, color);
    canvas.put_pixel(cx + y, cy + x, color);
    canvas.put_pixel(cx - y, cy + x, color);
    canvas.put_pixel(cx - x, cy + y, color);
    canvas.put_pixel(cx - x, cy - y, color);
    canvas.put_pixel(cx - y, cy - x, color);
    canvas.put_pixel(cx + y, cy - x, color);
    canvas.put_pixel(cx + x, cy - y, color);
}

fn isqrt(n: i32) -> i32 {
    if n <= 0 {
        return 0;
    }

    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

fn minute_angle_rad(minute: i32) -> f64 {
    let deg = (minute as f64 * 6.0) - 90.0;
    deg * core::f64::consts::PI / 180.0
}

fn minute_hand_angle_rad(minute: u8) -> f64 {
    minute_angle_rad(minute as i32)
}

fn hour_hand_angle_rad(hour: u8, minute: u8) -> f64 {
    let h = (hour % 12) as f64;
    let m = minute as f64;
    let deg = (h * 30.0) + (m * 0.5) - 90.0;
    deg * core::f64::consts::PI / 180.0
}

fn sin_cos(angle: f64) -> (f64, f64) {
    (libm::sin(angle), libm::cos(angle))
}

fn round_to_i32(v: f64) -> i32 {
    if v >= 0.0 {
        (v + 0.5) as i32
    } else {
        (v - 0.5) as i32
    }
}

fn format_date(day: u32, month: u8, year: u32) -> alloc::string::String {
    alloc::format!("{day} {} {year}", month_name(month))
}

fn month_name(month: u8) -> &'static str {
    match month {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "Unknown",
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
