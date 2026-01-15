#![allow(dead_code)]

use crate::kernel::fs::ext2::Ext2Err;
use crate::kernel::fs::fat32::FatErr;

#[derive(Debug)]
pub enum VfsError {
    NotMounted,
    BadPath,
    Io,
    Full,
    NotFound,
    Name,
    Unsupported,
    FsSpecific,
}

impl From<FatErr> for VfsError {
    fn from(e: FatErr) -> Self {
        match e {
            FatErr::Io => VfsError::Io,
            FatErr::BadBootSig | FatErr::NotFat32 | FatErr::BadBpb => VfsError::FsSpecific,
            FatErr::NotFound => VfsError::NotFound,
            FatErr::Name => VfsError::Name,
            FatErr::Full => VfsError::Full,
        }
    }
}

impl From<Ext2Err> for VfsError {
    fn from(e: Ext2Err) -> Self {
        match e {
            Ext2Err::Io => VfsError::Io,
            Ext2Err::BadMagic | Ext2Err::BadSuperblock => VfsError::FsSpecific,
            Ext2Err::NotFound => VfsError::NotFound,
            Ext2Err::Name => VfsError::Name,
            Ext2Err::Unsupported => VfsError::Unsupported,
            Ext2Err::Full => VfsError::Full,
        }
    }
}
