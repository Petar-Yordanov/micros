#![no_std]
#![no_main]

extern crate alloc;

use core::mem::size_of;

use rlibc::log::log;
use rlibc::{chan, proc, sched, shm, vfs};

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    let _ = log("ipc_srv: panic\n");
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

const MAGIC: u32 = 0x4348_414E;
const KIND_INIT: u32 = 1;
const KIND_TICK: u32 = 2;

fn msg_as_bytes(m: &Msg) -> &[u8] {
    unsafe { core::slice::from_raw_parts((m as *const Msg) as *const u8, size_of::<Msg>()) }
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

fn log_err(errno: i32) {
    let mut dec = [0u8; 32];
    let n = u64_to_dec(errno as u64, &mut dec);
    let _ = log("errno=");
    let _ = log(unsafe { core::str::from_utf8_unchecked(&dec[..n]) });
    let _ = log("\n");
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let _ = log("ipc_srv: start\n");

    let chan_id = match chan::create(0) {
        Ok(id) => id,
        Err(e) => {
            let _ = log("ipc_srv: chan::create failed\n");
            log_err(e.0);
            loop {
                let _ = sched::yield_now();
            }
        }
    };

    let _ = vfs::mkdir_p("/tmp");

    let mut dec = [0u8; 32];
    let n = u64_to_dec(chan_id, &mut dec);

    if vfs::write("/tmp/ipc_chan", &dec[..n]).is_err() {
        let _ = log("ipc_srv: vfs::write(/tmp/ipc_chan) failed\n");
        loop {
            let _ = sched::yield_now();
        }
    }

    let _ = log("ipc_srv: wrote /tmp/ipc_chan\n");

    let shm_size: u64 = 4096;
    let shm_id = match shm::create(shm_size, 0) {
        Ok(id) => id,
        Err(e) => {
            let _ = log("ipc_srv: shm::create failed\n");
            log_err(e.0);
            loop {
                let _ = sched::yield_now();
            }
        }
    };

    let shm_va = match shm::map(shm_id, 0, 0) {
        Ok(va) => va,
        Err(e) => {
            let _ = log("ipc_srv: shm::map failed\n");
            log_err(e.0);
            loop {
                let _ = sched::yield_now();
            }
        }
    };

    let p = shm_va as *mut u32;
    unsafe {
        for i in 0..64usize {
            *p.add(i) = 0xAABBCCDD;
        }
        *p.add(0) = 1;
    }

    let init = Msg {
        magic: MAGIC,
        kind: KIND_INIT,
        a: shm_id,
        b: shm_size,
        c: 0,
    };
    let _ = chan::send(chan_id, msg_as_bytes(&init));

    let _ = log("ipc_srv: sent init msg\n");

    match proc::spawn("/bin/ipc_cli.elf") {
        Ok(pid) => {
            let _ = log("ipc_srv: spawned ipc_cli\n");
            log_u64("ipc_srv: ipc_cli pid=", pid as u64);
        }
        Err(_) => {
            let _ = log("ipc_srv: failed to spawn /bin/ipc_cli.elf\n");
        }
    }

    let mut counter: u64 = 1;
    loop {
        counter = counter.wrapping_add(1);

        unsafe {
            *p.add(0) = counter as u32;
        }

        let tick = Msg {
            magic: MAGIC,
            kind: KIND_TICK,
            a: counter,
            b: unsafe { *p.add(0) as u64 },
            c: 0,
        };

        let _ = chan::send(chan_id, msg_as_bytes(&tick));
        let _ = sched::yield_now();
    }
}
