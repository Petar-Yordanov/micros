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

#[global_allocator]
static GLOBAL_ALLOC: heap::BumpAlloc = heap::BumpAlloc;
