#![allow(dead_code)]

//TODO: Fix values

// Basic GUI bring-up
pub const SYS_FB_INFO: u64 = 1;
pub const SYS_FB_MAP: u64 = 2;
pub const SYS_INPUT_NEXT_EVENT: u64 = 3;

// File/app infra
pub const SYS_VFS_READ: u64 = 10;
pub const SYS_VFS_WRITE: u64 = 11;
pub const SYS_VFS_LIST: u64 = 12;
pub const SYS_VFS_MKDIR: u64 = 13;
pub const SYS_VFS_MOUNT: u64 = 0x104;

// Process/sys
pub const SYS_EXIT: u64 = 20;
pub const SYS_YIELD: u64 = 21;
pub const SYS_GETPID: u64 = 22;
pub const SYS_PROC_SPAWN: u64 = 23;
pub const SYS_PROC_WAIT: u64 = 24;
pub const SYS_PROC_KILL: u64 = 25;
pub const SYS_PROC_LIST: u64 = 26;
pub const SYS_SLEEP_MS: u64 = 27;
pub const SYS_PROC_INFO: u64 = 28;

// RTC
pub const SYS_TIME_WALL: u64 = 29;
pub const SYS_TIME_UPTIME: u64 = 33;

// Debug logs
pub const SYS_LOG: u64 = 100;

// (TODO: later)
pub const SYS_CHAN_CREATE: u64 = 30;
pub const SYS_CHAN_SEND: u64 = 31;
pub const SYS_CHAN_RECV: u64 = 32;

pub const SYS_SHM_CREATE: u64 = 40;
pub const SYS_SHM_MAP: u64 = 41;

pub const SYS_EXEC: u64 = 42;

// Power
pub const SYS_POWER: u64 = 34;
