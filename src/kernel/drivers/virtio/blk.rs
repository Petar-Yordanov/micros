#![allow(dead_code)]

extern crate alloc;

use alloc::vec;

use core::mem::size_of;

use spin::{Mutex, Once};
use x86_64::VirtAddr;

use crate::kernel::mm::map::mapper as page;
use crate::kernel::mm::virt::vmarena;
use crate::sprintln;

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

            let h = self.vq0.alloc_desc();
            let d = self.vq0.alloc_desc();
            let s = self.vq0.alloc_desc();

            {
                let desc = &mut *self.vq0.desc.add(h as usize);
                desc.addr = hdr_pa.as_u64();
                desc.len = size_of::<VirtioBlkReqHdr>() as u32;
                desc.flags = VIRTQ_DESC_F_NEXT;
                desc.next = d;
            }
            {
                let pa = page::translate(VirtAddr::from_ptr(data)).expect("data VA must be mapped");
                let desc = &mut *self.vq0.desc.add(d as usize);
                desc.addr = pa.as_u64();
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

            self.vq0.push(h);
            self.vq0.notify(0);

            loop {
                if let Some(u) = self.vq0.pop_used() {
                    let st = core::ptr::read(status_va.as_ptr::<u8>());
                    vmarena::free(hdr_va);
                    vmarena::free(status_va);

                    if st != 0 {
                        sprintln!(
                            "[virtio-blk][xfer] used.id={} len={} status={} (FAIL)",
                            u.id,
                            u.len,
                            st
                        );
                    }
                    return st == 0;
                }
            }
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

        // read device features again (for blk size)
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

        // driver OK
        (*regs.common).device_status |= STATUS_DRIVER_OK;

        let bytes = capacity_sectors.saturating_mul(512);
        let mib = (bytes + (1 << 20) - 1) >> 20;
        sprintln!(
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
        m.lock()
            .as_mut()
            .map(|b| b.read_at(off, buf))
            .unwrap_or(false)
    } else {
        false
    }
}

pub fn write_at(off: u64, buf: &[u8]) -> bool {
    if let Some(m) = BLK_DEV.get() {
        m.lock()
            .as_mut()
            .map(|b| b.write_at(off, buf))
            .unwrap_or(false)
    } else {
        false
    }
}
