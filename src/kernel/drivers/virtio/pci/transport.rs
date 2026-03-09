use core::sync::atomic::{fence, Ordering::SeqCst};

use super::caps::VirtioPciCommonCfg;
use crate::kernel::drivers::virtio::virtqueue::VirtQueue;
use crate::ksprintln;

const VIRTIO_F_VERSION_1: u64 = 1 << 32;

const STATUS_ACKNOWLEDGE: u8 = 1;
const STATUS_DRIVER: u8 = 2;
pub const STATUS_DRIVER_OK: u8 = 4;
const STATUS_FEATURES_OK: u8 = 8;
const STATUS_FAILED: u8 = 0x80;

fn reset_device(common: *mut VirtioPciCommonCfg) {
    unsafe {
        (*common).device_status = 0;
        fence(SeqCst);
    }
}

fn set_status(common: *mut VirtioPciCommonCfg, bits: u8) {
    unsafe {
        (*common).device_status |= bits;
        fence(SeqCst);
    }
}

fn features_ok(common: *mut VirtioPciCommonCfg) -> bool {
    unsafe {
        (*common).device_status |= STATUS_FEATURES_OK;
        fence(SeqCst);
        ((*common).device_status & STATUS_FEATURES_OK) != 0
    }
}

pub fn negotiate_features(common: *mut VirtioPciCommonCfg) -> bool {
    unsafe {
        reset_device(common);
        set_status(common, STATUS_ACKNOWLEDGE);
        set_status(common, STATUS_DRIVER);

        (*common).device_feature_select = 0;
        let f0 = (*common).device_feature as u64;
        (*common).device_feature_select = 1;
        let f1 = (*common).device_feature as u64;
        let feats = f0 | (f1 << 32);

        ksprintln!(
            "[virtio-pci][feat] device f0={:#x} f1={:#x} all={:#x}",
            f0,
            f1,
            feats
        );

        let mut accept: u64 = 0;
        if (feats & VIRTIO_F_VERSION_1) != 0 {
            accept |= VIRTIO_F_VERSION_1;
        }

        (*common).driver_feature_select = 0;
        (*common).driver_feature = (accept & 0xFFFF_FFFF) as u32;
        (*common).driver_feature_select = 1;
        (*common).driver_feature = (accept >> 32) as u32;

        if !features_ok(common) {
            let st = (*common).device_status;
            ksprintln!(
                "[virtio-pci][ERR] FEATURES_OK rejected; status={:#x} device_feat={:#x} accept={:#x}",
                st, feats, accept
            );
            (*common).device_status = STATUS_FAILED;
            return false;
        }

        ksprintln!(
            "[virtio-pci][feat] FEATURES_OK accepted; status={:#x}",
            (*common).device_status
        );
        true
    }
}

pub fn setup_queue(
    common: *mut VirtioPciCommonCfg,
    notify_base: *mut u8,
    notify_mul: u32,
    qsel: u16,
) -> Option<VirtQueue> {
    unsafe {
        (*common).queue_select = qsel;
        let max = (*common).queue_size;
        if max == 0 {
            return None;
        }
        let qsz = if max > 256 { 256 } else { max };
        Some(VirtQueue::new(common, qsel, qsz, notify_base, notify_mul))
    }
}

pub fn devcfg_read_le32(base: *const u8, off: usize) -> u32 {
    unsafe {
        let p = base.add(off) as *const u32;
        u32::from_le(core::ptr::read_volatile(p))
    }
}

pub fn devcfg_read_le64(base: *const u8, off: usize) -> u64 {
    unsafe {
        let p = base.add(off) as *const u64;
        u64::from_le(core::ptr::read_volatile(p))
    }
}
