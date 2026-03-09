extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;

use rlibc::sched::yield_now;
use rlibc::time::wall_time;

use crate::boot::map_framebuffer;
use crate::desktop::Desktop;
use crate::input::{self, InputParser, ParsedInputEvent};
use crate::render;

use libui::event::{MouseButton as UiMouseButton, UiEvent};
use libui::geom::Rect;

const IDLE_YIELD_EVERY: u32 = 128;
const CLOCK_POLL_IDLE_EVERY: u32 = 64;

pub fn run() -> ! {
    let fb = match map_framebuffer() {
        Ok(v) => v,
        Err(_) => loop {
            let _ = yield_now();
        },
    };

    let mut desktop = Desktop::new(fb.width, fb.height);
    let mut parser = InputParser::new();

    let mut backbuffer: Vec<u32> = vec![0; fb.width.saturating_mul(fb.height)];

    render::draw(
        &desktop,
        backbuffer.as_mut_ptr(),
        fb.width,
        fb.width,
        fb.height,
        None,
    );
    present_rect(
        fb.ptr,
        fb.pitch_pixels,
        fb.width,
        fb.height,
        &backbuffer,
        Rect::new(0, 0, fb.width as i32, fb.height as i32),
    );

    let mut idle_iters = 0u32;
    let mut last_wall_secs = wall_time().ok().map(|ts| ts.secs);
    let mut last_clock_minute = last_wall_secs.map(|secs| secs / 60);

    loop {
        let mut did_work = false;

        while let Some(ev) = input::next_parsed_event(&mut parser) {
            did_work = true;

            match ev {
                ParsedInputEvent::MouseMove { dx, dy } => {
                    desktop.move_cursor(dx, dy);
                    let pos = desktop.cursor;
                    desktop.handle_event(UiEvent::MouseMove { pos });
                }

                ParsedInputEvent::MouseButton { button, pressed } => {
                    let pos = desktop.cursor;
                    let button = match button {
                        input::MouseButton::Left => UiMouseButton::Left,
                        input::MouseButton::Right => UiMouseButton::Right,
                        input::MouseButton::Middle => UiMouseButton::Middle,
                        input::MouseButton::Side => UiMouseButton::Other(0x113),
                        input::MouseButton::Extra => UiMouseButton::Other(0x114),
                        input::MouseButton::Unknown(code) => UiMouseButton::Other(code),
                    };

                    if pressed {
                        desktop.handle_event(UiEvent::MouseDown { pos, button });
                    } else {
                        desktop.handle_event(UiEvent::MouseUp { pos, button });
                    }
                }

                ParsedInputEvent::Key { key, pressed } => {
                    let code = key.to_code();

                    if pressed {
                        desktop.handle_event(UiEvent::KeyDown { code });
                    } else {
                        desktop.handle_event(UiEvent::KeyUp { code });
                    }
                }

                ParsedInputEvent::MouseWheel { delta } => {
                    let pos = desktop.cursor;
                    desktop.handle_event(UiEvent::MouseWheel { pos, delta });
                }
            }
        }

        let should_poll_clock = did_work || (idle_iters % CLOCK_POLL_IDLE_EVERY) == 0;
        if should_poll_clock {
            if let Ok(ts) = wall_time() {
                if last_wall_secs != Some(ts.secs) {
                    last_wall_secs = Some(ts.secs);
                }

                let minute = ts.secs / 60;
                if last_clock_minute != Some(minute) {
                    last_clock_minute = Some(minute);
                    desktop.mark_dirty_rect(desktop.clock_rect());
                }
            }
        }

        if let Some(dirty) = desktop.take_dirty_rect() {
            render::draw(
                &desktop,
                backbuffer.as_mut_ptr(),
                fb.width,
                fb.width,
                fb.height,
                Some(dirty),
            );
            present_rect(
                fb.ptr,
                fb.pitch_pixels,
                fb.width,
                fb.height,
                &backbuffer,
                dirty,
            );
        }

        if did_work {
            idle_iters = 0;
        } else {
            idle_iters = idle_iters.wrapping_add(1);

            if (idle_iters % IDLE_YIELD_EVERY) == 0 {
                let _ = yield_now();
            } else {
                for _ in 0..256 {
                    core::hint::spin_loop();
                }
            }
        }
    }
}

fn present_rect(
    fb_ptr: *mut u32,
    fb_pitch_pixels: usize,
    width: usize,
    height: usize,
    backbuffer: &[u32],
    rect: Rect,
) {
    if width == 0 || height == 0 || rect.w <= 0 || rect.h <= 0 {
        return;
    }

    let x0 = rect.x.max(0) as usize;
    let y0 = rect.y.max(0) as usize;
    let x1 = rect.right().min(width as i32).max(0) as usize;
    let y1 = rect.bottom().min(height as i32).max(0) as usize;

    if x0 >= x1 || y0 >= y1 {
        return;
    }

    let row_w = x1 - x0;

    for y in y0..y1 {
        let src_off = y * width + x0;
        let dst_off = y * fb_pitch_pixels + x0;

        unsafe {
            core::ptr::copy_nonoverlapping(
                backbuffer.as_ptr().add(src_off),
                fb_ptr.add(dst_off),
                row_w,
            );
        }
    }
}
