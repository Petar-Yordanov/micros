#![allow(dead_code)]

extern crate alloc;

use crate::ksprintln;

use super::error::VfsError;
use super::mount::{mount_ext2, mount_fat32};
use super::ops::{vfs_list, vfs_mkdir_p, vfs_read, vfs_write, vfs_write_overwrite};

fn fat32_vfs_selftest() -> Result<(), VfsError> {
    ksprintln!("[vfs] FAT32 selftest");

    vfs_mkdir_p("/TestDir/Sub")?;

    let msg = b"Hello via VFS!\n";
    vfs_write_overwrite("/TestDir/Sub/Note.txt", msg)?;

    match vfs_read("/TestDir/Sub/Note.txt") {
        Ok(buf) if buf == msg => ksprintln!("[vfs] readback OK"),
        Ok(_) => return Err(VfsError::FsSpecific),
        Err(e) => return Err(e),
    }

    let ap = b"APPEND!";
    vfs_write("/TestDir/Sub/Note.txt", ap)?;
    match vfs_read("/TestDir/Sub/Note.txt") {
        Ok(buf) if buf == [msg.as_slice(), ap.as_slice()].concat() => ksprintln!("[vfs] append OK"),
        Ok(_) => return Err(VfsError::FsSpecific),
        Err(e) => return Err(e),
    }

    match vfs_read("/testdir/sub/note.txt") {
        Ok(buf) if buf == [msg.as_slice(), ap.as_slice()].concat() => {
            ksprintln!("[vfs] case-insensitive lookup OK")
        }
        Ok(_) => ksprintln!("[vfs][WARN] case-insensitive lookup got wrong content"),
        Err(e) => ksprintln!("[vfs][WARN] case-insensitive lookup failed: {:?}", e),
    }

    Ok(())
}

fn ext2_vfs_selftest() -> Result<(), VfsError> {
    ksprintln!("[vfs] ext2 selftest (mkdir/write/read/list)");

    vfs_mkdir_p("/TestDir/Sub")?;
    ksprintln!("[vfs] ext2 mkdir OK");

    let msg = b"Hello via VFS!\n";
    vfs_write_overwrite("/TestDir/Sub/Note.txt", msg)?;
    ksprintln!("[vfs] ext2 overwrite OK");

    match vfs_read("/TestDir/Sub/Note.txt") {
        Ok(buf) if buf == msg => ksprintln!("[vfs] ext2 readback OK"),
        Ok(_) => return Err(VfsError::FsSpecific),
        Err(e) => return Err(e),
    }

    let ap = b"APPEND!";
    ksprintln!("[vfs] ext2 append begin");
    vfs_write("/TestDir/Sub/Note.txt", ap)?;
    ksprintln!("[vfs] ext2 append returned");

    ksprintln!("[vfs] ext2 read-after-append begin");
    match vfs_read("/TestDir/Sub/Note.txt") {
        Ok(buf) if buf == [msg.as_slice(), ap.as_slice()].concat() => {
            ksprintln!("[vfs] ext2 append OK")
        }
        Ok(_) => return Err(VfsError::FsSpecific),
        Err(e) => return Err(e),
    }

    ksprintln!("[vfs] ext2 ci read begin");
    match vfs_read("/testdir/sub/note.txt") {
        Ok(buf) if buf == [msg.as_slice(), ap.as_slice()].concat() => {
            ksprintln!("[vfs] ext2 case-insensitive lookup OK")
        }
        Ok(_) => ksprintln!("[vfs][WARN] ext2 ci lookup got wrong content"),
        Err(e) => return Err(e),
    }

    Ok(())
}

pub fn vfs_selftest() {
    match mount_fat32("/", 0) {
        Ok(()) => {
            ksprintln!("[vfs] mounted / (FAT32)");
            if let Err(e) = fat32_vfs_selftest() {
                ksprintln!("[vfs][FAIL] FAT32 selftest: {:?}", e);
                return;
            }
        }
        Err(e_fat) => {
            ksprintln!("[vfs] FAT32 mount failed: {:?} (trying ext2)", e_fat);

            if let Err(e2) = mount_ext2("/", 0) {
                ksprintln!("[vfs][FAIL] ext2 mount failed: {:?}", e2);
                return;
            }
            ksprintln!("[vfs] mounted / (ext2)");
            if let Err(e) = ext2_vfs_selftest() {
                ksprintln!("[vfs][FAIL] ext2 selftest: {:?}", e);
                return;
            }
        }
    }

    match vfs_list("/") {
        Ok(list) => {
            ksprintln!("[vfs] / listing:");
            for n in list {
                ksprintln!(" - {}", n);
            }
        }
        Err(e) => ksprintln!("[vfs][WARN] list /: {:?}", e),
    }

    match vfs_list("/TestDir/Sub") {
        Ok(list) => {
            ksprintln!("[vfs] /TestDir/Sub listing:");
            for n in list {
                ksprintln!(" - {}", n);
            }
        }
        Err(_e) => {
            // quiet
        }
    }

    ksprintln!("[vfs] PASS");
}
