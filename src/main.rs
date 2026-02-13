#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]

extern crate alloc;

mod arch;
mod kernel;
mod platform;

use crate::arch::x86_64::cpu::cpuid;
use crate::arch::x86_64::serial;
use crate::arch::x86_64::time::apic;
use crate::kernel::input::events::InputEvent;
use crate::kernel::mm::map::mapper::{self as page, Prot};
use crate::kernel::mm::phys::frame;
use crate::kernel::mm::virt::vmarena;
use crate::kernel::sched::proc;
use micros_abi::types::FbInfo;
use crate::platform::limine::hhdm::HHDM_REQ;
use alloc::{boxed::Box, string::String, vec, vec::Vec};
use core::fmt::Write;
use font8x8::UnicodeFonts;
use x86_64::registers::model_specific::Msr;
use x86_64::{PhysAddr, VirtAddr};

unsafe fn ptr_of_slice(bx: &Box<[u8]>) -> usize {
    bx.as_ptr() as usize
}
use crate::kernel::fs::vfs;

#[inline(always)]
fn assert_eq_u64(tag: &str, a: u64, b: u64) {
    if a != b {
        panic!("[TEST-FAIL] {}: {:#x} != {:#x}", tag, a, b);
    }
}
#[inline(always)]
fn assert_true(tag: &str, ok: bool) {
    if !ok {
        panic!("[TEST-FAIL] {}", tag);
    }
}

fn test_frames() {
    sprintln!("[TEST] frames");
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
    sprintln!("[TEST] frames OK");
}

fn test_paging() {
    sprintln!("[TEST] paging");

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

    sprintln!("[TEST] paging OK");
}

fn test_vmarena() {
    sprintln!("[TEST] vmarena");

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
    sprintln!("[TEST] vmarena OK");
}

fn test_heap() {
    sprintln!("[TEST] heap");

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

    sprintln!("[TEST] heap OK");
}

extern "C" fn hog(_: *mut u8) -> ! {
    x86_64::instructions::interrupts::enable();

    let mut n: u64 = 0;

    loop {
        n = n.wrapping_add(1);

        unsafe {
            core::ptr::read_volatile(&n);
        }

        if n & 0xFF_FFFF == 0 {
            crate::sprintln!("[hog] still running… n={}", n);
        }
    }
}

extern "C" fn idle(_: *mut u8) -> ! {
    x86_64::instructions::interrupts::enable();
    loop {
        x86_64::instructions::hlt();
    }
}

extern "C" fn ping(_: *mut u8) -> ! {
    x86_64::instructions::interrupts::enable();
    loop {
        sprintln!("[ping]");
        crate::kernel::sched::task::sleep_ms(200);
    }
}

extern "C" fn pong(_: *mut u8) -> ! {
    x86_64::instructions::interrupts::enable();
    loop {
        sprintln!("[pong]");
        crate::kernel::sched::task::sleep_ms(300);
    }
}

pub fn alloc_kstack_top(pages: usize) -> VirtAddr {
    assert!(pages >= 1);
    let total = pages + 1;
    let base = vmarena::alloc_n(total).expect("kstack vmarena alloc");

    if let Ok(pf) = page::unmap(base) {
        frame::free(pf);
    }
    base + ((total as u64) * 4096u64)
}

pub fn free_kstack_top(top: VirtAddr, pages: usize) {
    let total = pages + 1;
    let base = top - ((total as u64) * 4096u64);
    for i in 1..total {
        let va = base + ((i as u64) * 4096u64);
        if let Ok(pf) = page::unmap(va) {
            frame::free(pf);
        }
    }
    vmarena::free_n(base, total);
}

#[inline]
fn xorshift32(mut x: u32) -> u32 {
    if x == 0 {
        x = 0xdead_beef;
    }
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    x
}

fn fill_pattern(buf: &mut [u8], seed: u32, lba: u64) {
    let mut s = seed ^ (lba as u32).wrapping_mul(0x9E37_79B9);

    for chunk in buf.chunks_exact_mut(4) {
        s = xorshift32(s);
        chunk.copy_from_slice(&s.to_le_bytes());
    }

    let rem = buf.len() & 3;
    if rem != 0 {
        s = xorshift32(s);
        let src = s.to_le_bytes();

        let len = buf.len();
        let start = len - rem;
        buf[start..len].copy_from_slice(&src[..rem]);
    }
}

fn verify_pattern(buf: &[u8], seed: u32, lba: u64) -> bool {
    let mut tmp = vec![0u8; buf.len()];
    fill_pattern(&mut tmp, seed, lba);
    buf == &tmp[..]
}

fn test_one_roundtrip(lba: u64, sectors: usize, seed: u32) -> bool {
    let sector_size = 512;
    let bytes = sector_size * sectors;
    let mut w = vec![0u8; bytes];
    let mut r = vec![0u8; bytes];

    fill_pattern(&mut w, seed, lba);

    if !crate::kernel::drivers::virtio::blk::write_at(lba, &w) {
        crate::sprintln!("[blk-test] WRITE fail @LBA={} +{}s", lba, sectors);
        return false;
    }

    if !crate::kernel::drivers::virtio::blk::read_at(lba, &mut r) {
        crate::sprintln!("[blk-test] READ fail @LBA={} +{}s", lba, sectors);
        return false;
    }

    let ok = w == r && verify_pattern(&r, seed, lba);
    crate::sprintln!(
        "[blk-test] LBA={} +{}s {}",
        lba,
        sectors,
        if ok { "OK" } else { "MISMATCH" },
    );
    ok
}

