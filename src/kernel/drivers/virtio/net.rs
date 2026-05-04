#![allow(dead_code)]

extern crate alloc;

use alloc::vec::Vec;
use core::cmp::min;
use core::mem::size_of;
use core::ptr::{copy_nonoverlapping, read_unaligned, read_volatile, write_bytes};
use core::sync::atomic::{fence, Ordering::SeqCst};

use spin::{Mutex, Once};
use x86_64::VirtAddr;

use crate::kernel::mm::map::mapper as page;
use crate::kernel::mm::virt::vmarena;
use crate::ksprintln;

use super::pci::{self, VirtioPciCommonCfg, VirtioPciRegs, STATUS_DRIVER_OK};
use super::virtqueue::{VirtQueue, VIRTQ_DESC_F_WRITE};

const VIRTIO_NET_F_MTU: u64 = 1 << 3;
const VIRTIO_NET_F_MAC: u64 = 1 << 5;
const VIRTIO_NET_F_STATUS: u64 = 1 << 16;

const VIRTIO_NET_S_LINK_UP: u16 = 1;

const RX_QUEUE_SEL: u16 = 0;
const TX_QUEUE_SEL: u16 = 1;

const RX_BUF_SIZE: usize = 4096;
const TX_BUF_SIZE: usize = 4096;

const TX_WAIT_SPIN_LIMIT: u64 = 0x2000_0000;

#[repr(C, packed)]
#[derive(Clone, Copy, Default)]
struct VirtioNetHdr {
    flags: u8,
    gso_type: u8,
    hdr_len: u16,
    gso_size: u16,
    csum_start: u16,
    csum_offset: u16,
    num_buffers: u16,
}

const NET_HDR_LEN: usize = size_of::<VirtioNetHdr>();
const MAX_FRAME_LEN: usize = TX_BUF_SIZE - NET_HDR_LEN;

struct VirtioNet {
    common: *mut VirtioPciCommonCfg,
    device: *mut u8,
    device_len: u32,
    features: u64,
    rxq: VirtQueue,
    txq: VirtQueue,
    rx_bufs: Vec<VirtAddr>,
    tx_buf: VirtAddr,
    mac: Option<[u8; 6]>,
    mtu: u16,
    rx_log_count: u32,
    tx_log_count: u32,
}

unsafe impl Send for VirtioNet {}

impl VirtioNet {
    fn cfg_read_u8(&self, off: usize) -> Option<u8> {
        if self.device.is_null() || off >= self.device_len as usize {
            return None;
        }

        unsafe { Some(read_volatile(self.device.add(off) as *const u8)) }
    }

    fn cfg_read_le16(&self, off: usize) -> Option<u16> {
        if self.device.is_null() || off + 1 >= self.device_len as usize {
            return None;
        }

        unsafe {
            let p = self.device.add(off) as *const u16;
            Some(u16::from_le(read_volatile(p)))
        }
    }

    fn link_up(&self) -> bool {
        if (self.features & VIRTIO_NET_F_STATUS) == 0 {
            return true;
        }

        self.cfg_read_le16(6)
            .map(|st| (st & VIRTIO_NET_S_LINK_UP) != 0)
            .unwrap_or(true)
    }

    fn prime_rx(&mut self) {
        let qsz = self.rxq.qsz as usize;

        for _ in 0..qsz {
            let buf = vmarena::alloc().expect("virtio-net rx buf");
            let pa = page::translate(buf).expect("virtio-net rx pa");

            let d = self.rxq.alloc_desc();

            if self.rx_bufs.len() <= d as usize {
                self.rx_bufs.resize((d as usize) + 1, VirtAddr::new(0));
            }

            self.rx_bufs[d as usize] = buf;

            unsafe {
                write_bytes(buf.as_mut_ptr::<u8>(), 0, RX_BUF_SIZE);

                let desc = &mut *self.rxq.desc.add(d as usize);
                desc.addr = pa.as_u64();
                desc.len = RX_BUF_SIZE as u32;
                desc.flags = VIRTQ_DESC_F_WRITE;
                desc.next = 0;
            }

            self.rxq.push(d);
        }

        fence(SeqCst);
        self.rxq.notify(RX_QUEUE_SEL);

        ksprintln!(
            "[virtio-net][rx] primed {} buffers hdr_len={} rx_free_descs={}",
            qsz,
            NET_HDR_LEN,
            self.rxq.free_count()
        );
    }

