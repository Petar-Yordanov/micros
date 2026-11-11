#![no_std]
extern crate alloc;

use appkit::apps;
use appkit::{App,AppId,AppLaunch};
use libui::canvas::Canvas;
use libui::color::WINDOW_BG;
use libui::geom::{Point,Rect};
use libui::protocol::{self,GuiMessage,WM_SERVICE};

fn send(wm:u64,m:&GuiMessage) {
    // A full queue is never permitted to stall an application's renderer.
    let _=rlibc::chan::send(wm,m.bytes());
}
fn allocate_surface(w:i32,h:i32,capacity:u64)->Option<(u64,*mut u32,u64)> {
    if w<=0 || h<=0 || w>8192 || h>8192{return None;}
    let pixels=(w as u64).checked_mul(h as u64)?;
    // Geometric growth bounds the number of allocations when resizing.
    let reserved=pixels.max(capacity.saturating_mul(2));
    let bytes=reserved.checked_mul(4)?;
    if bytes>64*1024*1024{return None;}
    let id=rlibc::shm::create(bytes,0).ok()?;
    let va=rlibc::shm::map(id,0,0).ok()?;
    Some((id,va as *mut u32,reserved))
}
fn paint(app:&dyn App,ptr:*mut u32,w:i32,h:i32) {
    let mut canvas=Canvas::new(ptr,w as usize,w as usize,h as usize);
    canvas.fill_rect(Rect::new(0,0,w,h),WINDOW_BG);
    app.render(&mut canvas,Rect::new(0,0,w,h),true);
}
fn present(wm:u64,pid:u64,w:i32,h:i32) {
    let mut m=GuiMessage::new(protocol::PRESENT);
    m.pid=pid;m.x=0;m.y=0;m.w=w;m.h=h;
    send(wm,&m);
}
fn announce_cursor(wm:u64,pid:u64,app:&dyn App,pos:Point) {
    let mut m=GuiMessage::new(protocol::CURSOR);
    m.pid=pid;m.x=protocol::cursor_code(app.cursor(pos));
    send(wm,&m);
}

pub fn run(id:AppId)->! {
    let pid=rlibc::proc::getpid().unwrap_or(0) as u64;
    let wm=loop {
        if let Ok(ch)=rlibc::chan::find(WM_SERVICE){break ch;}
        let _=rlibc::sched::yield_now();
    };
    let events=rlibc::chan::create(0).unwrap_or_else(|_|rlibc::proc::exit(2));
    let (frame_w,frame_h)=id.frame_size();
    let mut width=frame_w-4;
    let mut height=frame_h-28;
    let (mut shm,mut ptr,mut capacity)=allocate_surface(width,height,0).unwrap_or_else(||rlibc::proc::exit(3));
    let mut app=apps::make_app(id);
    paint(&*app,ptr,width,height);
    let mut reg=GuiMessage::new(protocol::REGISTER);
    reg.pid=pid;reg.channel=events;reg.surface=shm;reg.x=id as i32;reg.w=width;reg.h=height;
    // WM's inbox is non-blocking. Retry until it accepts registration.
    loop {
        if rlibc::chan::send(wm,reg.bytes()).is_ok(){break;}
        let _=rlibc::sched::yield_now();
    }
    present(wm,pid,width,height);
    let _=rlibc::log::log("app_host: registered isolated window\n");
    let mut input_buf=[0u8;GuiMessage::SIZE];
    let mut last_clock_second=0u64;
    let mut pending:Option<GuiMessage>=None;
    let mut cursor=libui::event::CursorKind::Arrow;
    loop {
        let mut did_work=false;
        let mut needs_paint=false;
        let mut cursor_pos:Option<Point>=None;
        // Drain bounded batches to avoid starving compositor on input floods.
        for _ in 0..32 {
            let m=match pending.take(){
                Some(m)=>m,
                None=>{
                    let got=match rlibc::chan::recv(events,&mut input_buf){Ok(n)=>n,Err(_)=>break};
                    let Some(m)=GuiMessage::decode(&input_buf[..got]) else {continue};
                    m
                }
            };
            did_work=true;
            match m.kind {
                protocol::CLOSE=>rlibc::proc::exit(0),
                protocol::RESIZE=>{
                    let nw=m.w.clamp(1,8192);let nh=m.h.clamp(1,8192);
                    if nw!=width || nh!=height {
                        let target_pixels=(nw as u64).saturating_mul(nh as u64);
                        let mut okay=true;
                        if target_pixels>capacity {
                            match allocate_surface(nw,nh,capacity) {
                                Some((id,buffer,reserved))=>{
                                    let old=shm;
                                    shm=id;ptr=buffer;capacity=reserved;
                                    let _=rlibc::shm::unmap(old);
                                },
                                None=>okay=false,
                            }
                        }
                        if okay {
                            width=nw;height=nh;
                            needs_paint=true;
                            let mut msg=GuiMessage::new(protocol::SURFACE);
                            msg.pid=pid;msg.surface=shm;msg.w=width;msg.h=height;
                            send(wm,&msg);
                        }
                    }
                },
                protocol::OPEN_FILE if id==AppId::Notepad=>{
                    if let Some(path)=m.as_str(){app=apps::make_launch(AppLaunch::TextFile(alloc::string::String::from(path)));}
                    needs_paint=true;
                },
                _=>{
                    if let Some(ev)=m.to_event(){
                        if app.handle_event(&ev){needs_paint=true;}
                        if let libui::event::UiEvent::MouseMove{pos}=ev{cursor_pos=Some(pos);}
                    }
                },
            }
        }
        // One render per batch, even when 32 mouse events arrived. Avoids
        // turning an input flood into dozens of full client redraws.
        if let Some(pos)=cursor_pos {
            let new_cursor=app.cursor(pos);
            if new_cursor!=cursor {cursor=new_cursor;announce_cursor(wm,pid,&*app,pos);}
        }
        // Clock's original UI displays the time but doesn't generate mouse input.
        if id==AppId::Clock {
            if let Ok(t)=rlibc::time::wall_time() {
                if t.secs!=last_clock_second {last_clock_second=t.secs;needs_paint=true;did_work=true;}
            }
        }
        if needs_paint {paint(&*app,ptr,width,height);present(wm,pid,width,height);}
        if !did_work {
            // All apps sleep while idle. The clock gets a timed wake-up,
            // other apps wake only when the WM sends an event.
            let result=if id==AppId::Clock {
                rlibc::chan::recv_wait_timeout(events,&mut input_buf,1000)
            } else {
                rlibc::chan::recv_wait(events,&mut input_buf)
            };
            if let Ok(n)=result {pending=GuiMessage::decode(&input_buf[..n]);}
        }
    }
}
