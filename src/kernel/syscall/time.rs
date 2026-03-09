use micros_abi::errno;
use micros_abi::types::TimeSpec;

use crate::arch::x86_64::time::rtc;
use crate::kernel::mm::aspace::user_copy::copy_to_user;
use crate::kernel::sched::task;

pub(super) fn sys_time_wall(out_ptr: u64) -> i64 {
    if out_ptr == 0 {
        return -errno::EFAULT;
    }

    let secs = match rtc::wall_clock_epoch_secs() {
        Some(v) => v,
        None => return -errno::ENOSYS,
    };

    let ts = TimeSpec {
        secs,
        nanos: 0,
        _pad: 0,
    };

    unsafe {
        if copy_to_user(
            out_ptr as *mut u8,
            &ts as *const _ as *const u8,
            core::mem::size_of::<TimeSpec>(),
        )
        .is_err()
        {
            return -errno::EFAULT;
        }
    }

    0
}

pub(super) fn sys_time_uptime(out_ptr: u64) -> i64 {
    if out_ptr == 0 {
        return -errno::EFAULT;
    }

    let ticks = task::jiffies();

    let ts = TimeSpec {
        secs: ticks / 1000,
        nanos: ((ticks % 1000) * 1_000_000) as u32,
        _pad: 0,
    };

    unsafe {
        if copy_to_user(
            out_ptr as *mut u8,
            &ts as *const _ as *const u8,
            core::mem::size_of::<TimeSpec>(),
        )
        .is_err()
        {
            return -errno::EFAULT;
        }
    }

    0
}
