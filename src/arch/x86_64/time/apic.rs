#![allow(dead_code)]

use crate::ksprintln;
use core::ptr::{read_volatile, write_volatile};
use spin::Once;
use x86_64::registers::model_specific::Msr;

pub enum Backend {
    X2,
    Mmio,
}

struct ApicState {
    backend: Backend,
    mmio_base: usize,
}

static APIC: Once<ApicState> = Once::new();

#[inline(always)]
fn state() -> &'static ApicState {
    APIC.get().expect("APIC not initialized")
}

pub unsafe fn init(spurious_vec: u8, mmio_mapped_base: Option<*mut u8>) {
    APIC.call_once(|| {
        if x2apic_supported_by_cpu() {
            enable_x2apic();
            set_tpr_x2(0xFF);
            msr_wr(X2APIC_SVR, (1u64 << 8) | (spurious_vec as u64));
            msr_wr(X2APIC_LVT_TIMER, (1u64 << 16) | 0x20);
            ksprintln!("[APIC] using x2APIC backend");
            ApicState {
                backend: Backend::X2,
                mmio_base: 0,
            }
        } else {
            let base = mmio_mapped_base.expect(
                "APIC MMIO fallback requires a mapped LAPIC base VA (map phys 0xFEE00000 as UC)",
            );
            let mmio_base = base as usize;

            let mut apic_base = Msr::new(IA32_APIC_BASE).read();
            apic_base |= APIC_GLOBAL_ENABLE;
            apic_base &= !X2APIC_ENABLE;
            Msr::new(IA32_APIC_BASE).write(apic_base);

            mmio_w(mmio_base, REG_TPR, 0xFF);
            mmio_w(mmio_base, REG_SVR, (1 << 8) | (spurious_vec as u32));
            mmio_w(mmio_base, REG_LVT_TIMER, (1 << 16) | 0x20);

            ksprintln!(
                "[APIC] using xAPIC MMIO backend @ {:?}",
                mmio_base as *mut u32
            );
            ApicState {
                backend: Backend::Mmio,
                mmio_base,
            }
        }
    });
}

pub unsafe fn start_timer(vector: u8, init_count: u32, periodic: bool) {
    match state().backend {
        Backend::X2 => {
            msr_wr(X2APIC_DIV, 0b0011);
            let mode = if periodic { 0b01 } else { 0b00 };
            let lvt = (vector as u64) | ((mode as u64) << 17);
            msr_wr(X2APIC_LVT_TIMER, lvt);
            msr_wr(X2APIC_INIT_CNT, init_count as u64);
            ksprintln!(
                "[APIC] x2 start: DIV=/16 LVT=0x{:x} INIT_CNT=0x{:x}",
                lvt,
                init_count
            );
        }
        Backend::Mmio => {
            let base = state().mmio_base;
            mmio_w(base, REG_DIV, 0b0011);
            let mode = if periodic { 0b01 } else { 0b00 };
            let lvt = (vector as u32) | ((mode as u32) << 17);
            mmio_w(base, REG_LVT_TIMER, lvt);
            mmio_w(base, REG_TMRINIT, init_count);
            ksprintln!(
                "[APIC] mmio start: DIV=0x3 LVT_TIMER=0x{:x} INIT_CNT=0x{:x}",
                lvt,
                init_count
            );
        }
    }
}

pub unsafe fn eoi() {
    match state().backend {
        Backend::X2 => msr_wr(X2APIC_EOI, 0),
        Backend::Mmio => mmio_w(state().mmio_base, REG_EOI, 0),
    }
}

pub unsafe fn set_tpr(val: u8) {
    match state().backend {
        Backend::X2 => {
            set_tpr_x2(val);
            ksprintln!("[APIC] TPR <= 0x{:02x}", val);
        }
        Backend::Mmio => {
            mmio_w(state().mmio_base, REG_TPR, val as u32);
            ksprintln!("[APIC] TPR <= 0x{:02x}", val);
        }
    }
}

