#![allow(dead_code)]

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

use super::error::VfsError;
use super::mount::{mounts_mut, resolve};

pub fn vfs_read(path: &str) -> Result<Vec<u8>, VfsError> {
    let (fs, rel) = {
        let tbl = mounts_mut();
        resolve(path, &tbl[..])?
    };
    fs.read(&rel)
}

pub fn vfs_write(path: &str, data: &[u8]) -> Result<(), VfsError> {
    vfs_write_opts(path, data, false)
}

pub fn vfs_write_overwrite(path: &str, data: &[u8]) -> Result<(), VfsError> {
    vfs_write_opts(path, data, true)
}

pub fn vfs_write_opts(path: &str, data: &[u8], overwrite: bool) -> Result<(), VfsError> {
    let tbl = mounts_mut();
    let (fs, rel) = resolve(path, &tbl)?;
    fs.write(&rel, data, overwrite)
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
