extern crate alloc;

use alloc::{boxed::Box, string::String, vec, vec::Vec};

use crate::kernel::mm::map::mapper::{self as page, Prot};
use crate::kernel::mm::phys::frame;
use crate::kernel::mm::virt::vmarena;
use crate::ksprintln;
use crate::platform::limine::hhdm::HHDM_REQ;
use x86_64::VirtAddr;

unsafe fn ptr_of_slice(bx: &Box<[u8]>) -> usize {
    bx.as_ptr() as usize
}

#[inline(always)]
fn assert_eq_u64(tag: &str, a: u64, b: u64) {
    if a != b {
        panic!("[err] [TEST-FAIL] {}: {:#x} != {:#x}", tag, a, b);
    }
}

#[inline(always)]
fn assert_true(tag: &str, ok: bool) {
    if !ok {
        panic!("[err] [TEST-FAIL] {}", tag);
    }
}

pub fn test_frames() {
    ksprintln!("[test] frames");
    let f0 = frame::alloc().expect("frame alloc 0");
    let f1 = frame::alloc().expect("frame alloc 1");
    let f2 = frame::alloc().expect("frame alloc 2");

    let a0 = f0.start_address().as_u64();
    let a1 = f1.start_address().as_u64();
    let a2 = f2.start_address().as_u64();

    frame::free(f0);
    frame::free(f1);
    frame::free(f2);

    let g0 = frame::alloc().expect("frame re-alloc 0");
    let g1 = frame::alloc().expect("frame re-alloc 1");
    let g2 = frame::alloc().expect("frame re-alloc 2");

    assert_eq_u64("frame0 roundtrip", g0.start_address().as_u64(), a0);
    assert_eq_u64("frame1 roundtrip", g1.start_address().as_u64(), a1);
    assert_eq_u64("frame2 roundtrip", g2.start_address().as_u64(), a2);

    frame::free(g0);
    frame::free(g1);
    frame::free(g2);
    ksprintln!("[test] frames OK");
}

pub fn test_paging() {
    ksprintln!("[test] paging");

    let va = vmarena::alloc().expect("arena va");
    assert_true("arena gave va", vmarena::is_mapped(va));

    let orig = page::unmap(va).expect("unmap arena page");
    frame::free(orig);
    assert_true("va now unmapped", page::translate(va).is_none());

    let f = frame::alloc().expect("frame for map_fixed");
    page::map_fixed(va, f, Prot::RW).expect("map_fixed");

    let pa = f.start_address();
    let hhdm = HHDM_REQ.get_response().unwrap().offset();
    let pa_hhdm = VirtAddr::new(hhdm + pa.as_u64());

    unsafe {
        core::ptr::write::<u64>(va.as_mut_ptr(), 0xDEADBEEFCAFEBABEu64);
        let back = core::ptr::read::<u64>(pa_hhdm.as_ptr());
        assert_eq_u64("roundtrip VA->PA(HHDM)", back, 0xDEADBEEFCAFEBABE);
    }

    let f_back = page::unmap(va).expect("unmap test page");
    frame::free(f_back);
    vmarena::free(va);

    ksprintln!("[test] paging OK");
}

pub fn test_vmarena() {
    ksprintln!("[test] vmarena");

    let base1 = vmarena::alloc_n(4).expect("vmarena alloc 4");
    unsafe {
        let p0 = base1.as_u64() as *mut u64;
        core::ptr::write_volatile(p0, 0x1111_2222_3333_4444);

        let plast = (base1.as_u64() + 3 * 4096) as *mut u64;
        core::ptr::write_volatile(plast, 0xAAAA_BBBB_CCCC_DDDD);

        assert_eq_u64(
            "vmarena read first",
            core::ptr::read_volatile(p0),
            0x1111_2222_3333_4444,
        );
        assert_eq_u64(
            "vmarena read last",
            core::ptr::read_volatile(plast),
            0xAAAA_BBBB_CCCC_DDDD,
        );
    }
    vmarena::free_n(base1, 4);

    let base2 = vmarena::alloc_n(4).expect("vmarena realloc 4");
    assert_eq_u64(
        "vmarena same base after free",
        base2.as_u64(),
        base1.as_u64(),
    );

    vmarena::free_n(base2, 4);
    ksprintln!("[test] vmarena OK");
}

