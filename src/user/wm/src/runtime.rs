extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;

use rlibc::sched::yield_now;
use rlibc::chan;
use libui::protocol::{GuiMessage,WM_SERVICE};
use rlibc::time::wall_time;

use crate::boot::map_framebuffer;
use crate::desktop::Desktop;
use crate::input::{self, InputParser, ParsedInputEvent};
use crate::render;

use libui::event::{MouseButton as UiMouseButton, UiEvent};
use libui::geom::Rect;

// Bound idle CPU consumption without visibly delaying input/IPC polling.
const IDLE_SLEEP_MS: u64 = 4;
const CLOCK_POLL_IDLE_EVERY: u32 = 64;

pub fn run() -> ! {
    let fb = match map_framebuffer() {
        Ok(v) => v,
        Err(_) => loop {
            let _ = yield_now();
        },
    };

    let wm_inbox=match chan::create(0) {
        Ok(id)=>id,
        Err(_)=>rlibc::proc::exit(10),
    };
    if chan::publish(WM_SERVICE,wm_inbox).is_err() {rlibc::proc::exit(11);}
    let _=rlibc::log::log("wm: service micros.wm registered\n");
    let mut desktop = Desktop::new(fb.width, fb.height);
    let mut ipc_buffer=[0u8;GuiMessage::SIZE];
    let mut parser = InputParser::new();

    let mut backbuffer: Vec<u32> = vec![0; fb.width.saturating_mul(fb.height)];
    let mut dirty_regions: Vec<Rect> = Vec::new();

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
    let mut last_composited_window_count = 0usize;

    loop {
        let mut did_work = false;
        // Drain a bounded batch of small control messages without waiting for
        // applications or copying their framebuffers through the channel.
        for _ in 0..128 {
            let n=match chan::recv(wm_inbox,&mut ipc_buffer){Ok(n)=>n,Err(_)=>break};
            if let Some(msg)=GuiMessage::decode(&ipc_buffer[..n]) {
                desktop.handle_ipc(&msg);
                did_work=true;
            }
        }

        // Do not let a continuous mouse/event stream starve composition.
        // Unprocessed events remain in the kernel queue for the next frame.
        for _ in 0..96 {
            let Some(ev) = input::next_parsed_event(&mut parser) else { break; };
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
                    desktop.reap_exited();
                }

                let minute = ts.secs / 60;
                if last_clock_minute != Some(minute) {
                    last_clock_minute = Some(minute);
                    desktop.mark_dirty_rect(desktop.clock_rect());
                }
            }
        }

        desktop.take_dirty_rects(&mut dirty_regions);
        if !dirty_regions.is_empty() {
            let first_window_frame = desktop.windows.len() > last_composited_window_count;
            if first_window_frame {
                let _ = rlibc::log::log("wm: composing first application frame\n");
            }
            for dirty in dirty_regions.iter().copied() {
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
            if first_window_frame {
                let _ = rlibc::log::log("wm: first application frame presented\n");
            }
            last_composited_window_count = desktop.windows.len();
        }

        // Cleanup must follow drawing, not block a close-button handler.
        // Both SHM unmap and process VM teardown are bounded to one batch
        // per compositor iteration.
        desktop.service_cleanup();

        if did_work {
            idle_iters = 0;
        } else {
            idle_iters = idle_iters.wrapping_add(1);

            // SYS_YIELD only requests a deferred switch; an unconditional
            // tight yield loop saturates the syscall path. Sleep for a small
            // number of timer ticks when there is no work instead.
            if rlibc::sched::sleep_ms(IDLE_SLEEP_MS).is_err() {
                let _ = yield_now();
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
