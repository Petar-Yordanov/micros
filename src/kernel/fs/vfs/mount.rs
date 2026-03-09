#![allow(dead_code)]

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

use core::cell::UnsafeCell;
use core::cmp::Ordering;

use crate::kernel::fs::ext2::Ext2;
use crate::kernel::fs::fat32::Fat32;
use crate::ksprintln;

use super::error::VfsError;
use super::path::{normalize_mountpoint, normalize_path, strip_mount_prefix};

pub trait FileSystem: Send + Sync {
    fn read(&self, path: &str) -> Result<Vec<u8>, VfsError>;
    fn write(&self, path: &str, data: &[u8], overwrite: bool) -> Result<(), VfsError>;
    fn mkdir_p(&self, path: &str) -> Result<(), VfsError>;
    fn list(&self, path: &str) -> Result<Vec<String>, VfsError>;
}

impl FileSystem for Fat32 {
    fn read(&self, path: &str) -> Result<Vec<u8>, VfsError> {
        Ok(Fat32::read_file(self, path)?)
    }

    fn write(&self, path: &str, data: &[u8], overwrite: bool) -> Result<(), VfsError> {
        Ok(Fat32::write_file(self, path, data, overwrite)?)
    }

    fn mkdir_p(&self, path: &str) -> Result<(), VfsError> {
        let p = normalize_path(path)?;
        if p == "/" {
            return Ok(());
        }

        let mut acc = String::from("/");
        for part in p.trim_start_matches('/').split('/') {
            if part.is_empty() {
                continue;
            }
            if acc.len() > 1 {
                acc.push('/');
            }
            acc.push_str(part);

            if let Err(e) = Fat32::mkdir(self, &acc) {
                return Err(VfsError::from(e));
            }
        }
        Ok(())
    }

    fn list(&self, path: &str) -> Result<Vec<String>, VfsError> {
        Ok(if path == "/" {
            Fat32::list_root(self)?
        } else {
            Fat32::list_dir(self, path)?
        })
    }
}

impl FileSystem for Ext2 {
    fn read(&self, path: &str) -> Result<Vec<u8>, VfsError> {
        Ok(Ext2::read_file(self, path)?)
    }

    fn write(&self, path: &str, data: &[u8], overwrite: bool) -> Result<(), VfsError> {
        Ok(Ext2::write_file(self, path, data, overwrite)?)
    }

    fn mkdir_p(&self, path: &str) -> Result<(), VfsError> {
        let p = normalize_path(path)?;
        if p == "/" {
            return Ok(());
        }

        let mut acc = String::from("/");
        for part in p.trim_start_matches('/').split('/') {
            if part.is_empty() {
                continue;
            }
            if acc.len() > 1 {
                acc.push('/');
            }
            acc.push_str(part);

            if let Err(e) = Ext2::mkdir(self, &acc) {
                return Err(VfsError::from(e));
            }
        }
        Ok(())
    }

    fn list(&self, path: &str) -> Result<Vec<String>, VfsError> {
        Ok(Ext2::list_dir(self, path)?)
    }
}

enum FsKind {
    Fat32(Fat32),
    Ext2(Ext2),
}

impl FsKind {
    fn as_fs(&self) -> &dyn FileSystem {
        match self {
            FsKind::Fat32(fs) => fs,
            FsKind::Ext2(fs) => fs,
        }
    }
}

pub(crate) struct Mount {
    mp: String,
    fs: FsKind,
}

struct Global<T>(UnsafeCell<T>);
unsafe impl<T> Sync for Global<T> {}
static MOUNTS: Global<Vec<Mount>> = Global(UnsafeCell::new(Vec::new()));

#[inline(always)]
pub(crate) fn mounts_mut() -> &'static mut Vec<Mount> {
    unsafe { &mut *MOUNTS.0.get() }
}

pub fn mount_fat32(mountpoint: &str, base_off_bytes: u64) -> Result<(), VfsError> {
    let mp = normalize_mountpoint(mountpoint)?;
    let fs = Fat32::mount(base_off_bytes).map_err(VfsError::from)?;
    let tbl = mounts_mut();

    if let Some(slot) = tbl.iter_mut().find(|m| m.mp == mp) {
        slot.fs = FsKind::Fat32(fs);
        ksprintln!("[vfs] remounted FAT32 at {}", mp);
        return Ok(());
    }

    tbl.push(Mount {
        mp: mp.clone(),
        fs: FsKind::Fat32(fs),
    });

    tbl.sort_unstable_by(|a, b| b.mp.len().cmp(&a.mp.len()));
    ksprintln!("[vfs] mounted FAT32 at {}", mp);
    Ok(())
}

pub fn mount_ext2(mountpoint: &str, base_off_bytes: u64) -> Result<(), VfsError> {
    let mp = normalize_mountpoint(mountpoint)?;
    let fs = Ext2::mount(base_off_bytes).map_err(VfsError::from)?;
    let tbl = mounts_mut();

    if let Some(slot) = tbl.iter_mut().find(|m| m.mp == mp) {
        slot.fs = FsKind::Ext2(fs);
        ksprintln!("[vfs] remounted ext2 at {}", mp);
        return Ok(());
    }

    tbl.push(Mount {
        mp: mp.clone(),
        fs: FsKind::Ext2(fs),
    });

    tbl.sort_unstable_by(|a, b| b.mp.len().cmp(&a.mp.len()));
    ksprintln!("[vfs] mounted ext2 at {}", mp);
    Ok(())
}

pub(crate) fn resolve<'a>(
    path: &str,
    mounts: &'a [Mount],
) -> Result<(&'a dyn FileSystem, String), VfsError> {
    let p = normalize_path(path)?;
    for m in mounts {
        let ok = match p.len().cmp(&m.mp.len()) {
            Ordering::Less => false,
            Ordering::Equal => p == m.mp,
            Ordering::Greater => {
                p.starts_with(&m.mp) && (m.mp == "/" || p.as_bytes()[m.mp.len()] == b'/')
            }
        };
        if ok {
            let rel = strip_mount_prefix(&p, &m.mp);
            return Ok((m.fs.as_fs(), rel));
        }
    }
    Err(VfsError::NotMounted)
}

pub fn mount_root_auto() -> Result<(), VfsError> {
    match mount_fat32("/", 0) {
        Ok(()) => {
            ksprintln!("[vfs] mounted / (FAT32)");
            Ok(())
        }
        Err(e_fat) => {
            ksprintln!("[vfs] FAT32 mount failed: {:?} (trying ext2)", e_fat);
            mount_ext2("/", 0)?;
            ksprintln!("[vfs] mounted / (ext2)");
            Ok(())
        }
    }
}
