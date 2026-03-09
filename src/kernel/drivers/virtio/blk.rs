#![allow(dead_code)]

extern crate alloc;

use alloc::vec;

use crate::Prot;
use spin::{Mutex, Once};
use x86_64::PhysAddr;
use x86_64::VirtAddr;

use crate::kernel::mm::map::mapper as page;
use crate::kernel::mm::virt::vmarena;
use crate::ksprintln;

use super::pci::{self, VirtioPciCommonCfg, VirtioPciRegs, STATUS_DRIVER_OK};
use super::virtqueue::{VirtQueue, VIRTQ_DESC_F_NEXT, VIRTQ_DESC_F_WRITE};

#[repr(C, packed)]
struct VirtioBlkReqHdr {
    req_type: u32,
    _reserved: u32,
    sector: u64,
}

const VIRTIO_BLK_T_IN: u32 = 0;
const VIRTIO_BLK_T_OUT: u32 = 1;

const VIRTIO_BLK_F_BLK_SIZE: u64 = 1 << 6;

struct VirtioBlk {
    common: *mut VirtioPciCommonCfg,
    vq0: VirtQueue,
    #[allow(dead_code)]
    capacity_sectors: u64,
    #[allow(dead_code)]
    blk_size: u32,
}

unsafe impl Send for VirtioBlk {}

impl VirtioBlk {
    fn read_sectors512(&mut self, sector512: u64, buf: &mut [u8]) -> bool {
        self.xfer(sector512, buf.as_mut_ptr(), buf.len(), true)
    }
    fn write_sectors512(&mut self, sector512: u64, buf: &[u8]) -> bool {
        self.xfer(sector512, buf.as_ptr() as *mut u8, buf.len(), false)
    }

    fn xfer(&mut self, sector512: u64, data: *mut u8, len: usize, is_read: bool) -> bool {
        use core::sync::atomic::{fence, Ordering::SeqCst};

        unsafe {
            let hdr_va = vmarena::alloc().expect("blk hdr va");
            let hdr_pa = page::translate(hdr_va).unwrap();
            core::ptr::write(
                hdr_va.as_mut_ptr::<VirtioBlkReqHdr>(),
                VirtioBlkReqHdr {
                    req_type: if is_read {
                        VIRTIO_BLK_T_IN
                    } else {
                        VIRTIO_BLK_T_OUT
                    },
                    _reserved: 0,
                    sector: sector512,
                },
            );

            let status_va = vmarena::alloc().expect("blk status va");
            let status_pa = page::translate(status_va).unwrap();
            core::ptr::write(status_va.as_mut_ptr::<u8>(), 0x5a);

            let (dma_va_base, dma_pa_base, _dma_pages, dma_off, dma_needs_copy) =
                Self::dma_prepare_buffer(data, len);

            if dma_needs_copy && !is_read {
                core::ptr::copy_nonoverlapping(
                    data,
                    (dma_va_base.as_u64() + dma_off) as *mut u8,
                    len,
                );
            }

            let h: u16 = 0;
            let d: u16 = 1;
            let s: u16 = 2;

            {
                let desc = &mut *self.vq0.desc.add(h as usize);
                desc.addr = hdr_pa.as_u64();
                desc.len = core::mem::size_of::<VirtioBlkReqHdr>() as u32;
                desc.flags = VIRTQ_DESC_F_NEXT;
                desc.next = d;
            }

            {
                let desc = &mut *self.vq0.desc.add(d as usize);
                desc.addr = dma_pa_base.as_u64() + dma_off;
                desc.len = len as u32;
                desc.flags = (if is_read { VIRTQ_DESC_F_WRITE } else { 0 }) | VIRTQ_DESC_F_NEXT;
                desc.next = s;
            }

            {
                let desc = &mut *self.vq0.desc.add(s as usize);
                desc.addr = status_pa.as_u64();
                desc.len = 1;
                desc.flags = VIRTQ_DESC_F_WRITE;
                desc.next = 0;
            }

            fence(SeqCst);

            self.vq0.push(h);
            self.vq0.notify(0);

            let mut spins: u64 = 0;
            loop {
                if let Some(u) = self.vq0.pop_used() {
                    let st = core::ptr::read(status_va.as_ptr::<u8>());

                    if st != 0 {
                        ksprintln!(
                            "[virtio-blk][xfer] used.id={} len={} status={} (FAIL)",
                            u.id,
                            u.len,
                            st
                        );
                    }
                    return st == 0;
                }

                spins += 1;

                if (spins & 0x00FF_FFFF) == 0 {
                    let a = &*self.vq0.avail;
                    let u = &*self.vq0.used;
                    let avail_idx = core::ptr::read_volatile(&a.idx);
                    let used_idx = core::ptr::read_volatile(&u.idx);

                    let _isr =
                        core::ptr::read_volatile((self.common as *mut u8).add(0) as *const u8);

                    ksprintln!(
                        "[virtio-blk][xfer] waiting... spins={} avail.idx={} used.idx={} last_used={}",
                        spins, avail_idx, used_idx, self.vq0.last_used_idx
                    );
                }

                core::hint::spin_loop();
            }
        }
    }

