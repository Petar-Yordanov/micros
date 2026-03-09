#![no_std]
extern crate alloc;

pub mod errno;
pub mod syscall;

pub mod chan;
pub mod exec;
pub mod fb;
pub mod heap;
pub mod input;
pub mod log;
pub mod power;
pub mod proc;
pub mod sched;
pub mod shm;
pub mod time;
pub mod vfs;

#[global_allocator]
static GLOBAL_ALLOC: heap::BumpAlloc = heap::BumpAlloc;
