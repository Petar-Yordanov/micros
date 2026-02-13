extern crate alloc;

use core::mem;

use micros_abi::errno;

use crate::kernel::mm::aspace::user_copy::copy_to_user;
use micros_abi::types::{
    AbiInputEvent, ABI_IN_KIND_KEY, ABI_IN_KIND_OTHER, ABI_IN_KIND_REL, ABI_IN_KIND_SYN,
};

pub(super) fn sys_input_next_event(user_out_ptr: u64) -> i64 {
    if user_out_ptr == 0 {
        return -errno::EFAULT;
    }

    let msg = match crate::kernel::drivers::virtio::input::poll_msg() {
        Some(m) => m,
        None => return -errno::EAGAIN,
    };

    crate::sprintln!("[syscall] SYS_INPUT_NEXT_EVENT got msg={:?}", msg);

    let ev: AbiInputEvent = match msg {
        crate::kernel::drivers::virtio::input::InputMsg::Key {
            code,
            pressed,
            repeat,
        } => AbiInputEvent {
            kind: ABI_IN_KIND_KEY,
            code,
            value: if repeat { 2 } else if pressed { 1 } else { 0 },
        },

        crate::kernel::drivers::virtio::input::InputMsg::Rel { code, value } => AbiInputEvent {
            kind: ABI_IN_KIND_REL,
            code,
            value,
        },

        crate::kernel::drivers::virtio::input::InputMsg::Syn => AbiInputEvent {
            kind: ABI_IN_KIND_SYN,
            code: 0,
            value: 0,
        },

        crate::kernel::drivers::virtio::input::InputMsg::Other { etype, code, value } => {
            let packed = ((etype & 0x00FF) << 8) | (code & 0x00FF);
            AbiInputEvent {
                kind: ABI_IN_KIND_OTHER,
                code: packed,
                value,
            }
        }
    };

    unsafe {
        if copy_to_user(
            user_out_ptr as *mut u8,
            &ev as *const _ as *const u8,
            mem::size_of::<AbiInputEvent>(),
        )
        .is_err()
        {
            return -errno::EFAULT;
        }
    }

    0
}