    fn recycle_rx(&mut self, id: u16) {
        if let Some(buf) = self.rx_bufs.get(id as usize).copied() {
            if buf.as_u64() != 0 {
                unsafe {
                    write_bytes(buf.as_mut_ptr::<u8>(), 0, NET_HDR_LEN);
                }
            }
        }

        self.rxq.push(id);
        self.rxq.notify(RX_QUEUE_SEL);
    }

    fn poll_rx_into(&mut self, out: &mut [u8]) -> Option<usize> {
        let used = self.rxq.pop_used()?;
        let id = (used.id & 0xFFFF) as u16;

        let buf_va = self
            .rx_bufs
            .get(id as usize)
            .copied()
            .unwrap_or(VirtAddr::new(0));

        if buf_va.as_u64() == 0 {
            ksprintln!("[virtio-net][rx] used unknown buffer id={}", id);
            self.recycle_rx(id);
            return Some(0);
        }

        let total_len = used.len as usize;

        if total_len < NET_HDR_LEN {
            ksprintln!(
                "[virtio-net][rx] short used buffer: id={} len={} hdr_len={}",
                id,
                total_len,
                NET_HDR_LEN
            );
            self.recycle_rx(id);
            return Some(0);
        }

        unsafe {
            let _hdr = read_unaligned(buf_va.as_ptr::<VirtioNetHdr>());

            let frame_len = total_len - NET_HDR_LEN;
            let copy_len = min(frame_len, out.len());

            if copy_len != 0 {
                copy_nonoverlapping(
                    (buf_va.as_u64() + NET_HDR_LEN as u64) as *const u8,
                    out.as_mut_ptr(),
                    copy_len,
                );
            }

            if self.rx_log_count < 96 {
                if copy_len >= 14 {
                    let ethertype = u16::from_be_bytes([out[12], out[13]]);

                    ksprintln!(
                        "[virtio-net][rx] id={} total_len={} frame_len={} copy_len={} dst={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x} src={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x} ethertype={:#06x}",
                        id,
                        total_len,
                        frame_len,
                        copy_len,
                        out[0],
                        out[1],
                        out[2],
                        out[3],
                        out[4],
                        out[5],
                        out[6],
                        out[7],
                        out[8],
                        out[9],
                        out[10],
                        out[11],
                        ethertype
                    );
                } else {
                    ksprintln!(
                        "[virtio-net][rx] id={} total_len={} frame_len={} copy_len={} short_eth",
                        id,
                        total_len,
                        frame_len,
                        copy_len
                    );
                }

                self.rx_log_count += 1;
            }

            if frame_len > out.len() {
                ksprintln!(
                    "[virtio-net][rx] frame truncated: frame_len={} out_len={}",
                    frame_len,
                    out.len()
                );
            }

            self.recycle_rx(id);
            Some(copy_len)
        }
    }

