#![allow(dead_code)]

// TODO: Fix naming convention

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
    pub pathPtr: u64,
    pub pathLen: u64,
    pub argvPtr: u64,
    pub argc: u64,
    pub flags: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ProcInfo {
    pub pid: u32,
    pub state: u32,
    pub namePtr: u64,
    pub nameLen: u32,
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
    pub pathPtr: u64,
    pub pathLen: u64,
    pub off: u64,
    pub bufPtr: u64,
    pub bufLen: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct VfsWriteArgs {
    pub pathPtr: u64,
    pub pathLen: u64,
    pub off: u64,
    pub bufPtr: u64,
    pub bufLen: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct VfsListArgs {
    pub pathPtr: u64,
    pub pathLen: u64,
    pub outPtr: u64,
    pub outLen: u64,
}

#[repr(u32)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum VfsMountFs {
    Fat32 = 1,
    Ext2  = 2,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct VfsMountArgs {
    pub fs: u32,
    pub mountPtr: u64,
    pub mountLen: u64,
    pub baseOffBytes: u64,
}
