#![allow(dead_code)]

extern crate alloc;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use core::cell::UnsafeCell;
use core::cmp::Ordering;

use crate::kernel::fs::fat16::{Fat16, FatErr};
use crate::sprintln;

#[derive(Debug)]
pub enum VfsError {
    NotMounted,
    BadPath,
    Io,
    Full,
    NotFound,
    Name,
    FsSpecific,
}

impl From<FatErr> for VfsError {
    fn from(e: FatErr) -> Self {
        match e {
            FatErr::Io => VfsError::Io,
            FatErr::BadBootSig | FatErr::NotFat16 | FatErr::BadBpb => VfsError::FsSpecific,
            FatErr::NotFound => VfsError::NotFound,
            FatErr::Name => VfsError::Name,
            FatErr::Full => VfsError::Full,
        }
    }
}

pub trait FileSystem: Send + Sync {
    fn read(&self, path: &str) -> Result<Vec<u8>, VfsError>;
    fn write(&self, path: &str, data: &[u8]) -> Result<(), VfsError>;
    fn mkdir_p(&self, path: &str) -> Result<(), VfsError>;
    fn list(&self, path: &str) -> Result<Vec<String>, VfsError>;
}

impl FileSystem for Fat16 {
    fn read(&self, path: &str) -> Result<Vec<u8>, VfsError> {
        Ok(Fat16::read_file(self, path)?)
    }
    fn write(&self, path: &str, data: &[u8]) -> Result<(), VfsError> {
        Ok(Fat16::write_file(self, path, data)?)
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

            if let Err(e) = Fat16::mkdir(self, &acc) {
                match e {
                    FatErr::Name => return Err(VfsError::Name),
                    FatErr::Full => return Err(VfsError::Full),
                    FatErr::Io => return Err(VfsError::Io),
                    _ => return Err(VfsError::FsSpecific),
                }
            }
        }
        Ok(())
    }
    fn list(&self, path: &str) -> Result<Vec<String>, VfsError> {
        Ok(if path == "/" {
            Fat16::list_root(self)?
        } else {
            Fat16::list_dir(self, path)?
        })
    }
}

enum FsKind {
    Fat16(Fat16),
}

impl FsKind {
    fn as_fs(&self) -> &dyn FileSystem {
        match self {
            FsKind::Fat16(fs) => fs,
        }
    }
}

struct Mount {
    mp: String,
    fs: FsKind,
}

struct Global<T>(UnsafeCell<T>);
unsafe impl<T> Sync for Global<T> {}
static MOUNTS: Global<Vec<Mount>> = Global(UnsafeCell::new(Vec::new()));

#[inline(always)]
fn mounts_mut() -> &'static mut Vec<Mount> {
    unsafe { &mut *MOUNTS.0.get() }
}

fn normalize_path(p: &str) -> Result<String, VfsError> {
    if p.is_empty() {
        return Err(VfsError::BadPath);
    }

    let mut out = String::new();
    if !p.starts_with('/') {
        out.push('/');
    }
    out.push_str(p);

    let mut collapsed = String::new();
    let mut last_is_slash = false;
    for b in out.bytes() {
        let is_slash = b == b'/';
        if is_slash {
            if !last_is_slash {
                collapsed.push('/');
            }
        } else {
            collapsed.push(b as char);
        }
        last_is_slash = is_slash;
    }
    if collapsed.len() > 1 && collapsed.ends_with('/') {
        collapsed.pop();
    }
    Ok(collapsed)
}

fn normalize_mountpoint(mp: &str) -> Result<String, VfsError> {
    let mut s = normalize_path(mp)?;
    if s.len() > 1 && s.ends_with('/') {
        s.pop();
    }
    Ok(s)
}

fn strip_mount_prefix(path: &str, mp: &str) -> String {
    if mp == "/" {
        return path.to_string();
    }

    let tail = &path[mp.len()..];
    if tail.is_empty() {
        "/".to_string()
    } else if tail.starts_with('/') {
        tail.to_string()
    } else {
        let mut s = String::from("/");
        s.push_str(tail);
        s
    }
}

pub fn mount_fat16(mountpoint: &str, base_off_bytes: u64) -> Result<(), VfsError> {
    let mp = normalize_mountpoint(mountpoint)?;
    let fs = Fat16::mount(base_off_bytes).map_err(VfsError::from)?;
    let tbl = mounts_mut();

    if let Some(slot) = tbl.iter_mut().find(|m| m.mp == mp) {
        slot.fs = FsKind::Fat16(fs);
        sprintln!("[vfs] remounted FAT16 at {}", mp);
        return Ok(());
    }

    tbl.push(Mount {
        mp: mp.clone(),
        fs: FsKind::Fat16(fs),
    });

    tbl.sort_unstable_by(|a, b| {
        let la = a.mp.len();
        let lb = b.mp.len();
        lb.cmp(&la)
    });

    sprintln!("[vfs] mounted FAT16 at {}", mp);
    Ok(())
}

fn resolve<'a>(path: &str, mounts: &'a [Mount]) -> Result<(&'a dyn FileSystem, String), VfsError> {
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

pub fn vfs_read(path: &str) -> Result<Vec<u8>, VfsError> {
    let (fs, rel) = {
        let tbl = mounts_mut();
        resolve(path, &tbl[..])?
    };
    fs.read(&rel)
}

pub fn vfs_write(path: &str, data: &[u8]) -> Result<(), VfsError> {
    let tbl = mounts_mut();
    let (fs, rel) = resolve(path, &tbl)?;
    fs.write(&rel, data)
}

pub fn vfs_mkdir_p(path: &str) -> Result<(), VfsError> {
    let tbl = mounts_mut();
    let (fs, rel) = resolve(path, &tbl)?;
    fs.mkdir_p(&rel)
}

pub fn vfs_list(path: &str) -> Result<Vec<String>, VfsError> {
    let tbl = mounts_mut();
    let (fs, rel) = resolve(path, &tbl)?;
    fs.list(&rel)
}

pub fn vfs_selftest() {
    if let Err(e) = mount_fat16("/", 0) {
        sprintln!("[vfs][FAIL] mount: {:?}", e);
        return;
    }
    sprintln!("[vfs] mounted /");

    if let Err(e) = vfs_mkdir_p("/TESTDIR/SUB") {
        sprintln!("[vfs][FAIL] mkdir_p: {:?}", e);
        return;
    }

    let msg = b"Hello via VFS!\n";
    if let Err(e) = vfs_write("/TESTDIR/SUB/NOTE.TXT", msg) {
        sprintln!("[vfs][FAIL] write: {:?}", e);
        return;
    }

    match vfs_read("/TESTDIR/SUB/NOTE.TXT") {
        Ok(buf) if buf == msg => sprintln!("[vfs] readback OK"),
        Ok(_) => {
            sprintln!("[vfs][FAIL] content mismatch");
            return;
        }
        Err(e) => {
            sprintln!("[vfs][FAIL] read: {:?}", e);
            return;
        }
    }

    match vfs_list("/") {
        Ok(list) => {
            sprintln!("[vfs] / listing:");
            for n in list {
                sprintln!(" - {}", n);
            }
        }
        Err(e) => sprintln!("[vfs][WARN] list /: {:?}", e),
    }

    match vfs_list("/TESTDIR/SUB") {
        Ok(list) => {
            sprintln!("[vfs] /TESTDIR/SUB listing:");
            for n in list {
                sprintln!(" - {}", n);
            }
        }
        Err(e) => sprintln!("[vfs][WARN] list sub: {:?}", e),
    }

    sprintln!("[vfs] PASS");
}
