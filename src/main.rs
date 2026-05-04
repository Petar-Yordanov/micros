#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]

extern crate alloc;

mod arch;
mod kernel;
mod platform;

use crate::arch::x86_64::cpu::{cpuid, fpu};
pub use crate::arch::x86_64::serial;
use crate::arch::x86_64::time::apic;
use crate::kernel::boot::idle;
use crate::kernel::bootlog;
use crate::kernel::bootlog::boot_progress_step;
use crate::kernel::fs::vfs;
use crate::kernel::mm::map::mapper::{self as page, Prot};
use crate::kernel::mm::phys::frame;
use crate::kernel::mm::virt::vmarena;
use crate::kernel::sched::kstack::alloc_kstack_top;
use crate::kernel::selftest::{test_frames, test_heap, test_paging, test_virtio_net, test_vmarena};
use x86_64::registers::model_specific::Msr;
use x86_64::{PhysAddr, VirtAddr};

#[no_mangle]
pub extern "C" fn _start() -> ! {
    unsafe {
        x86_64::instructions::interrupts::disable();
        bootlog::bootlog_set_progress_total(19);
        bootlog::boot_progress_step(0);
        bootlog::try_init();
        bootlog::bootlog_fb_enable();

        boot_progress_step(0);
        ksprintln!("[boot] MicrOS64 kernel starting");

        let rsp0_top = crate::arch::x86_64::descriptors::tss::bootstrap_rsp0_top();
        crate::arch::x86_64::descriptors::gdt::init(rsp0_top);
        boot_progress_step(1);
        ksprintln!("[boot] GDT + TSS initialized");

        crate::arch::x86_64::descriptors::idt::init(1);
        boot_progress_step(2);
        ksprintln!("[boot] IDT + syscalls initialized");

        crate::arch::x86_64::legacy::pic8259::disable_8259_pic();
        boot_progress_step(3);
        ksprintln!("[boot] disabled legacy PIC");

        if let Ok(dt) = crate::arch::x86_64::time::rtc::read_cmos_datetime() {
            ksprintln!("[rtc] {}", dt);
        }

        let ci = cpuid::detect();
        ksprintln!(
            "[cpu] vendor={} fam={:X} model={:X} step={}",
            core::str::from_utf8(&ci.vendor_str).unwrap_or("?"),
            ci.family,
            ci.model,
            ci.stepping
        );
        boot_progress_step(4);

        fpu::enable_fpu_sse();
        boot_progress_step(5);
        ksprintln!("[cpu] FPU/SSE enabled");

        frame::init();
        boot_progress_step(6);
        ksprintln!("[mm] frame allocator initialized");

        page::init();
        boot_progress_step(7);
        ksprintln!("[mm] paging initialized");

        crate::kernel::mm::aspace::address_space::init_kernel_aspace_snapshot();
        boot_progress_step(8);
        ksprintln!("[mm] kernel address space snapshotted");

        const VM_ARENA_BASE: u64 = 0xFFFF_FF00_0000_0000;
        const VM_ARENA_SIZE: u64 = 256 * 1024 * 1024;
        vmarena::init(VirtAddr::new(VM_ARENA_BASE), VM_ARENA_SIZE);
        boot_progress_step(9);
        ksprintln!(
            "[mm] vmarena base={:#x} size={} MiB",
            VM_ARENA_BASE,
            VM_ARENA_SIZE / (1024 * 1024)
        );

        page::init_scratch_va();

        let pages = (64 * 1024 * 1024) / 4096;
        crate::kernel::mm::heap::global_alloc::init(pages).expect("heap init");
        boot_progress_step(10);
        ksprintln!("[mm] heap initialized");

        test_frames();
        test_paging();
        test_vmarena();
        test_heap();
        boot_progress_step(11);
        ksprintln!("[test] all memory tests OK");

        crate::kernel::drivers::virtio::pci::init();
        boot_progress_step(12);
        ksprintln!("[virtio] pci scan complete");

        test_virtio_net();
        crate::kernel::net::init();
        boot_progress_step(13);
        ksprintln!("[test] virtio-net selftest complete");

        let mut buf = [0u8; 512];
        let ok = crate::kernel::drivers::virtio::blk::read_at(0, &mut buf);
        boot_progress_step(14);
        ksprintln!("[virtio] blk LBA0 read {}", if ok { "OK" } else { "ERR" });

        vfs::vfs_selftest();
        boot_progress_step(15);
        ksprintln!("[fs] vfs selftest complete");

        let apic_base_msr = Msr::new(0x1B).read();
        let lapic_pa_bits = apic_base_msr & 0xFFFFF000;
        let lapic_pa = PhysAddr::new(lapic_pa_bits);
        let lapic_va = VirtAddr::new(0xFFFF_FF10_0000_0000);

        let pf = x86_64::structures::paging::PhysFrame::containing_address(lapic_pa);
        page::map_fixed(lapic_va, pf, Prot::MMIO).expect("map LAPIC UC");

        apic::init(0xEF, Some(lapic_va.as_mut_ptr()));
        boot_progress_step(16);
        ksprintln!("[apic] local APIC initialized");

        let idle_top = alloc_kstack_top(2);
        let idle_task = crate::kernel::sched::task::spawn_kthread(
            "idle",
            idle,
            core::ptr::null_mut(),
            idle_top,
        );
        crate::kernel::sched::task::init(idle_task);
        boot_progress_step(17);
        ksprintln!("[sched] idle task ready");

        // disabled, spin-mutex net poller can deadlock under single-core preemption
        ksprintln!("[net] background poller disabled; syscalls poll RX synchronously");

        let init_top = alloc_kstack_top(4);
        crate::kernel::exec::init::spawn_init(init_top);
        boot_progress_step(18);
        ksprintln!("[init] /bin/init.elf spawned");

        let lapic_hz = 1_000_000;
        let tick_hz = 1000;
        let _init_cnt = (lapic_hz / tick_hz as u64) as u32;
        apic::start_timer(0x20, 250_000, true);
        apic::set_tpr(0x00);

        apic::debug_dump();
        let _ = apic::probe_timer_countdown(500);

        boot_progress_step(19);
        x86_64::instructions::interrupts::enable();

        ksprintln!("[handoff] entering scheduler");
        bootlog::bootlog_fb_disable();

        crate::kernel::sched::task::schedule();
    }

    ksprintln!("[warn] schedule() returned to boot thread; parking");
    loop {
        x86_64::instructions::hlt();
    }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    ksprintln!("[err] PANIC {info}");
    loop {
        x86_64::instructions::hlt();
    }
}