    fn send_frame(&mut self, frame: &[u8]) -> bool {
        if frame.is_empty() {
            return false;
        }

        if frame.len() > MAX_FRAME_LEN {
            ksprintln!(
                "[virtio-net][tx] frame too large: len={} max={}",
                frame.len(),
                MAX_FRAME_LEN
            );
            return false;
        }

        unsafe {
            write_bytes(self.tx_buf.as_mut_ptr::<u8>(), 0, TX_BUF_SIZE);

            copy_nonoverlapping(
                frame.as_ptr(),
                (self.tx_buf.as_u64() + NET_HDR_LEN as u64) as *mut u8,
                frame.len(),
            );

            let pa = page::translate(self.tx_buf).expect("virtio-net tx pa");
            let d = self.txq.alloc_desc();

            let desc = &mut *self.txq.desc.add(d as usize);
            desc.addr = pa.as_u64();
            desc.len = (NET_HDR_LEN + frame.len()) as u32;
            desc.flags = 0;
            desc.next = 0;

            if self.tx_log_count < 96 {
                if frame.len() >= 14 {
                    let ethertype = u16::from_be_bytes([frame[12], frame[13]]);

                    ksprintln!(
                        "[virtio-net][tx] id={} frame_len={} total_len={} hdr_len={} dst={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x} src={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x} ethertype={:#06x} tx_free_before={}",
                        d,
                        frame.len(),
                        NET_HDR_LEN + frame.len(),
                        NET_HDR_LEN,
                        frame[0],
                        frame[1],
                        frame[2],
                        frame[3],
                        frame[4],
                        frame[5],
                        frame[6],
                        frame[7],
                        frame[8],
                        frame[9],
                        frame[10],
                        frame[11],
                        ethertype,
                        self.txq.free_count()
                    );
                } else {
                    ksprintln!(
                        "[virtio-net][tx] id={} frame_len={} total_len={} hdr_len={} short_eth tx_free_before={}",
                        d,
                        frame.len(),
                        NET_HDR_LEN + frame.len(),
                        NET_HDR_LEN,
                        self.txq.free_count()
                    );
                }

                self.tx_log_count += 1;
            }

            fence(SeqCst);
            self.txq.push(d);
            self.txq.notify(TX_QUEUE_SEL);

            let mut spins: u64 = 0;

            loop {
                if let Some(u) = self.txq.pop_used() {
                    let used_id = (u.id & 0xFFFF) as u16;

                    if used_id != d {
                        ksprintln!(
                            "[virtio-net][tx] WARN: completion id mismatch: expected={} got={}",
                            d,
                            used_id
                        );
                    }

                    self.txq.free_desc(used_id);

                    if self.tx_log_count < 128 {
                        ksprintln!(
                            "[virtio-net][tx] complete id={} used_len={} tx_free_after={}",
                            used_id,
                            u.len,
                            self.txq.free_count()
                        );
                    }

                    return true;
                }

                spins = spins.wrapping_add(1);

                if (spins & 0x00FF_FFFF) == 0 {
                    let a = &*self.txq.avail;
                    let u = &*self.txq.used;
                    let avail_idx = core::ptr::read_volatile(&a.idx);
                    let used_idx = core::ptr::read_volatile(&u.idx);

                    ksprintln!(
                        "[virtio-net][tx] waiting... spins={} avail.idx={} used.idx={} last_used={} tx_free={}",
                        spins,
                        avail_idx,
                        used_idx,
                        self.txq.last_used_idx,
                        self.txq.free_count()
                    );
                }

                if spins >= TX_WAIT_SPIN_LIMIT {
                    ksprintln!(
                        "[virtio-net][tx] ERROR: timeout waiting for tx completion id={} frame_len={} tx_free={}",
                        d,
                        frame.len(),
                        self.txq.free_count()
                    );
                    return false;
                }

                core::hint::spin_loop();
            }
        }
    }
}

static NET_DEV: Once<Mutex<Option<VirtioNet>>> = Once::new();

pub fn ensure_globals() {
    NET_DEV.call_once(|| Mutex::new(None));
}

pub fn is_ready() -> bool {
    NET_DEV.get().map(|m| m.lock().is_some()).unwrap_or(false)
}

pub fn mac_addr() -> Option<[u8; 6]> {
    let m = NET_DEV.get()?;
    let guard = m.lock();
    guard.as_ref().and_then(|n| n.mac)
}

pub fn mtu() -> Option<u16> {
    let m = NET_DEV.get()?;
    let guard = m.lock();
    guard.as_ref().map(|n| n.mtu)
}

