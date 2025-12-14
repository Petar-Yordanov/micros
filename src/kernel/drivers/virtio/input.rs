#![allow(dead_code)]

extern crate alloc;

use alloc::vec::Vec;
use core::mem::size_of;
use core::ptr::addr_of;

use spin::{Mutex, Once};
use x86_64::VirtAddr;

use crate::kernel::mm::map::mapper as page;
use crate::kernel::mm::virt::vmarena;

use super::pci::{self, VirtioPciCommonCfg, VirtioPciRegs, STATUS_DRIVER_OK};
use super::virtqueue::{VirtQueue, VIRTQ_DESC_F_WRITE};

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct RawInputEvent {
    etype: u16,
    code: u16,
    value: i32,
}

#[derive(Clone, Copy)]
pub enum InputMsg {
    Key {
        code: u16,
        pressed: bool,
        repeat: bool,
    },
    Rel {
        code: u16,
        value: i32,
    },
    Syn,
    Other {
        etype: u16,
        code: u16,
        value: i32,
    },
}

struct VirtioInput {
    _common: *mut VirtioPciCommonCfg,
    vq_evt: VirtQueue,
    ev_bufs: Vec<VirtAddr>,
}

unsafe impl Send for VirtioInput {}

impl VirtioInput {
    fn prime(&mut self, n: usize) {
        let mut posted = 0usize;
        let limit = core::cmp::min(n, self.vq_evt.qsz as usize);

        while posted < limit {
            let buf = vmarena::alloc().expect("input ev buf");
            let pa = page::translate(buf).unwrap();

            let d = self.vq_evt.alloc_desc();
            if self.ev_bufs.len() <= d as usize {
                self.ev_bufs.resize((d as usize) + 1, VirtAddr::new(0));
            }
            self.ev_bufs[d as usize] = buf;

            unsafe {
                let desc = &mut *self.vq_evt.desc.add(d as usize);
                desc.addr = pa.as_u64();
                desc.len = size_of::<RawInputEvent>() as u32;
                desc.flags = VIRTQ_DESC_F_WRITE;
                desc.next = 0;
            }

            self.vq_evt.push(d);
            posted += 1;
        }

        self.vq_evt.notify(0);
    }

    fn recycle(&mut self, id: u16) {
        self.vq_evt.push(id);
        self.vq_evt.notify(0);
    }

    fn poll_one(&mut self) -> Option<(u16, InputMsg)> {
        let u = self.vq_evt.pop_used()?;
        let id = (u.id & 0xFFFF) as u16;

        let buf_va = self
            .ev_bufs
            .get(id as usize)
            .copied()
            .unwrap_or(VirtAddr::new(0));

        if buf_va.as_u64() == 0 {
            return Some((
                id,
                InputMsg::Other {
                    etype: 0,
                    code: 0,
                    value: 0,
                },
            ));
        }

        unsafe {
            let ev = core::ptr::read(buf_va.as_ptr::<RawInputEvent>());
            let msg = match ev.etype {
                0x01 => InputMsg::Key {
                    code: ev.code,
                    pressed: ev.value != 0,
                    repeat: ev.value == 2,
                },
                0x02 => InputMsg::Rel {
                    code: ev.code,
                    value: ev.value,
                },
                0x00 => InputMsg::Syn,
                _ => InputMsg::Other {
                    etype: ev.etype,
                    code: ev.code,
                    value: ev.value,
                },
            };
            Some((id, msg))
        }
    }
}

static IN_DEVS: Once<Mutex<Vec<VirtioInput>>> = Once::new();

pub fn ensure_globals() {
    IN_DEVS.call_once(|| Mutex::new(Vec::new()));
}

pub fn count_devices() -> usize {
    IN_DEVS.get().map(|m| m.lock().len()).unwrap_or(0)
}

pub(crate) fn try_attach(regs: VirtioPciRegs) -> bool {
    unsafe {
        if !pci::negotiate_features(regs.common) {
            return false;
        }

        let vq_evt = match pci::setup_queue(regs.common, regs.notify, regs.notify_mul, 0) {
            Some(q) => q,
            None => return false,
        };

        (*regs.common).device_status |= STATUS_DRIVER_OK;

        let mut d = VirtioInput {
            _common: regs.common,
            vq_evt,
            ev_bufs: Vec::new(),
        };

        let qsz = addr_of!((*regs.common).queue_size).read_volatile();
        let pre = core::cmp::min(32usize, qsz as usize);
        d.prime(pre);

        if let Some(m) = IN_DEVS.get() {
            m.lock().push(d);
        }

        true
    }
}

pub fn poll_msg() -> Option<InputMsg> {
    let m = IN_DEVS.get()?;
    let mut devs = m.lock();

    for d in devs.iter_mut() {
        if let Some((id, msg)) = d.poll_one() {
            d.recycle(id);
            return Some(msg);
        }
    }

    None
}

pub fn drain_log(max_events: usize) {
    let mut left = max_events;
    while left > 0 {
        let Some(msg) = poll_msg() else { break };
        left -= 1;

        match msg {
            InputMsg::Key { code, pressed, .. } => {
                if pressed {
                    crate::sprintln!("[raw] key code={}", code);
                }
                crate::sprintln!(
                    "[input] key code={} {}",
                    code,
                    if pressed { "down" } else { "up" }
                );
            }

            InputMsg::Rel { code, value } => {
                crate::sprintln!("[input] rel code={} value={}", code, value);
            }

            InputMsg::Syn => crate::sprintln!("[input] syn"),

            InputMsg::Other { etype, code, value } => {
                crate::sprintln!(
                    "[input] other type={:#x} code={:#x} value={}",
                    etype,
                    code,
                    value
                );
            }
        }
    }
}