    fn dma_prepare_buffer(data: *mut u8, len: usize) -> (VirtAddr, PhysAddr, usize, u64, bool) {
        use crate::kernel::mm::phys::frame;

        let start_va_u64 = data as u64;
        let end_va_u64 = start_va_u64 + (len as u64).saturating_sub(1);

        let start_page = start_va_u64 & !0xfffu64;
        let end_page = end_va_u64 & !0xfffu64;
        let off_in_page = start_va_u64 & 0xfff;

        let mut cur_va = start_page;
        let mut last_pa_page: Option<u64> = None;

        while cur_va <= end_page {
            let pa = match page::translate(VirtAddr::new(cur_va)) {
                Some(p) => p.as_u64() & !0xfff,
                None => {
                    last_pa_page = None;
                    break;
                }
            };

            if let Some(prev) = last_pa_page {
                if pa != prev + 4096 {
                    last_pa_page = None;
                    break;
                }
            }
            last_pa_page = Some(pa);
            cur_va += 4096;
        }

        if let Some(first_pa_page) = last_pa_page
            .map(|_| page::translate(VirtAddr::new(start_page)).unwrap().as_u64() & !0xfff)
        {
            let base_pa = PhysAddr::new(first_pa_page);
            return (VirtAddr::new(0), base_pa, 0, off_in_page, false);
        }

        let pages = ((off_in_page as usize + len) + 4095) / 4096;
        let dma_va_base = vmarena::alloc_n(pages).expect("blk dma vmarena");

        for i in 0..pages {
            let va = dma_va_base + (i as u64) * 4096;
            if let Ok(pf) = page::unmap(va) {
                frame::free(pf);
            }
        }

        'retry: loop {
            let mut frames = alloc::vec::Vec::with_capacity(pages);
            for _ in 0..pages {
                frames.push(frame::alloc().expect("blk dma frame alloc"));
            }

            let base = frames[0].start_address().as_u64();
            for i in 1..pages {
                let expect = base + (i as u64) * 4096;
                if frames[i].start_address().as_u64() != expect {
                    for pf in frames {
                        frame::free(pf);
                    }
                    continue 'retry;
                }
            }

            for (i, pf) in frames.iter().enumerate() {
                let va = dma_va_base + (i as u64) * 4096;
                page::map_fixed(va, *pf, Prot::RW).expect("blk dma map_fixed");
            }

            return (dma_va_base, PhysAddr::new(base), pages, off_in_page, true);
        }
    }

    fn read_at(&mut self, off: u64, buf: &mut [u8]) -> bool {
        let first512 = off / 512;
        let end_off = off + buf.len() as u64;
        let last512 = (end_off + 511) / 512;
        let nsec512 = (last512 - first512).max(1);

        if off % 512 == 0 && buf.len() % 512 == 0 {
            return self.read_sectors512(first512, buf);
        }

        let mut tmp = vec![0u8; (nsec512 as usize) * 512];
        if !self.read_sectors512(first512, &mut tmp) {
            return false;
        }
        let start = (off % 512) as usize;
        let end = start + buf.len();
        buf.copy_from_slice(&tmp[start..end]);
        true
    }

    fn write_at(&mut self, off: u64, buf: &[u8]) -> bool {
        let first512 = off / 512;
        let end_off = off + buf.len() as u64;
        let last512 = (end_off + 511) / 512;
        let nsec512 = (last512 - first512).max(1);

        if off % 512 == 0 && buf.len() % 512 == 0 {
            return self.write_sectors512(first512, buf);
        }

        let mut tmp = vec![0u8; (nsec512 as usize) * 512];
        if !self.read_sectors512(first512, &mut tmp) {
            return false;
        }
        let start = (off % 512) as usize;
        let end = start + buf.len();
        tmp[start..end].copy_from_slice(buf);
        self.write_sectors512(first512, &tmp)
    }
}

static BLK_DEV: Once<Mutex<Option<VirtioBlk>>> = Once::new();

pub fn ensure_globals() {
    BLK_DEV.call_once(|| Mutex::new(None));
}

pub(super) fn try_attach(regs: VirtioPciRegs) -> bool {
    unsafe {
        if !pci::negotiate_features(regs.common) {
            return false;
        }

        let vq0 = match pci::setup_queue(regs.common, regs.notify, regs.notify_mul, 0) {
            Some(q) => q,
            None => return false,
        };

        (*regs.common).device_feature_select = 0;
        let f0 = (*regs.common).device_feature as u64;
        (*regs.common).device_feature_select = 1;
        let f1 = (*regs.common).device_feature as u64;
        let dev_feats = f0 | (f1 << 32);

        let capacity_sectors = pci::devcfg_read_le64(regs.device, 0x00);

        let mut blk_size: u32 = 512;
        if (dev_feats & VIRTIO_BLK_F_BLK_SIZE) != 0 {
            let bs = pci::devcfg_read_le32(regs.device, 0x14);
            if bs != 0 {
                blk_size = bs;
            }
        }

        (*regs.common).device_status |= STATUS_DRIVER_OK;

        let bytes = capacity_sectors.saturating_mul(512);
        let mib = (bytes + (1 << 20) - 1) >> 20;
        ksprintln!(
            "[virtio-pci][blk] capacity={} sectors (~{} MiB), blk_size={} (bytes)",
            capacity_sectors,
            mib,
            blk_size
        );

        if let Some(m) = BLK_DEV.get() {
            *m.lock() = Some(VirtioBlk {
                common: regs.common,
                vq0,
                capacity_sectors,
                blk_size,
            });
        }
        true
    }
}

pub fn read_at(off: u64, buf: &mut [u8]) -> bool {
    if let Some(m) = BLK_DEV.get() {
        let mut guard = m.lock();
        guard.as_mut().map(|b| b.read_at(off, buf)).unwrap_or(false)
    } else {
        false
    }
}

pub fn write_at(off: u64, buf: &[u8]) -> bool {
    if let Some(m) = BLK_DEV.get() {
        let mut guard = m.lock();
        guard
            .as_mut()
            .map(|b| b.write_at(off, buf))
            .unwrap_or(false)
    } else {
        false
    }
}
