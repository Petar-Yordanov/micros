extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};
use spin::Mutex;

use micros_abi::errno;
use micros_abi::types::{ChanCreateArgs, ChanRecvArgs, ChanSendArgs, ChanNameArgs};

use crate::kernel::mm::aspace::user_copy::{copy_from_user, copy_to_user};

use super::util::{copy_user_struct, copy_user_str};

const MAX_MSG: usize = 1024;
const MAX_QUEUE_MSGS: usize = 64;

#[derive(Default)]
struct Chan {
    q: VecDeque<Vec<u8>>,
    owner: u64,
    waiting: Vec<u64>,
}

static NEXT_CHAN_ID: AtomicU64 = AtomicU64::new(1);
static CHANS: Mutex<BTreeMap<u64, Chan>> = Mutex::new(BTreeMap::new());
static SERVICES: Mutex<BTreeMap<alloc::string::String, (u64, u64)>> = Mutex::new(BTreeMap::new());

pub(super) fn sys_chan_publish(args_ptr:u64)->i64 {
    let args:ChanNameArgs=match copy_user_struct(args_ptr){Ok(v)=>v,Err(e)=>return e};
    let name=match copy_user_str(args.name_ptr,args.name_len){Ok(s)=>s,Err(e)=>return e};
    if name.is_empty() || name.len()>64 || args.chan_id==0 {return -errno::EINVAL}
    let caller=crate::kernel::sched::proc::current_pid().unwrap_or(0);
    let chans=CHANS.lock();
    let Some(ch)=chans.get(&args.chan_id) else {return -errno::ESRCH};
    if ch.owner!=caller {return -errno::EPERM}
    drop(chans);
    let mut s=SERVICES.lock();
    if let Some((_,old_pid))=s.get(&name) {
        let alive=crate::kernel::sched::proc::for_pid(*old_pid).map(|p|p.state != crate::kernel::sched::proc::ProcState::Zombie).unwrap_or(false);
        if alive {return -errno::EEXIST}
    }
    s.insert(name,(args.chan_id,caller));
    0
}

pub(super) fn sys_chan_find(args_ptr:u64)->i64 {
    let args:ChanNameArgs=match copy_user_struct(args_ptr){Ok(v)=>v,Err(e)=>return e};
    let name=match copy_user_str(args.name_ptr,args.name_len){Ok(s)=>s,Err(e)=>return e};
    let s=SERVICES.lock();
    let Some((id,pid))=s.get(&name) else {return -errno::ESRCH};
    let alive=crate::kernel::sched::proc::for_pid(*pid).map(|p|p.state != crate::kernel::sched::proc::ProcState::Zombie).unwrap_or(false);
    if !alive{return -errno::ESRCH}
    *id as i64
}

pub(super) fn sys_chan_create(args_ptr: u64) -> i64 {
    let _args: ChanCreateArgs = match copy_user_struct(args_ptr) {
        Ok(v) => v,
        Err(e) => return e,
    };

    let id = NEXT_CHAN_ID.fetch_add(1, Ordering::Relaxed);

    let mut m = CHANS.lock();
    m.insert(id, Chan {owner:crate::kernel::sched::proc::current_pid().unwrap_or(0), ..Chan::default() });

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
    let waiting=core::mem::take(&mut ch.waiting);
    drop(m);
    for tid in waiting {crate::kernel::sched::task::wake_tid(tid);}
    len as i64
}

pub(super) fn sys_chan_recv(args_ptr: u64) -> i64 {recv_impl(args_ptr,false,0)}

pub(super) fn sys_chan_recv_wait(args_ptr: u64,timeout_ms:u64) -> i64 {
    recv_impl(args_ptr,true,timeout_ms)
}

fn recv_impl(args_ptr:u64,block:bool,timeout_ms:u64)->i64 {
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
        if block {
            // Register the waiter before deferring scheduling so no sender
            // can be lost between the empty check and blocking.
            if let Some(tid)=crate::kernel::sched::task::current_tid() {
                if !ch.waiting.contains(&tid){ch.waiting.push(tid);}
                if timeout_ms==0 {crate::kernel::sched::task::block_current_user();}
                else {crate::kernel::sched::task::sleep_current_user(timeout_ms);}
            }
        }
        return -errno::EAGAIN;
    };

    if front.len() > cap {
        return -errno::EINVAL;
    }

    let msg = ch.q.pop_front().unwrap();
    if let Some(tid)=crate::kernel::sched::task::current_tid() {
        ch.waiting.retain(|id|*id!=tid);
    }
    unsafe {
        if copy_to_user(args.out_ptr as *mut u8, msg.as_ptr(), msg.len()).is_err() {
            return -errno::EFAULT;
        }
    }

    msg.len() as i64
}

/// Drop channels owned by a reaped process and its stale wait registrations.
/// Existing remote handles naturally start returning ESRCH.
pub(super) fn release_process(pid: u64, tid: u64) {
    let mut chans = CHANS.lock();
    chans.retain(|_, ch| ch.owner != pid);
    for ch in chans.values_mut() {
        ch.waiting.retain(|id| *id != tid);
    }
    drop(chans);
    SERVICES.lock().retain(|_, (_, owner)| *owner != pid);
}
