use core::ptr::read_volatile;

use x86_64::{PhysAddr, VirtAddr};

use crate::kernel::drivers::pci::cfg_io;
use crate::kernel::mm::map::mapper::{self as page, Prot};
use crate::ksprintln;

const PCI_VENDOR_VIRTIO: u16 = 0x1AF4;
const PCI_CAP_PTR: u8 = 0x34;

const PCI_BAR0: u8 = 0x10;
const NUM_BARS: usize = 6;

const PCI_CAP_ID_VENDOR: u8 = 0x09;

const VIRTIO_PCI_CAP_COMMON_CFG: u8 = 1;
const VIRTIO_PCI_CAP_NOTIFY_CFG: u8 = 2;
const VIRTIO_PCI_CAP_ISR_CFG: u8 = 3;
const VIRTIO_PCI_CAP_DEVICE_CFG: u8 = 4;

#[repr(C, packed)]
pub struct VirtioPciCommonCfg {
    pub device_feature_select: u32,
    pub device_feature: u32,
    pub driver_feature_select: u32,
    pub driver_feature: u32,
    pub msix_config: u16,
    pub num_queues: u16,
    pub device_status: u8,
    pub config_generation: u8,

    pub queue_select: u16,
    pub queue_size: u16,
    pub queue_msix_vector: u16,
    pub queue_enable: u16,
    pub queue_notify_off: u16,

    pub queue_desc: u64,
    pub queue_driver: u64,
    pub queue_device: u64,
}

#[derive(Clone, Copy)]
pub(crate) struct BarMapping {
    pa: PhysAddr,
    va: VirtAddr,
    len: u64,
    #[allow(dead_code)]
    bar_index: usize,
}