pub unsafe fn debug_dump() {
    match state().backend {
        Backend::X2 => {
            ksprintln!("[APIC] x2 dump: (use rdmsr if needed)");
        }
        Backend::Mmio => {
            let base = state().mmio_base;
            let svr = mmio_r(base, REG_SVR);
            let lvt = mmio_r(base, REG_LVT_TIMER);
            let tpr = mmio_r(base, REG_TPR);
            let div = mmio_r(base, REG_DIV);
            let init = mmio_r(base, REG_TMRINIT);
            let cur = mmio_r(base, REG_TMRCUR);
            ksprintln!(
                "[APIC] mmio dump: SVR=0x{:x} LVT_TIMER=0x{:x} TPR=0x{:x} DIV=0x{:x} INIT=0x{:x} CUR=0x{:x}",
                svr, lvt, tpr, div, init, cur
            );
        }
    }
}

pub unsafe fn probe_timer_countdown(spin: u32) -> (u32, u32) {
    let before = match state().backend {
        Backend::X2 => 0,
        Backend::Mmio => mmio_r(state().mmio_base, REG_TMRCUR),
    };
    for _ in 0..spin {
        core::hint::spin_loop();
    }
    let after = match state().backend {
        Backend::X2 => 0,
        Backend::Mmio => mmio_r(state().mmio_base, REG_TMRCUR),
    };
    ksprintln!(
        "[APIC] probe: CUR start=0x{:x} end=0x{:x} (ticking)",
        before,
        after
    );
    (before, after)
}

const IA32_APIC_BASE: u32 = 0x1B;
const APIC_GLOBAL_ENABLE: u64 = 1 << 11;
const X2APIC_ENABLE: u64 = 1 << 10;

const X2APIC_BASE: u32 = 0x800;
const X2APIC_TPR: u32 = 0x808;
const X2APIC_EOI: u32 = 0x80B;
const X2APIC_SVR: u32 = 0x80F;
const X2APIC_LVT_TIMER: u32 = 0x832;
const X2APIC_INIT_CNT: u32 = 0x838;
const X2APIC_DIV: u32 = 0x83E;

#[inline(always)]
fn x2apic_supported_by_cpu() -> bool {
    let r = core::arch::x86_64::__cpuid(1);
    (r.ecx & (1 << 21)) != 0
}

#[inline(always)]
unsafe fn enable_x2apic() {
    let mut v = Msr::new(IA32_APIC_BASE).read();
    v |= APIC_GLOBAL_ENABLE | X2APIC_ENABLE;
    Msr::new(IA32_APIC_BASE).write(v);
}

#[inline(always)]
unsafe fn set_tpr_x2(val: u8) {
    msr_wr(X2APIC_TPR, val as u64);
}

#[inline(always)]
unsafe fn msr_wr(off: u32, val: u64) {
    Msr::new(X2APIC_BASE + off).write(val)
}

const REG_TPR: usize = 0x080 >> 2;
const REG_EOI: usize = 0x0B0 >> 2;
const REG_SVR: usize = 0x0F0 >> 2;
const REG_LVT_TIMER: usize = 0x320 >> 2;
const REG_TMRINIT: usize = 0x380 >> 2;
const REG_TMRCUR: usize = 0x390 >> 2;
const REG_DIV: usize = 0x3E0 >> 2;

#[inline(always)]
unsafe fn mmio_w(base: usize, idx: usize, val: u32) {
    debug_assert!(base != 0);
    let base = base as *mut u32;
    write_volatile(base.add(idx), val);
    let _ = read_volatile(base.add(idx));
}

#[inline(always)]
unsafe fn mmio_r(base: usize, idx: usize) -> u32 {
    debug_assert!(base != 0);
    let base = base as *mut u32;
    read_volatile(base.add(idx))
}
