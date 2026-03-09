extern crate alloc;

use micros_abi::errno;
use micros_abi::types::AbiInputEvent;

use crate::kernel::drivers::virtio::input::InputMsg;
use crate::kernel::mm::aspace::user_copy::copy_to_user;

pub(super) fn sys_input_next_event(user_out_ptr: u64) -> i64 {
    if user_out_ptr == 0 {
        return -errno::EFAULT;
    }

    let Some(msg) = crate::kernel::drivers::virtio::input::poll_msg() else {
        return -errno::EAGAIN;
    };

    let (etype_u16, code_u16, value_i32): (u16, u16, i32) = match msg {
        InputMsg::Syn => (0x00, 0, 0),
        InputMsg::Key {
            code,
            pressed,
            repeat,
        } => {
            let v = if repeat {
                2
            } else if pressed {
                1
            } else {
                0
            };
            (0x01, code, v)
        }
        InputMsg::Rel { code, value } => (0x02, code, value),
        InputMsg::Other { etype, code, value } => (etype, code, value),
    };

    if core::mem::size_of::<AbiInputEvent>() < 8 {
        return -errno::EINVAL;
    }

    let mut out: AbiInputEvent = unsafe { core::mem::zeroed() };
    unsafe {
        let p = (&mut out as *mut AbiInputEvent) as *mut u8;

        let et = etype_u16.to_le_bytes();
        core::ptr::write(p.add(0), et[0]);
        core::ptr::write(p.add(1), et[1]);

        let co = code_u16.to_le_bytes();
        core::ptr::write(p.add(2), co[0]);
        core::ptr::write(p.add(3), co[1]);

        let va = value_i32.to_le_bytes();
        core::ptr::write(p.add(4), va[0]);
        core::ptr::write(p.add(5), va[1]);
        core::ptr::write(p.add(6), va[2]);
        core::ptr::write(p.add(7), va[3]);
    }

    unsafe {
        if copy_to_user(
            user_out_ptr as *mut u8,
            (&out as *const AbiInputEvent) as *const u8,
            core::mem::size_of::<AbiInputEvent>(),
        )
        .is_err()
        {
            return -errno::EFAULT;
        }
    }

    0
}