pub fn run_blk_tests() {
    crate::sprintln!("[blk-test] start (safe window)");
    const SCRATCH_BASE: u64 = 262_144;
    const SCRATCH_SPAN: u64 = 32_768;

    assert!(test_one_roundtrip(SCRATCH_BASE + 0, 1, 0x1111_1111));
    assert!(test_one_roundtrip(SCRATCH_BASE + 512, 8, 0x2222_2222));
    assert!(test_one_roundtrip(SCRATCH_BASE + 1023, 3, 0x4444_4444));

    let mut seed = 0x5555_5555u32;
    for _ in 0..32 {
        seed = xorshift32(seed);
        let lba = SCRATCH_BASE + (seed as u64 % SCRATCH_SPAN);
        let nsec = 1 + (seed as usize % 16);
        assert!(test_one_roundtrip(lba, nsec, seed));
    }

    crate::sprintln!("[blk-test] all passed");
}

extern "C" fn user_test_main(_: *mut u8) -> ! {
    x86_64::instructions::interrupts::enable();
    loop {
        let pid = crate::kernel::sched::proc::current_pid();
        sprintln!("[user-test] hello from pid={:?}", pid);
        crate::kernel::sched::task::sleep_ms(1000);
    }
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    unsafe {
        x86_64::instructions::interrupts::disable();

        // GDT/TSS
        let rsp0_top = crate::arch::x86_64::descriptors::tss::bootstrap_rsp0_top();
        crate::arch::x86_64::descriptors::gdt::init(rsp0_top);
        sprintln!("[Serial] GDT + TSS Initialized!");

        // IDT
        crate::arch::x86_64::descriptors::idt::init(1);
        sprintln!("[Serial] IDT + syscalls initialized!");

        // PIC off
        crate::arch::x86_64::legacy::pic8259::disable_8259_pic();
        sprintln!("[Serial] Disabled 8259 PIC (Legacy controller)");

        // CMOS
        if let Ok(dt) = crate::arch::x86_64::time::rtc::read_cmos_datetime() {
            sprintln!("[RTC] {}", dt);
        }

        // CPUID
        let ci = cpuid::detect();
        sprintln!(
            "[CPU] vendor={} fam={:X} model={:X} step={}",
            core::str::from_utf8(&ci.vendor_str).unwrap_or("?"),
            ci.family,
            ci.model,
            ci.stepping
        );

        // Memory subsystems
        frame::init();
        sprintln!("[MEMORY] Frames Initialized!");
        page::init();
        sprintln!("[MEMORY] Paging Initialized!");

        const VM_ARENA_BASE: u64 = 0xFFFF_FF00_0000_0000;
        const VM_ARENA_SIZE: u64 = 256 * 1024 * 1024;
        vmarena::init(VirtAddr::new(VM_ARENA_BASE), VM_ARENA_SIZE);
        sprintln!(
            "[VMARENA] base={:#x} size={} MiB",
            VM_ARENA_BASE,
            VM_ARENA_SIZE / (1024 * 1024)
        );
        page::init_scratch_va();

        let pages = (64 * 1024 * 1024) / 4096;
        crate::kernel::mm::heap::global_alloc::init(pages).expect("heap init");
        sprintln!("[MEMORY] Heap Initialized!");

        // Tests
        test_frames();
        test_paging();
        test_vmarena();
        test_heap();
        sprintln!("[TEST] ALL OK");

        crate::kernel::drivers::virtio::pci::init();

        let mut buf = [0u8; 512];
        let ok = crate::kernel::drivers::virtio::blk::read_at(0, &mut buf);
        sprintln!("[virtio-blk] LBA0 read {}", if ok { "OK" } else { "ERR" });

        vfs::vfs_selftest();

        let apic_base_msr = Msr::new(0x1B).read();
        let lapic_pa_bits = apic_base_msr & 0xFFFFF000;
        let lapic_pa = PhysAddr::new(lapic_pa_bits);
        let lapic_va = VirtAddr::new(0xFFFF_FF10_0000_0000);

        let pf = x86_64::structures::paging::PhysFrame::containing_address(lapic_pa);
        page::map_fixed(lapic_va, pf, Prot::MMIO).expect("map LAPIC UC");

        // APIC
        apic::init(0xEF, Some(lapic_va.as_mut_ptr()));
        sprintln!("[Serial] APIC Initialized!");


        // Create threads
        let idle_top = alloc_kstack_top(2);
        let idle_task = crate::kernel::sched::task::spawn_kthread(
            "idle",
            idle,
            core::ptr::null_mut(),
            idle_top,
        );
        crate::kernel::sched::task::init(idle_task);

        /*
        let t1 = alloc_kstack_top(2);
        let t2 = alloc_kstack_top(2);
        crate::kernel::sched::task::spawn_kthread("ping", ping, core::ptr::null_mut(), t1);
        crate::kernel::sched::task::spawn_kthread("pong", pong, core::ptr::null_mut(), t2);
        */

        let hog_top = alloc_kstack_top(2);
        // Spawn init (kernel thread) -> will exec into ring3 userland
        let init_top = alloc_kstack_top(4);
        crate::kernel::exec::init::spawn_init(init_top);

        // Init the APIC timer
        let lapic_hz = 1_000_000;
        let tick_hz = 1000;
        let _init_cnt = (lapic_hz / tick_hz as u64) as u32;
        apic::start_timer(0x20, 250_000, true);
        apic::set_tpr(0x00);

        apic::debug_dump();
        let _ = apic::probe_timer_countdown(500);

        x86_64::instructions::interrupts::enable();

        sprintln!("[handoff] entering scheduler…");
        crate::kernel::sched::task::schedule();
    }

    sprintln!("[handoff] WARN: schedule() returned to boot thread; parking.");
    loop {
        x86_64::instructions::hlt();
    }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    crate::sprintln!("[PANIC] {info}");
    loop {
        x86_64::instructions::hlt();
    }
}