pub fn link_up() -> bool {
    if let Some(m) = NET_DEV.get() {
        let guard = m.lock();

        if let Some(n) = guard.as_ref() {
            return n.link_up();
        }
    }

    false
}

pub fn max_frame_len() -> usize {
    MAX_FRAME_LEN
}

pub fn send_frame(frame: &[u8]) -> bool {
    if let Some(m) = NET_DEV.get() {
        let mut guard = m.lock();

        return guard.as_mut().map(|n| n.send_frame(frame)).unwrap_or(false);
    }

    false
}

pub fn recv_frame(out: &mut [u8]) -> Option<usize> {
    let m = NET_DEV.get()?;
    let mut guard = m.lock();

    guard.as_mut()?.poll_rx_into(out)
}

pub(crate) fn try_attach(regs: VirtioPciRegs) -> bool {
    ensure_globals();

    unsafe {
        let offered = pci::read_device_features(regs.common);
        let wanted = offered & (VIRTIO_NET_F_MTU | VIRTIO_NET_F_MAC | VIRTIO_NET_F_STATUS);

        let accepted = match pci::negotiate_features_with(regs.common, wanted) {
            Some(f) => f,
            None => return false,
        };

        let rxq = match pci::setup_queue(regs.common, regs.notify, regs.notify_mul, RX_QUEUE_SEL) {
            Some(q) => q,
            None => {
                ksprintln!("[virtio-net] missing rx queue");
                return false;
            }
        };

        let txq = match pci::setup_queue(regs.common, regs.notify, regs.notify_mul, TX_QUEUE_SEL) {
            Some(q) => q,
            None => {
                ksprintln!("[virtio-net] missing tx queue");
                return false;
            }
        };

        let tx_buf = vmarena::alloc().expect("virtio-net tx buf");
        write_bytes(tx_buf.as_mut_ptr::<u8>(), 0, TX_BUF_SIZE);

        let mac =
            if (accepted & VIRTIO_NET_F_MAC) != 0 && !regs.device.is_null() && regs.device_len >= 6
            {
                let mut m = [0u8; 6];

                for (i, b) in m.iter_mut().enumerate() {
                    *b = read_volatile(regs.device.add(i) as *const u8);
                }

                Some(m)
            } else {
                None
            };

        let mtu = if (accepted & VIRTIO_NET_F_MTU) != 0
            && !regs.device.is_null()
            && regs.device_len >= 10
        {
            let p = regs.device.add(8) as *const u16;
            u16::from_le(read_volatile(p))
        } else {
            1500
        };

        let mut dev = VirtioNet {
            common: regs.common,
            device: regs.device,
            device_len: regs.device_len,
            features: accepted,
            rxq,
            txq,
            rx_bufs: Vec::new(),
            tx_buf,
            mac,
            mtu,
            rx_log_count: 0,
            tx_log_count: 0,
        };

        ksprintln!(
            "[virtio-net] features offered={:#x} wanted={:#x} accepted={:#x} net_hdr_len={} max_frame_len={}",
            offered,
            wanted,
            accepted,
            NET_HDR_LEN,
            MAX_FRAME_LEN
        );

        dev.prime_rx();

        (*regs.common).device_status |= STATUS_DRIVER_OK;

        if let Some(mac) = dev.mac {
            ksprintln!(
                "[virtio-net] ready: qsz_rx={} qsz_tx={} mtu={} mac={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x} link={}",
                dev.rxq.qsz,
                dev.txq.qsz,
                dev.mtu,
                mac[0],
                mac[1],
                mac[2],
                mac[3],
                mac[4],
                mac[5],
                dev.link_up()
            );
        } else {
            ksprintln!(
                "[virtio-net] ready: qsz_rx={} qsz_tx={} mtu={} mac=none link={}",
                dev.rxq.qsz,
                dev.txq.qsz,
                dev.mtu,
                dev.link_up()
            );
        }

        if let Some(m) = NET_DEV.get() {
            *m.lock() = Some(dev);
        }

        true
    }
}