impl BarMapping {
    fn map_mmio(pa: u64, len: u64, va_hint: u64, bar_index: usize) -> Option<Self> {
        let mut off = 0u64;
        while off < len {
            let p = PhysAddr::new((pa & !0xFFF) + off);
            let v = VirtAddr::new(va_hint + off);

            let pf = x86_64::structures::paging::PhysFrame::containing_address(p);
            if page::translate(v).is_none() {
                page::map_fixed(v, pf, Prot::MMIO).ok()?;
            }

            off += 4096;
        }
        Some(Self {
            pa: PhysAddr::new(pa),
            va: VirtAddr::new(va_hint),
            len,
            bar_index,
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum VirtioDevKind {
    Blk,
    Input,
    Other,
}

pub(crate) struct VirtioPciRegs {
    pub(crate) bar_map: [Option<BarMapping>; NUM_BARS],
    pub common: *mut VirtioPciCommonCfg,
    pub notify: *mut u8,
    pub notify_mul: u32,
    #[allow(dead_code)]
    pub isr: *mut u8,
    pub device: *mut u8,
    pub device_len: u32,
    pub(crate) kind: VirtioDevKind,
}

impl VirtioPciRegs {
    fn new() -> Self {
        Self {
            bar_map: [None, None, None, None, None, None],
            common: core::ptr::null_mut(),
            notify: core::ptr::null_mut(),
            notify_mul: 0,
            isr: core::ptr::null_mut(),
            device: core::ptr::null_mut(),
            device_len: 0,
            kind: VirtioDevKind::Other,
        }
    }
}

fn map_all_bars(bus: u8, dev: u8, func: u8, regs: &mut VirtioPciRegs) {
    const MAP_LEN: u64 = 0x40_0000;

    let mut i = 0usize;
    while i < NUM_BARS {
        let off = PCI_BAR0 + (i as u8) * 4;
        let bar_lo = cfg_io::read_u32(bus, dev, func, off);
        if bar_lo == 0 {
            i += 1;
            continue;
        }

        if (bar_lo & 1) != 0 {
            i += 1;
            continue;
        }

        let bar_type = (bar_lo >> 1) & 0x3;
        let is_64 = bar_type == 0x2;

        let mut pa: u64 = (bar_lo & !0xFu32) as u64;
        if is_64 {
            let bar_hi = cfg_io::read_u32(bus, dev, func, off + 4) as u64;
            pa |= bar_hi << 32;
        }

        let fn_key = ((bus as u64) << 24) | ((dev as u64) << 16) | ((func as u64) << 8);
        let va_hint = 0xFFFF_FF30_0000_0000u64
            .wrapping_add(fn_key << 8)
            .wrapping_add((i as u64) * 0x0040_0000);

        if pa != 0 {
            if let Some(m) = BarMapping::map_mmio(pa, MAP_LEN, va_hint, i) {
                ksprintln!(
                    "[virtio-pci][map] {:02x}:{:02x}.{} BAR{} phys={:#x} va={:#x} len={:#x} {}",
                    bus,
                    dev,
                    func,
                    i,
                    m.pa.as_u64(),
                    m.va.as_u64(),
                    m.len,
                    if is_64 { "64-bit" } else { "32-bit" }
                );
                regs.bar_map[i] = Some(m);
            }
        }

        i += if is_64 { 2 } else { 1 };
    }
}

fn map_bar_direct(regs: &VirtioPciRegs, bar_idx: usize, off: u32, len: u32) -> Option<*mut u8> {
    if bar_idx >= NUM_BARS {
        return None;
    }
    let m = regs.bar_map[bar_idx]?;
    let end = (off as u64).saturating_add(len as u64);
    if end > m.len {
        ksprintln!(
            "[virtio-pci][WARN] cap exceeds mapped BAR window: bar{} off={:#x} len={:#x} > map_len={:#x}",
            bar_idx, off, len, m.len
        );
        return None;
    }
    Some((m.va.as_u64() + off as u64) as *mut u8)
}

pub(crate) fn parse_caps_for_device(bus: u8, dev: u8, func: u8) -> Option<VirtioPciRegs> {
    let vendor = cfg_io::read_u16(bus, dev, func, 0x00);
    if vendor != PCI_VENDOR_VIRTIO {
        return None;
    }

    let mut regs = VirtioPciRegs::new();
    map_all_bars(bus, dev, func, &mut regs);

    let mut ptr = cfg_io::read_u8(bus, dev, func, PCI_CAP_PTR);
    if ptr < 0x40 {
        return None;
    }

    let mut hops = 0;
    while ptr >= 0x40 && hops < 64 {
        hops += 1;

        let cap_id = cfg_io::read_u8(bus, dev, func, ptr + 0);
        let cap_nxt = cfg_io::read_u8(bus, dev, func, ptr + 1);
        let cap_len = cfg_io::read_u8(bus, dev, func, ptr + 2);

        if cap_id == PCI_CAP_ID_VENDOR && cap_len >= 16 {
            let cfg_type = cfg_io::read_u8(bus, dev, func, ptr + 3);
            let bar_idx = cfg_io::read_u8(bus, dev, func, ptr + 4) as usize;
            let _id = cfg_io::read_u8(bus, dev, func, ptr + 5);
            let offset = cfg_io::read_u32(bus, dev, func, ptr + 8);
            let length = cfg_io::read_u32(bus, dev, func, ptr + 12);

            let base_ptr = map_bar_direct(&regs, bar_idx, offset, length);
            if let Some(base) = base_ptr {
                match cfg_type {
                    VIRTIO_PCI_CAP_COMMON_CFG => {
                        regs.common = base as *mut VirtioPciCommonCfg;
                        ksprintln!(
                            "[virtio-pci][cap] COMMON bar={} off={:#x} len={:#x} -> {:p}",
                            bar_idx,
                            offset,
                            length,
                            regs.common
                        );
                    }
                    VIRTIO_PCI_CAP_NOTIFY_CFG => {
                        let mul = if cap_len >= 20 {
                            cfg_io::read_u32(bus, dev, func, ptr + 16)
                        } else {
                            2
                        };
                        regs.notify = base;
                        regs.notify_mul = mul;
                        ksprintln!(
                            "[virtio-pci][cap] NOTIFY bar={} off={:#x} len={:#x} mul={} -> {:p}",
                            bar_idx,
                            offset,
                            length,
                            mul,
                            regs.notify
                        );
                    }
                    VIRTIO_PCI_CAP_ISR_CFG => {
                        regs.isr = base;
                        ksprintln!(
                            "[virtio-pci][cap] ISR bar={} off={:#x} len={:#x} -> {:p}",
                            bar_idx,
                            offset,
                            length,
                            regs.isr
                        );
                    }
                    VIRTIO_PCI_CAP_DEVICE_CFG => {
                        regs.device = base;
                        regs.device_len = length;
                        ksprintln!(
                            "[virtio-pci][cap] DEVICE bar={} off={:#x} len={:#x} -> {:p}",
                            bar_idx,
                            offset,
                            length,
                            regs.device
                        );
                    }
                    _ => {
                        ksprintln!(
                            "[virtio-pci][cap] OTHER cfg_type={} bar={} off={:#x} len={:#x} -> {:p}",
                            cfg_type,
                            bar_idx,
                            offset,
                            length,
                            base
                        );
                    }
                }
            } else {
                ksprintln!(
                    "[virtio-pci][cap] SKIP cfg_type={} bar={} off={:#x} len={:#x} (out of range)",
                    cfg_type,
                    bar_idx,
                    offset,
                    length
                );
            }
        }

        if cap_nxt == 0 || cap_nxt == ptr {
            break;
        }
        ptr = cap_nxt;
    }

    let did = cfg_io::read_u16(bus, dev, func, 0x02);
    ksprintln!(
        "[virtio-pci] {:02x}:{:02x}.{} ven={:04x} dev={:04x}",
        bus,
        dev,
        func,
        PCI_VENDOR_VIRTIO,
        did
    );

    regs.kind = match did {
        0x1042 | 0x1001 => VirtioDevKind::Blk,
        0x1052 | 0x1048 | 0x1006 => VirtioDevKind::Input,
        _ => {
            if !regs.device.is_null() && regs.device_len >= 0x08 {
                let cap = unsafe { read_volatile(regs.device as *const u64) };
                if cap != 0 {
                    VirtioDevKind::Blk
                } else {
                    VirtioDevKind::Other
                }
            } else {
                VirtioDevKind::Other
            }
        }
    };

    if regs.common.is_null() || regs.notify.is_null() {
        ksprintln!(
            "[virtio-pci][ERR] missing required caps: common={:p} notify={:p}",
            regs.common,
            regs.notify
        );
        return None;
    }

    Some(regs)
}

pub(crate) fn enable_pci_function(bus: u8, dev: u8, func: u8) {
    let cmd = cfg_io::read_u16(bus, dev, func, 0x04);
    let new_cmd = cmd | 0x0002 | 0x0004;
    if new_cmd != cmd {
        cfg_io::write_u16(bus, dev, func, 0x04, new_cmd);
    }
    let chk = cfg_io::read_u16(bus, dev, func, 0x04);
    let mem = (chk & 0x2) != 0;
    let bm = (chk & 0x4) != 0;

    ksprintln!(
        "[pci] enable {:02x}:{:02x}.{} cmd={:#x} -> mem={} busmaster={}",
        bus,
        dev,
        func,
        chk,
        mem,
        bm
    );
}
