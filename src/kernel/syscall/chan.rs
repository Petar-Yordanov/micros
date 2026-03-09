extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};
use spin::Mutex;

use micros_abi::errno;
use micros_abi::types::{ChanCreateArgs, ChanRecvArgs, ChanSendArgs};

use crate::kernel::mm::aspace::user_copy::{copy_from_user, copy_to_user};

use super::util::copy_user_struct;

const MAX_MSG: usize = 1024;
const MAX_QUEUE_MSGS: usize = 64;

#[derive(Default)]
struct Chan {
    q: VecDeque<Vec<u8>>,
}

static NEXT_CHAN_ID: AtomicU64 = AtomicU64::new(1);
static CHANS: Mutex<BTreeMap<u64, Chan>> = Mutex::new(BTreeMap::new());

pub(super) fn sys_chan_create(args_ptr: u64) -> i64 {
    let _args: ChanCreateArgs = match copy_user_struct(args_ptr) {
        Ok(v) => v,
        Err(e) => return e,
    };

    let id = NEXT_CHAN_ID.fetch_add(1, Ordering::Relaxed);

    let mut m = CHANS.lock();
    m.insert(id, Chan::default());

    id as i64
}

pub(super) fn sys_chan_send(args_ptr: u64) -> i64 {
    let args: ChanSendArgs = match copy_user_struct(args_ptr) {
        Ok(v) => v,
        Err(e) => return e,
    };

    if args.chan_id == 0 {
        return -errno::EINVAL;
    }
    if args.data_ptr == 0 {
        return -errno::EFAULT;
    }

    let len = args.data_len as usize;
    if len == 0 {
        return 0;
    }
    if len > MAX_MSG {
        return -errno::EINVAL;
    }

    let mut buf = Vec::<u8>::with_capacity(len);
    unsafe { buf.set_len(len) };

    unsafe {
        if copy_from_user(buf.as_mut_ptr(), args.data_ptr as *const u8, len).is_err() {
            return -errno::EFAULT;
        }
    }

    let mut m = CHANS.lock();
    let Some(ch) = m.get_mut(&args.chan_id) else {
        return -errno::ESRCH;
    };

    if ch.q.len() >= MAX_QUEUE_MSGS {
        return -errno::EAGAIN;
    }

    ch.q.push_back(buf);
    len as i64
}

pub(super) fn sys_chan_recv(args_ptr: u64) -> i64 {
    let args: ChanRecvArgs = match copy_user_struct(args_ptr) {
        Ok(v) => v,
        Err(e) => return e,
    };

    if args.chan_id == 0 {
        return -errno::EINVAL;
    }
    if args.out_ptr == 0 {
        return -errno::EFAULT;
    }

    let cap = args.out_cap as usize;
    if cap == 0 {
        return 0;
    }

    let mut m = CHANS.lock();
    let Some(ch) = m.get_mut(&args.chan_id) else {
        return -errno::ESRCH;
    };

    let Some(front) = ch.q.front() else {
        return -errno::EAGAIN;
    };

    if front.len() > cap {
        return -errno::EINVAL;
    }

    let msg = ch.q.pop_front().unwrap();
    unsafe {
        if copy_to_user(args.out_ptr as *mut u8, msg.as_ptr(), msg.len()).is_err() {
            return -errno::EFAULT;
        }
    }

    msg.len() as i64
}
