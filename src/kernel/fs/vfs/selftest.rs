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
        Ok(buf) => {
            ksprintln!(
                "[vfs] ext2 first read len={} expected_len={}",
                buf.len(),
                msg.len()
            );
            for (i, b) in buf.iter().enumerate() {
                ksprintln!("[vfs] ext2 first read buf[{}]={:#x}", i, *b);
            }
            for (i, b) in msg.iter().enumerate() {
                ksprintln!("[vfs] ext2 first read exp[{}]={:#x}", i, *b);
            }

            if buf == msg {
                ksprintln!("[vfs] ext2 readback OK");
            } else {
                ksprintln!("[vfs][FAIL] ext2 first read content mismatch");
                return Err(VfsError::FsSpecific);
            }
        }
        Err(e) => return Err(e),
    }

    let ap = b"APPEND!";
    ksprintln!("[vfs] ext2 append begin");
    vfs_write("/TestDir/Sub/Note.txt", ap)?;
    ksprintln!("[vfs] ext2 append returned");

    let expected = [msg.as_slice(), ap.as_slice()].concat();
    ksprintln!(
        "[vfs] ext2 read-after-append begin expected_len={}",
        expected.len()
    );

    match vfs_read("/TestDir/Sub/Note.txt") {
        Ok(buf) => {
            ksprintln!(
                "[vfs] ext2 read-after-append got len={} expected_len={}",
                buf.len(),
                expected.len()
            );

            for (i, b) in buf.iter().enumerate() {
                ksprintln!("[vfs] ext2 append buf[{}]={:#x}", i, *b);
            }
            for (i, b) in expected.iter().enumerate() {
                ksprintln!("[vfs] ext2 append exp[{}]={:#x}", i, *b);
            }

            if buf == expected {
                ksprintln!("[vfs] ext2 append OK");
            } else {
                ksprintln!("[vfs][FAIL] ext2 append content mismatch");
                return Err(VfsError::FsSpecific);
            }
        }
        Err(e) => return Err(e),
    }

    ksprintln!("[vfs] ext2 ci lookup begin");
    match vfs_read("/testdir/sub/note.txt") {
        Ok(buf) => {
            ksprintln!(
                "[vfs] ext2 ci read len={} expected_len={}",
                buf.len(),
                expected.len()
            );

            for (i, b) in buf.iter().enumerate() {
                ksprintln!("[vfs] ext2 ci buf[{}]={:#x}", i, *b);
            }

            if buf == expected {
                ksprintln!("[vfs] ext2 case-insensitive lookup OK");
            } else {
                ksprintln!("[vfs][FAIL] ext2 ci content mismatch");
                return Err(VfsError::FsSpecific);
            }
        }
        Err(e) => {
            ksprintln!("[vfs][FAIL] ext2 ci lookup err: {:?}", e);
            return Err(e);
        }
    }

    ksprintln!("[vfs] ext2 selftest done");
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
