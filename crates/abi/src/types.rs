#![allow(dead_code)]

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct FbInfo {
    pub addr: u64,
    pub width: u32,
    pub height: u32,
    pub pitch: u32,
    pub bpp: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct AbiInputEvent {
    pub kind: u16,
    pub code: u16,
    pub value: i32,
}

pub const ABI_IN_KIND_SYN: u16 = 0x00;
pub const ABI_IN_KIND_KEY: u16 = 0x01;
pub const ABI_IN_KIND_REL: u16 = 0x02;

pub const ABI_IN_KIND_OTHER: u16 = 0xFFFF;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ProcSpawnArgs {
    pub path_ptr: u64,
    pub path_len: u64,
    pub argv_ptr: u64,
    pub argc: u64,
    pub flags: u64,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcStateAbi {
    New = 0,
    Running = 1,
    Zombie = 2,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ProcListEntry {
    pub pid: u32,
    pub state: u32,
    pub name_len: u32,
    pub _pad: u32,
    pub name: [u8; 32],
}

impl Default for ProcListEntry {
    fn default() -> Self {
        Self {
            pid: 0,
            state: 0,
            name_len: 0,
            _pad: 0,
            name: [0; 32],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ProcInfo {
    pub pid: u32,
    pub state: u32,
    pub main_tid: u64,
    pub name_len: u32,
    pub _pad: u32,
    pub name: [u8; 32],
}

impl Default for ProcInfo {
    fn default() -> Self {
        Self {
            pid: 0,
            state: 0,
            main_tid: 0,
            name_len: 0,
            _pad: 0,
            name: [0; 32],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct TimeSpec {
    pub secs: u64,
    pub nanos: u32,
    pub _pad: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct VfsPath {
    pub ptr: u64,
    pub len: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct VfsReadArgs {
    pub path_ptr: u64,
    pub path_len: u64,
    pub off: u64,
    pub buf_ptr: u64,
    pub buf_len: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct VfsWriteArgs {
    pub path_ptr: u64,
    pub path_len: u64,
    pub off: u64,
    pub buf_ptr: u64,
    pub buf_len: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct VfsListArgs {
    pub path_ptr: u64,
    pub path_len: u64,
    pub out_ptr: u64,
    pub out_len: u64,
}

#[repr(u32)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum VfsMountFs {
    Fat32 = 1,
    Ext2 = 2,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct VfsMountArgs {
    pub fs: u32,
    pub mount_ptr: u64,
    pub mount_len: u64,
    pub base_off_bytes: u64,
}

// Channels (IPC)

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ChanCreateArgs {
    pub flags: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ChanSendArgs {
    pub chan_id: u64,
    pub data_ptr: u64,
    pub data_len: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ChanRecvArgs {
    pub chan_id: u64,
    pub out_ptr: u64,
    pub out_cap: u64,
}

// Shared memory

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ShmCreateArgs {
    pub size: u64,
    pub flags: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ShmMapArgs {
    pub shm_id: u64,
    pub desired_va: u64, // kernel chooses a deterministic VA
    pub flags: u64,
}
