use crate::kernel::drivers::pci::cfg_io;
use crate::ksprintln;

use super::caps::{enable_pci_function, parse_caps_for_device, VirtioDevKind};

const PCI_VENDOR_VIRTIO: u16 = 0x1AF4;

pub fn init() {
    let mut got_blk = false;
    let mut got_net = false;

    super::super::blk::ensure_globals();
    super::super::input::ensure_globals();
    super::super::net::ensure_globals();

    for bus in 0u8..=255 {
        for dev in 0u8..32 {
            for func in 0u8..8 {
                let vendor = cfg_io::read_u16(bus, dev, func, 0x00);
                if vendor == 0xFFFF || vendor != PCI_VENDOR_VIRTIO {
                    continue;
                }

                enable_pci_function(bus, dev, func);

                if let Some(regs) = parse_caps_for_device(bus, dev, func) {
                    match regs.kind {
                        VirtioDevKind::Blk if !got_blk => {
                            if super::super::blk::try_attach(regs) {
                                ksprintln!("[virtio-pci] blk ready");
                                got_blk = true;
                            }
                        }
                        VirtioDevKind::Net if !got_net => {
                            if super::super::net::try_attach(regs) {
                                ksprintln!("[virtio-pci] net ready");
                                got_net = true;
                            }
                        }
                        VirtioDevKind::Input => {
                            if super::super::input::try_attach(regs) {
                                ksprintln!("[virtio-pci] input ready");
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    if !got_blk {
        ksprintln!("[virtio-pci] WARN: no blk found");
    }
    if !got_net {
        ksprintln!("[virtio-pci] WARN: no net found");
    }
    if super::super::input::count_devices() == 0 {
        ksprintln!("[virtio-pci] WARN: no input found");
    }
}
