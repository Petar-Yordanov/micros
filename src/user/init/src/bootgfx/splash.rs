use super::fb::get_fb;
use super::gfx::Surface;

const BLACK: u32 = 0x000000;
const WHITE: u32 = 0xF3F6FA;
const MICR: u32 = 0xC8CDD5;
const OS: u32 = 0x4AA3FF;
const BAR_FRAME: u32 = 0x5A6470;
const BAR_BG: u32 = 0x0D1218;
const BAR_SEG: u32 = 0x5AA8FF;
const BAR_SEG_HI: u32 = 0xB8D9FF;
const BAR_W: usize = 240;
const BAR_H: usize = 22;
const BAR_SEG_W: usize = 18;
const BAR_GAP: usize = 10;

fn busy_delay(iterations: usize) {
    let mut i = 0usize;
    while i < iterations {
        core::hint::spin_loop();
        i += 1;
    }
}

fn draw_branding(s: &mut Surface<'_>) {
    let big = 8usize;
    let small = 4usize;

    let micr_w = 4 * 6 * big;
    let os_w = 2 * 6 * big;
    let sup_w = 2 * 6 * small;
    let total_w = micr_w + os_w + sup_w;

    let x0 = (s.width().saturating_sub(total_w)) / 2;
    let y0 = s.height() / 2usize - (7 * big) / 2usize - 30;

    s.draw_text_5x7(x0, y0, "Micr", big, MICR);
    s.draw_text_5x7(x0 + micr_w, y0, "OS", big, OS);

    let sup_x = x0 + micr_w + os_w - small;
    let sup_y = y0.saturating_sub(2 * small);
    s.draw_text_5x7(sup_x, sup_y, "64", small, WHITE);
}

pub fn draw_loading_bar(s: &mut Surface<'_>, frame: usize) {
    let w = core::cmp::min(BAR_W, s.width().saturating_sub(120));
    let h = BAR_H;
    let x = (s.width().saturating_sub(w)) / 2;
    let y = s.height() / 2 + 68;

    s.fill_rect(x, y, w, h, BAR_BG);
    s.stroke_rect(x, y, w, h, BAR_FRAME);

    let inner_x = x + 3;
    let inner_y = y + 3;
    let inner_w = w.saturating_sub(6);
    let inner_h = h.saturating_sub(6);

    let step = BAR_SEG_W + BAR_GAP;
    let cycle = inner_w + 4 * step;
    let base = (frame % cycle) as isize - (3 * step) as isize;

    let mut i = 0usize;
    while i < 4 {
        let pos = base + (i * step) as isize;
        let sx = inner_x as isize + pos;

        if sx < (inner_x + inner_w) as isize && sx + BAR_SEG_W as isize > inner_x as isize {
            let draw_x = core::cmp::max(sx, inner_x as isize) as usize;
            let seg_end =
                core::cmp::min(sx + BAR_SEG_W as isize, (inner_x + inner_w) as isize) as usize;
            let draw_w = seg_end.saturating_sub(draw_x);

            if draw_w != 0 {
                s.fill_rect(draw_x, inner_y, draw_w, inner_h, BAR_SEG);
                if inner_h > 4 && draw_w > 4 {
                    s.fill_rect(draw_x + 1, inner_y + 1, draw_w - 2, 2, BAR_SEG_HI);
                }
            }
        }

        i += 1;
    }
}

fn clear_loading_bar_area(s: &mut Surface<'_>) {
    let w = core::cmp::min(BAR_W, s.width().saturating_sub(120));
    let h = BAR_H;
    let x = (s.width().saturating_sub(w)) / 2;
    let y = s.height() / 2 + 68;
    s.fill_rect(x, y, w, h, BLACK);
}

pub fn run_splash() {
    let Some((info, buf)) = get_fb() else {
        return;
    };

    let mut surf = Surface { info, buf };

    surf.clear(BLACK);
    draw_branding(&mut surf);

    let mut frame = 0usize;
    while frame < 320 {
        clear_loading_bar_area(&mut surf);
        draw_loading_bar(&mut surf, frame * 3);
        busy_delay(250_000);
        frame += 1;
    }
}