pub fn test_heap() {
    ksprintln!("[test] heap");

    {
        let b = Box::new(0x1234_5678u64);
        assert_eq_u64("heap box", *b, 0x1234_5678);
        let mut v = Vec::<u64>::with_capacity(1024);
        for i in 0..1024 {
            v.push(i as u64);
        }
        assert_eq_u64("heap vec len", v.len() as u64, 1024);
        let s = String::from("hello heap");
        assert_true("heap string non-empty", !s.is_empty());
    }

    let a: Box<[u8]> = vec![0u8; 3 * 1024].into_boxed_slice();
    let b: Box<[u8]> = vec![0u8; 16 * 1024].into_boxed_slice();
    let c: Box<[u8]> = vec![0u8; 4 * 1024].into_boxed_slice();

    let pb = unsafe { ptr_of_slice(&b) };
    drop(b);

    let x: Box<[u8]> = vec![0u8; 8 * 1024].into_boxed_slice();
    let px = unsafe { ptr_of_slice(&x) };
    assert_true(
        "heap reused freed middle block",
        px >= pb && px < (pb + 32 * 1024),
    );

    drop(a);
    drop(x);
    drop(c);

    let big: Box<[u8]> = vec![0u8; 512 * 1024].into_boxed_slice();
    assert_true("heap big alloc ok", big.len() == 512 * 1024);
    drop(big);

    ksprintln!("[test] heap OK");
}

pub fn test_virtio_net() {
    use crate::kernel::drivers::virtio::net;

    ksprintln!("[test] virtio-net");

    if !net::is_ready() {
        ksprintln!("[test] virtio-net SKIP (no device attached)");
        return;
    }

    let mtu = net::mtu().unwrap_or(0);
    assert_true("virtio-net mtu nonzero", mtu != 0);

    if let Some(mac) = net::mac_addr() {
        let all_zero = mac.iter().all(|&b| b == 0);
        let all_ff = mac.iter().all(|&b| b == 0xFF);

        assert_true("virtio-net mac not all zero", !all_zero);
        assert_true("virtio-net mac not broadcast", !all_ff);
        assert_true("virtio-net mac is unicast", (mac[0] & 1) == 0);

        ksprintln!(
            "[virtio-net] mac={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            mac[0],
            mac[1],
            mac[2],
            mac[3],
            mac[4],
            mac[5]
        );
    } else {
        ksprintln!("[virtio-net] mac=(not advertised)");
    }

    let link = net::link_up();
    ksprintln!(
        "[virtio-net] link={} mtu={}",
        if link { "up" } else { "down" },
        mtu
    );

    let mut scratch = [0u8; 4096];
    let mut polls = 0usize;
    let mut frames = 0usize;

    for _ in 0..8 {
        polls += 1;

        match net::recv_frame(&mut scratch) {
            None => {}
            Some(n) => {
                assert_true("virtio-net rx frame fits scratch", n <= scratch.len());

                if n != 0 {
                    frames += 1;

                    if n >= 14 {
                        let ethertype = ((scratch[12] as u16) << 8) | (scratch[13] as u16);
                        ksprintln!(
                            "[virtio-net] rx len={} dst={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x} src={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x} ethertype={:#06x}",
                            n,
                            scratch[0],
                            scratch[1],
                            scratch[2],
                            scratch[3],
                            scratch[4],
                            scratch[5],
                            scratch[6],
                            scratch[7],
                            scratch[8],
                            scratch[9],
                            scratch[10],
                            scratch[11],
                            ethertype
                        );
                    } else {
                        ksprintln!("[virtio-net] rx short frame len={}", n);
                    }
                }
            }
        }
    }

    ksprintln!(
        "[test] virtio-net RX poll OK (polls={} frames={})",
        polls,
        frames
    );
    ksprintln!("[test] virtio-net OK");
}
