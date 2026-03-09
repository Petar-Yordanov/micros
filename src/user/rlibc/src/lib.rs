#![no_std]
extern crate alloc;

pub mod errno;
pub mod syscall;

pub mod log;
pub mod fb;
pub mod input;
pub mod vfs;
pub mod sched;
pub mod proc;
pub mod exec;
pub mod heap;
pub mod time;
pub mod power;
pub mod chan;
pub mod shm;

#[global_allocator]
static GLOBAL_ALLOC: heap::BumpAlloc = heap::BumpAlloc;
