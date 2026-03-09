#![no_std]
#![no_main]

extern crate alloc;

use core::mem::size_of;

use rlibc::log::log;
use rlibc::{chan, sched, shm, vfs};

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    let _ = log("ipc_cli: panic\n");
    loop {
        let _ = sched::yield_now();
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Msg {
    magic: u32,
    kind: u32,
    a: u64,
    b: u64,
    c: u64,
}

const MAGIC: u32 = 0x4348_414E; // 'CHAN'
const KIND_INIT: u32 = 1;
const KIND_TICK: u32 = 2;

fn parse_u64_ascii(buf: &[u8]) -> Option<u64> {
    let mut i = 0usize;
    while i < buf.len()
        && (buf[i] == b' ' || buf[i] == b'\n' || buf[i] == b'\r' || buf[i] == b'\t')
    {
        i += 1;
    }
    if i >= buf.len() {
        return None;
    }

    let mut v: u64 = 0;
    let mut any = false;
    while i < buf.len() {
        let b = buf[i];
        if b < b'0' || b > b'9' {
            break;
        }
        any = true;
        v = v.saturating_mul(10).saturating_add((b - b'0') as u64);
        i += 1;
    }

    any.then_some(v)
}

fn bytes_to_msg(buf: &[u8]) -> Option<Msg> {
    if buf.len() < size_of::<Msg>() {
        return None;
    }

    let mut m = Msg {
        magic: 0,
        kind: 0,
        a: 0,
        b: 0,
        c: 0,
    };

    unsafe {
        core::ptr::copy_nonoverlapping(
            buf.as_ptr(),
            (&mut m as *mut Msg) as *mut u8,
            size_of::<Msg>(),
        );
    }

    Some(m)
}

fn u64_to_dec(mut v: u64, out: &mut [u8; 32]) -> usize {
    if v == 0 {
        out[0] = b'0';
        return 1;
    }

    let mut tmp = [0u8; 32];
    let mut n = 0usize;
    while v > 0 {
        let d = (v % 10) as u8;
        tmp[n] = b'0' + d;
        n += 1;
        v /= 10;
    }

    for i in 0..n {
        out[i] = tmp[n - 1 - i];
    }
    n
}

fn log_u64(prefix: &str, v: u64) {
    let mut dec = [0u8; 32];
    let n = u64_to_dec(v, &mut dec);
    let _ = log(prefix);
    let _ = log(unsafe { core::str::from_utf8_unchecked(&dec[..n]) });
    let _ = log("\n");
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let _ = log("ipc_cli: start\n");

    let chan_bytes = loop {
        match vfs::read("/tmp/ipc_chan", 64) {
            Ok(v) => break v,
            Err(e) => {
                if e.0 == 2 || e.0 == 11 {
                    let _ = sched::yield_now();
                    continue;
                }
                let _ = log("ipc_cli: failed to read /tmp/ipc_chan\n");
                loop {
                    let _ = sched::yield_now();
                }
            }
        }
    };

    let chan_id = match parse_u64_ascii(&chan_bytes) {
        Some(v) => v,
        None => {
            let _ = log("ipc_cli: could not parse chan id\n");
            loop {
                let _ = sched::yield_now();
            }
        }
    };

    log_u64("ipc_cli: chanId=", chan_id);

    let mut buf = [0u8; 128];
    let init_msg = loop {
        match chan::recv(chan_id, &mut buf) {
            Ok(n) => {
                if let Some(m) = bytes_to_msg(&buf[..n]) {
                    if m.magic == MAGIC && m.kind == KIND_INIT {
                        break m;
                    }
                }
            }
            Err(e) => {
                if e.0 == 11 {
                    let _ = sched::yield_now();
                    continue;
                }
                let _ = log("ipc_cli: chan_recv error\n");
                loop {
                    let _ = sched::yield_now();
                }
            }
        }
        let _ = sched::yield_now();
    };

    let shm_id = init_msg.a;
    let shm_size = init_msg.b;

    log_u64("ipc_cli: shmId=", shm_id);
    log_u64("ipc_cli: shmSize=", shm_size);

    let shm_va = match shm::map(shm_id, 0, 0) {
        Ok(va) => va,
        Err(_) => {
            let _ = log("ipc_cli: shm::map failed\n");
            loop {
                let _ = sched::yield_now();
            }
        }
    };

    let p = shm_va as *const u32;
    let mut ok = true;
    unsafe {
        for i in 1..64usize {
            if *p.add(i) != 0xAABBCCDD {
                ok = false;
                break;
            }
        }
    }

    if ok {
        let _ = log("ipc_cli: pattern OK\n");
    } else {
        let _ = log("ipc_cli: pattern MISMATCH\n");
    }

    let mut first_tick_logged = false;

    loop {
        match chan::recv(chan_id, &mut buf) {
            Ok(n) => {
                if let Some(m) = bytes_to_msg(&buf[..n]) {
                    if m.magic == MAGIC && m.kind == KIND_TICK {
                        if !first_tick_logged {
                            first_tick_logged = true;
                            let _ = log("ipc_cli: first tick received\n");
                        }
                    }
                }
            }
            Err(e) => {
                if e.0 == 11 {
                    let _ = sched::yield_now();
                    continue;
                }
                let _ = log("ipc_cli: chan_recv error\n");
                loop {
                    let _ = sched::yield_now();
                }
            }
        }

        let _ = sched::yield_now();
    }
}
