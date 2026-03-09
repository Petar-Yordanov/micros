#![allow(dead_code)]

extern crate alloc;
use alloc::{string::String, vec, vec::Vec};

use core::cmp::min;

use crate::kernel::drivers::virtio::blk::{read_at, write_at};
use crate::ksprintln;

const SEC: usize = 512;

#[derive(Debug)]
pub enum Ext2Err {
    Io,
    BadSuperblock,
    BadMagic,
    Unsupported,
    NotFound,
    Name,
    Full,
}

#[inline]
fn rd16(bs: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([bs[off], bs[off + 1]])
}
#[inline]
fn rd32(bs: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([bs[off], bs[off + 1], bs[off + 2], bs[off + 3]])
}
#[inline]
fn wr16(bs: &mut [u8], off: usize, v: u16) {
    let b = v.to_le_bytes();
    bs[off] = b[0];
    bs[off + 1] = b[1];
}
#[inline]
fn wr32(bs: &mut [u8], off: usize, v: u32) {
    let b = v.to_le_bytes();
    bs[off] = b[0];
    bs[off + 1] = b[1];
    bs[off + 2] = b[2];
    bs[off + 3] = b[3];
}

#[inline]
fn is_dir(mode: u16) -> bool {
    (mode & 0xF000) == 0x4000
}
#[inline]
fn is_reg(mode: u16) -> bool {
    (mode & 0xF000) == 0x8000
}

#[inline]
fn eq_ci(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes()
        .zip(b.bytes())
        .all(|(x, y)| x.to_ascii_lowercase() == y.to_ascii_lowercase())
}

const EXT2_ROOT_INO: u32 = 2;

const INODE_OFF_MODE: usize = 0;
const INODE_OFF_SIZE: usize = 4;
const INODE_OFF_ATIME: usize = 8;
const INODE_OFF_CTIME: usize = 12;
const INODE_OFF_MTIME: usize = 16;
const INODE_OFF_DTIME: usize = 20;
const INODE_OFF_LINKS: usize = 26;
const INODE_OFF_BLOCKS: usize = 28;
const INODE_OFF_FLAGS: usize = 32;
const INODE_OFF_BLOCK: usize = 40;

const FT_UNKNOWN: u8 = 0;
const FT_REG: u8 = 1;
const FT_DIR: u8 = 2;

fn dirent_ideal_len(name_len: usize) -> usize {
    let base = 8 + name_len;
    (base + 3) & !3
}

pub struct Ext2 {
    base_off_bytes: u64,
    block_size: u32,
    blocks_per_group: u32,
    inodes_per_group: u32,
    inode_size: u16,
    first_inode: u32,
    bgdt_off: u64,
}

impl Ext2 {
    #[inline]
    fn abs(&self, off: u64) -> u64 {
        self.base_off_bytes + off
    }

    #[inline]
    fn blk_off(&self, block: u32) -> u64 {
        self.base_off_bytes + (block as u64) * (self.block_size as u64)
    }

    fn read_exact(&self, off: u64, out: &mut [u8]) -> Result<(), Ext2Err> {
        if !read_at(self.abs(off), out) {
            return Err(Ext2Err::Io);
        }
        Ok(())
    }

    fn write_exact(&self, off: u64, src: &[u8]) -> Result<(), Ext2Err> {
        if !write_at(self.abs(off), src) {
            return Err(Ext2Err::Io);
        }
        Ok(())
    }

    fn read_sb(&self) -> Result<[u8; 1024], Ext2Err> {
        let mut sb = [0u8; 1024];
        let ok0 = read_at(self.base_off_bytes + 1024, &mut sb[0..512]);
        let ok1 = read_at(self.base_off_bytes + 1024 + 512, &mut sb[512..1024]);
        if !ok0 || !ok1 {
            return Err(Ext2Err::Io);
        }
        Ok(sb)
    }

    fn write_sb(&self, sb: &[u8; 1024]) -> Result<(), Ext2Err> {
        if !write_at(self.base_off_bytes + 1024, &sb[0..512]) {
            return Err(Ext2Err::Io);
        }
        if !write_at(self.base_off_bytes + 1024 + 512, &sb[512..1024]) {
            return Err(Ext2Err::Io);
        }
        Ok(())
    }

    pub fn mount(base_off_bytes: u64) -> Result<Self, Ext2Err> {
        let mut sb = [0u8; 1024];
        let ok0 = read_at(base_off_bytes + 1024, &mut sb[0..512]);
        let ok1 = read_at(base_off_bytes + 1024 + 512, &mut sb[512..1024]);
        if !ok0 || !ok1 {
            return Err(Ext2Err::Io);
        }

        let magic = rd16(&sb, 56);
        if magic != 0xEF53 {
            return Err(Ext2Err::BadMagic);
        }

        let log_bs = rd32(&sb, 24);
        let block_size = 1024u32.checked_shl(log_bs).ok_or(Ext2Err::Unsupported)?;

        if block_size != 1024 && block_size != 2048 && block_size != 4096 {
            return Err(Ext2Err::Unsupported);
        }

        let blocks_per_group = rd32(&sb, 32);
        let inodes_per_group = rd32(&sb, 40);
        let first_inode = rd32(&sb, 84);

        let inode_size = {
            let v = rd16(&sb, 88);
            if v == 0 {
                128
            } else {
                v
            }
        };

        let bgdt_block = if block_size == 1024 { 2 } else { 1 };
        let bgdt_off = (bgdt_block as u64) * (block_size as u64);

        let fs = Self {
            base_off_bytes,
            block_size,
            blocks_per_group,
            inodes_per_group,
            inode_size,
            first_inode,
            bgdt_off,
        };

        ksprintln!(
            "[ext2] mount: bsz={} bpg={} ipg={} inode_sz={} first_inode={} bgdt_off={:#x}",
            fs.block_size,
            fs.blocks_per_group,
            fs.inodes_per_group,
            fs.inode_size,
            fs.first_inode,
            fs.bgdt_off
        );

        Ok(fs)
    }

    pub fn debug_probe_magic(&self) -> Result<u16, Ext2Err> {
        let sb = self.read_sb()?;
        Ok(rd16(&sb, 56))
    }

    fn read_block(&self, block: u32, out: &mut [u8]) -> Result<(), Ext2Err> {
        if out.len() != self.block_size as usize {
            ksprintln!(
                "[ext2][ERR] read_block size mismatch: block={} got={} want={}",
                block,
                out.len(),
                self.block_size
            );
            return Err(Ext2Err::BadSuperblock);
        }

        ksprintln!(
            "[ext2][io] read_block block={} off={:#x} len={}",
            block,
            (block as u64) * (self.block_size as u64),
            out.len()
        );

        self.read_exact((block as u64) * (self.block_size as u64), out)
    }

    fn write_block(&self, block: u32, src: &[u8]) -> Result<(), Ext2Err> {
        if src.len() != self.block_size as usize {
            ksprintln!(
                "[ext2][ERR] write_block size mismatch: block={} got={} want={}",
                block,
                src.len(),
                self.block_size
            );
            return Err(Ext2Err::BadSuperblock);
        }

        ksprintln!(
            "[ext2][io] write_block block={} off={:#x} len={}",
            block,
            (block as u64) * (self.block_size as u64),
            src.len()
        );

        self.write_exact((block as u64) * (self.block_size as u64), src)
    }

    fn read_group_desc(&self, group: u32) -> Result<[u8; 32], Ext2Err> {
        let off = self.bgdt_off + (group as u64) * 32;
        let mut gd = [0u8; 32];
        self.read_exact(off, &mut gd)?;
        Ok(gd)
    }

    fn write_group_desc(&self, group: u32, gd: &[u8; 32]) -> Result<(), Ext2Err> {
        let off = self.bgdt_off + (group as u64) * 32;
        self.write_exact(off, gd)?;
        Ok(())
    }

    fn inode_table_block(&self, group: u32) -> Result<u32, Ext2Err> {
        let gd = self.read_group_desc(group)?;
        let it = rd32(&gd, 8);
        if it == 0 {
            return Err(Ext2Err::BadSuperblock);
        }
        Ok(it)
    }

    fn group_block_bitmap(&self, group: u32) -> Result<u32, Ext2Err> {
        let gd = self.read_group_desc(group)?;
        let b = rd32(&gd, 0);
        if b == 0 {
            return Err(Ext2Err::BadSuperblock);
        }
        Ok(b)
    }

    fn group_inode_bitmap(&self, group: u32) -> Result<u32, Ext2Err> {
        let gd = self.read_group_desc(group)?;
        let b = rd32(&gd, 4);
        if b == 0 {
            return Err(Ext2Err::BadSuperblock);
        }
        Ok(b)
    }

    fn read_inode_raw(&self, ino: u32) -> Result<Vec<u8>, Ext2Err> {
        if ino == 0 {
            return Err(Ext2Err::NotFound);
        }
        let idx0 = ino - 1;
        let group = idx0 / self.inodes_per_group;
        let index = idx0 % self.inodes_per_group;

        let it_block = self.inode_table_block(group)?;
        let inode_sz = self.inode_size as u32;

        let byte_off = (index as u64) * (inode_sz as u64);
        let blk = it_block + ((byte_off as u32) / self.block_size);
        let off_in_blk = (byte_off as u32) % self.block_size;

        let mut buf = vec![0u8; inode_sz as usize];
        let bs = self.block_size as usize;

        let mut b0 = vec![0u8; bs];
        self.read_block(blk, &mut b0)?;
        let take0 = min(buf.len(), bs - (off_in_blk as usize));
        buf[..take0].copy_from_slice(&b0[(off_in_blk as usize)..(off_in_blk as usize) + take0]);

        if take0 < buf.len() {
            let mut b1 = vec![0u8; bs];
            self.read_block(blk + 1, &mut b1)?;
            let take1 = buf.len() - take0;
            buf[take0..].copy_from_slice(&b1[..take1]);
        }

        Ok(buf)
    }

    fn write_inode_raw(&self, ino: u32, raw: &[u8]) -> Result<(), Ext2Err> {
        if ino == 0 {
            return Err(Ext2Err::NotFound);
        }
        if raw.len() != self.inode_size as usize {
            return Err(Ext2Err::BadSuperblock);
        }

        let idx0 = ino - 1;
        let group = idx0 / self.inodes_per_group;
        let index = idx0 % self.inodes_per_group;

        let it_block = self.inode_table_block(group)?;
        let inode_sz = self.inode_size as u32;

        let byte_off = (index as u64) * (inode_sz as u64);
        let blk = it_block + ((byte_off as u32) / self.block_size);
        let off_in_blk = (byte_off as u32) % self.block_size;

        let bs = self.block_size as usize;

        let mut b0 = vec![0u8; bs];
        self.read_block(blk, &mut b0)?;
        let take0 = min(raw.len(), bs - (off_in_blk as usize));
        b0[(off_in_blk as usize)..(off_in_blk as usize) + take0].copy_from_slice(&raw[..take0]);
        self.write_block(blk, &b0)?;

        if take0 < raw.len() {
            let mut b1 = vec![0u8; bs];
            self.read_block(blk + 1, &mut b1)?;
            let take1 = raw.len() - take0;
            b1[..take1].copy_from_slice(&raw[take0..]);
            self.write_block(blk + 1, &b1)?;
        }

        Ok(())
    }

    fn inode_mode_size_blocks(&self, ino: u32) -> Result<(u16, u32, [u32; 15]), Ext2Err> {
        let raw = self.read_inode_raw(ino)?;
        if raw.len() < 100 {
            return Err(Ext2Err::BadSuperblock);
        }
        let mode = rd16(&raw, INODE_OFF_MODE);
        let size = rd32(&raw, INODE_OFF_SIZE);

        let mut blkptrs = [0u32; 15];
        for i in 0..15usize {
            blkptrs[i] = rd32(&raw, INODE_OFF_BLOCK + i * 4);
        }
        Ok((mode, size, blkptrs))
    }

    fn set_inode_u16(raw: &mut [u8], off: usize, v: u16) {
        wr16(raw, off, v);
    }
    fn set_inode_u32(raw: &mut [u8], off: usize, v: u32) {
        wr32(raw, off, v);
    }

    fn read_u32_block_entry(&self, block: u32, index: u32) -> Result<u32, Ext2Err> {
        if block == 0 {
            return Ok(0);
        }

        let per_block = self.block_size / 4;
        if index >= per_block {
            return Ok(0);
        }

        let bs = self.block_size as usize;
        let mut buf = vec![0u8; bs];
        self.read_block(block, &mut buf)?;
        Ok(rd32(&buf, (index as usize) * 4))
    }

    fn inode_data_block_no(
        &self,
        blkptrs: &[u32; 15],
        file_block_idx: u32,
    ) -> Result<u32, Ext2Err> {
        let per_block = self.block_size / 4;

        // direct
        if file_block_idx < 12 {
            return Ok(blkptrs[file_block_idx as usize]);
        }

        // single indirect
        let idx = file_block_idx - 12;
        if idx < per_block {
            return self.read_u32_block_entry(blkptrs[12], idx);
        }

        // double indirect
        let idx = idx - per_block;
        let doubly_cap = per_block
            .checked_mul(per_block)
            .ok_or(Ext2Err::Unsupported)?;

        if idx < doubly_cap {
            let outer = idx / per_block;
            let inner = idx % per_block;

            let l1 = self.read_u32_block_entry(blkptrs[13], outer)?;
            if l1 == 0 {
                return Ok(0);
            }
            return self.read_u32_block_entry(l1, inner);
        }

        Err(Ext2Err::Unsupported)
    }

    fn read_inode_data(&self, ino: u32, max_bytes: usize) -> Result<Vec<u8>, Ext2Err> {
        let (mode, size, blkptrs) = self.inode_mode_size_blocks(ino)?;
        ksprintln!(
            "[ext2][read_inode_data] ino={} mode={:#x} size={} max_bytes={}",
            ino,
            mode,
            size,
            max_bytes
        );

        if !(is_reg(mode) || is_dir(mode)) {
            ksprintln!("[ext2][read_inode_data] ino={} unsupported mode", ino);
            return Err(Ext2Err::Unsupported);
        }

        let want = min(size as usize, max_bytes);
        ksprintln!("[ext2][read_inode_data] ino={} want={}", ino, want);

        if want == 0 {
            ksprintln!("[ext2][read_inode_data] ino={} empty", ino);
            return Ok(Vec::new());
        }

        let bs = self.block_size as usize;
        let blocks_needed = (want + bs - 1) / bs;
        ksprintln!(
            "[ext2][read_inode_data] ino={} bs={} blocks_needed={}",
            ino,
            bs,
            blocks_needed
        );

        let mut out = Vec::with_capacity(want);

        for file_blk in 0..(blocks_needed as u32) {
            let disk_blk = self.inode_data_block_no(&blkptrs, file_blk)?;
            ksprintln!(
                "[ext2][read_inode_data] ino={} file_blk={} -> disk_blk={}",
                ino,
                file_blk,
                disk_blk
            );

            let mut buf = vec![0u8; bs];
            if disk_blk != 0 {
                self.read_block(disk_blk, &mut buf)?;
            } else {
                buf.fill(0);
            }

            let take = min(bs, want - out.len());
            ksprintln!(
                "[ext2][read_inode_data] ino={} file_blk={} take={} out_before={}",
                ino,
                file_blk,
                take,
                out.len()
            );

            out.extend_from_slice(&buf[..take]);

            ksprintln!(
                "[ext2][read_inode_data] ino={} file_blk={} out_after={}",
                ino,
                file_blk,
                out.len()
            );

            if out.len() >= want {
                break;
            }
        }

        out.truncate(want);
        ksprintln!(
            "[ext2][read_inode_data] ino={} done final_len={}",
            ino,
            out.len()
        );

        Ok(out)
    }

    fn split_path(p: &str) -> Vec<&str> {
        p.split('/').filter(|c| !c.is_empty()).collect()
    }

    fn lookup_in_dir_ci(&self, dir_ino: u32, name: &str) -> Result<u32, Ext2Err> {
        let (mode, _size, _blkptrs) = self.inode_mode_size_blocks(dir_ino)?;
        if !is_dir(mode) {
            return Err(Ext2Err::NotFound);
        }

        let raw = self.read_inode_data(dir_ino, 512 * 1024)?;
        let mut off = 0usize;

        while off + 8 <= raw.len() {
            let ino = rd32(&raw, off);
            let rec_len = rd16(&raw, off + 4) as usize;
            let name_len = raw[off + 6] as usize;

            if rec_len == 0 {
                break;
            }

            if ino != 0 && off + rec_len <= raw.len() && off + 8 + name_len <= raw.len() {
                let nm_bytes = &raw[off + 8..off + 8 + name_len];
                if let Ok(nm) = core::str::from_utf8(nm_bytes) {
                    if eq_ci(nm, name) {
                        return Ok(ino);
                    }
                }
            }

            off += rec_len;
        }

        Err(Ext2Err::NotFound)
    }

    fn resolve_inode_ci(&self, path: &str) -> Result<u32, Ext2Err> {
        if path.is_empty() {
            return Err(Ext2Err::Name);
        }
        if path == "/" {
            return Ok(EXT2_ROOT_INO);
        }

        let comps = Self::split_path(path);
        if comps.is_empty() {
            return Ok(EXT2_ROOT_INO);
        }

        let mut cur = EXT2_ROOT_INO;
        for c in comps {
            cur = self.lookup_in_dir_ci(cur, c)?;
        }
        Ok(cur)
    }

    fn resolve_parent_dir_ci<'a>(&self, path: &'a str) -> Result<(u32, &'a str), Ext2Err> {
        let comps = Self::split_path(path);
        if comps.is_empty() {
            return Err(Ext2Err::Name);
        }
        let (parent, leaf) = comps.split_at(comps.len() - 1);
        let leaf = leaf[0];
        let mut cur = EXT2_ROOT_INO;
        for c in parent {
            cur = self.lookup_in_dir_ci(cur, c)?;
        }
        Ok((cur, leaf))
    }

    pub fn list_dir(&self, path: &str) -> Result<Vec<String>, Ext2Err> {
        let ino = self.resolve_inode_ci(path)?;
        let (mode, _size, _blkptrs) = self.inode_mode_size_blocks(ino)?;
        if !is_dir(mode) {
            return Err(Ext2Err::NotFound);
        }

        let raw = self.read_inode_data(ino, 512 * 1024)?;
        let mut out = Vec::new();

        let mut off = 0usize;
        while off + 8 <= raw.len() {
            let child_ino = rd32(&raw, off);
            let rec_len = rd16(&raw, off + 4) as usize;
            let name_len = raw[off + 6] as usize;
            let file_type = raw[off + 7];

            if rec_len == 0 {
                break;
            }

            if child_ino != 0 && off + rec_len <= raw.len() && off + 8 + name_len <= raw.len() {
                let nm_bytes = &raw[off + 8..off + 8 + name_len];
                if let Ok(nm) = core::str::from_utf8(nm_bytes) {
                    let mut s = String::from(nm);

                    let is_d = if file_type == FT_DIR {
                        true
                    } else if file_type == FT_REG {
                        false
                    } else {
                        match self.inode_mode_size_blocks(child_ino) {
                            Ok((m, _, _)) => is_dir(m),
                            Err(_) => false,
                        }
                    };

                    if is_d {
                        s.push('/');
                    }
                    out.push(s);
                }
            }

            off += rec_len;
        }

        Ok(out)
    }

    pub fn read_file(&self, path: &str) -> Result<Vec<u8>, Ext2Err> {
        ksprintln!("[ext2][read_file] path={}", path);

        let ino = self.resolve_inode_ci(path)?;
        ksprintln!("[ext2][read_file] resolved ino={}", ino);

        let (mode, size, blkptrs) = self.inode_mode_size_blocks(ino)?;
        ksprintln!(
            "[ext2][read_file] ino={} mode={:#x} size={} ptrs=[{},{},{},{},{},{},{},{},{},{},{},{}]",
            ino,
            mode,
            size,
            blkptrs[0],
            blkptrs[1],
            blkptrs[2],
            blkptrs[3],
            blkptrs[4],
            blkptrs[5],
            blkptrs[6],
            blkptrs[7],
            blkptrs[8],
            blkptrs[9],
            blkptrs[10],
            blkptrs[11],
        );

        if !is_reg(mode) {
            ksprintln!("[ext2][read_file] ino={} not a regular file", ino);
            return Err(Ext2Err::NotFound);
        }

        let mut v = self.read_inode_data(ino, size as usize)?;
        ksprintln!(
            "[ext2][read_file] ino={} read_inode_data returned len={}",
            ino,
            v.len()
        );

        v.truncate(size as usize);
        ksprintln!(
            "[ext2][read_file] ino={} final truncated len={}",
            ino,
            v.len()
        );

        Ok(v)
    }

    fn sb_dec_free_inodes(&self, n: u32) -> Result<(), Ext2Err> {
        let mut sb = self.read_sb()?;
        let cur = rd32(&sb, 16);
        wr32(&mut sb, 16, cur.saturating_sub(n));
        self.write_sb(&sb)?;
        Ok(())
    }
    fn sb_dec_free_blocks(&self, n: u32) -> Result<(), Ext2Err> {
        let mut sb = self.read_sb()?;
        let cur = rd32(&sb, 12);
        wr32(&mut sb, 12, cur.saturating_sub(n));
        self.write_sb(&sb)?;
        Ok(())
    }
    fn sb_inc_free_blocks(&self, n: u32) -> Result<(), Ext2Err> {
        let mut sb = self.read_sb()?;
        let cur = rd32(&sb, 12);
        wr32(&mut sb, 12, cur.wrapping_add(n));
        self.write_sb(&sb)?;
        Ok(())
    }

    fn gd_dec_free_inodes(&self, group: u32, n: u16) -> Result<(), Ext2Err> {
        let mut gd = self.read_group_desc(group)?;
        let cur = rd16(&gd, 14);
        wr16(&mut gd, 14, cur.saturating_sub(n));
        self.write_group_desc(group, &gd)?;
        Ok(())
    }
    fn gd_dec_free_blocks(&self, group: u32, n: u16) -> Result<(), Ext2Err> {
        let mut gd = self.read_group_desc(group)?;
        let cur = rd16(&gd, 12);
        wr16(&mut gd, 12, cur.saturating_sub(n));
        self.write_group_desc(group, &gd)?;
        Ok(())
    }
    fn gd_inc_free_blocks(&self, group: u32, n: u16) -> Result<(), Ext2Err> {
        let mut gd = self.read_group_desc(group)?;
        let cur = rd16(&gd, 12);
        wr16(&mut gd, 12, cur.wrapping_add(n));
        self.write_group_desc(group, &gd)?;
        Ok(())
    }
    fn gd_inc_used_dirs(&self, group: u32, n: u16) -> Result<(), Ext2Err> {
        let mut gd = self.read_group_desc(group)?;
        let cur = rd16(&gd, 16);
        wr16(&mut gd, 16, cur.wrapping_add(n));
        self.write_group_desc(group, &gd)?;
        Ok(())
    }

    fn bitmap_find_and_set(
        bitset: &mut [u8],
        start_bit: usize,
        limit_bits: usize,
    ) -> Option<usize> {
        let mut b = start_bit;
        while b < limit_bits {
            let byte = b / 8;
            let bit = b % 8;
            let mask = 1u8 << bit;
            if (bitset[byte] & mask) == 0 {
                bitset[byte] |= mask;
                return Some(b);
            }
            b += 1;
        }
        None
    }

    fn alloc_inode_group0(&self, is_dir: bool) -> Result<u32, Ext2Err> {
        let group = 0u32;
        let ibm_blk = self.group_inode_bitmap(group)?;
        let bs = self.block_size as usize;
        let mut bm = vec![0u8; bs];
        self.read_block(ibm_blk, &mut bm)?;

        let start = self.first_inode.saturating_sub(1) as usize;
        let limit = self.inodes_per_group as usize;

        let bit = Self::bitmap_find_and_set(&mut bm, start, limit).ok_or(Ext2Err::Full)?;
        self.write_block(ibm_blk, &bm)?;

        self.sb_dec_free_inodes(1)?;
        self.gd_dec_free_inodes(group, 1)?;
        if is_dir {
            self.gd_inc_used_dirs(group, 1)?;
        }

        Ok((bit as u32) + 1)
    }

    fn alloc_block_group0(&self) -> Result<u32, Ext2Err> {
        let group = 0u32;
        let bbm_blk = self.group_block_bitmap(group)?;
        let bs = self.block_size as usize;
        let mut bm = vec![0u8; bs];
        self.read_block(bbm_blk, &mut bm)?;

        let start = 0usize;
        let limit = self.blocks_per_group as usize;

        let bit = Self::bitmap_find_and_set(&mut bm, start, limit).ok_or(Ext2Err::Full)?;
        self.write_block(bbm_blk, &bm)?;

        self.sb_dec_free_blocks(1)?;
        self.gd_dec_free_blocks(group, 1)?;

        Ok(bit as u32)
    }

    fn free_block_group0(&self, block: u32) -> Result<(), Ext2Err> {
        let group = 0u32;
        let bbm_blk = self.group_block_bitmap(group)?;
        let bs = self.block_size as usize;
        let mut bm = vec![0u8; bs];
        self.read_block(bbm_blk, &mut bm)?;

        let bit = block as usize;
        let byte = bit / 8;
        let b = bit % 8;
        if byte >= bm.len() {
            return Err(Ext2Err::BadSuperblock);
        }
        let mask = 1u8 << b;
        bm[byte] &= !mask;

        self.write_block(bbm_blk, &bm)?;
        self.sb_inc_free_blocks(1)?;
        self.gd_inc_free_blocks(group, 1)?;
        Ok(())
    }

    fn read_inode_mutable(&self, ino: u32) -> Result<Vec<u8>, Ext2Err> {
        self.read_inode_raw(ino)
    }

    fn inode_get_block_ptrs(raw: &[u8]) -> [u32; 15] {
        let mut blkptrs = [0u32; 15];
        for i in 0..15usize {
            blkptrs[i] = rd32(raw, INODE_OFF_BLOCK + i * 4);
        }
        blkptrs
    }

    fn inode_set_block_ptr(raw: &mut [u8], idx: usize, blk: u32) {
        wr32(raw, INODE_OFF_BLOCK + idx * 4, blk);
    }

    fn inode_get_links(raw: &[u8]) -> u16 {
        rd16(raw, INODE_OFF_LINKS)
    }

    fn inode_set_links(raw: &mut [u8], v: u16) {
        wr16(raw, INODE_OFF_LINKS, v);
    }

    fn inode_set_size(raw: &mut [u8], v: u32) {
        wr32(raw, INODE_OFF_SIZE, v);
    }

    fn inode_set_blocks_512(raw: &mut [u8], v: u32) {
        wr32(raw, INODE_OFF_BLOCKS, v);
    }

    fn inode_init_common(raw: &mut [u8], mode: u16, links: u16) {
        raw.fill(0);
        wr16(raw, INODE_OFF_MODE, mode);
        wr16(raw, 2, 0);
        wr32(raw, INODE_OFF_ATIME, 0);
        wr32(raw, INODE_OFF_CTIME, 0);
        wr32(raw, INODE_OFF_MTIME, 0);
        wr32(raw, INODE_OFF_DTIME, 0);
        wr16(raw, 24, 0);
        wr16(raw, INODE_OFF_LINKS, links);
        wr32(raw, INODE_OFF_FLAGS, 0);
    }

    fn add_dir_entry(
        &self,
        dir_ino: u32,
        child_ino: u32,
        name: &str,
        ftype: u8,
    ) -> Result<(), Ext2Err> {
        if name.is_empty() {
            return Err(Ext2Err::Name);
        }
        if name.as_bytes().iter().any(|&b| b == 0 || b == b'/') {
            return Err(Ext2Err::Name);
        }

        let bs = self.block_size as usize;

        let mut dir_raw = self.read_inode_mutable(dir_ino)?;
        let (dir_mode, dir_size, dir_ptrs) = {
            let mode = rd16(&dir_raw, INODE_OFF_MODE);
            let size = rd32(&dir_raw, INODE_OFF_SIZE);
            let ptrs = Self::inode_get_block_ptrs(&dir_raw);
            (mode, size, ptrs)
        };

        if !is_dir(dir_mode) {
            return Err(Ext2Err::NotFound);
        }

        if self.lookup_in_dir_ci(dir_ino, name).is_ok() {
            return Ok(());
        }

        let mut blocks_used = (dir_size as usize + bs - 1) / bs;
        if blocks_used == 0 {
            blocks_used = 1;
        }

        if blocks_used > 12 {
            return Err(Ext2Err::Unsupported);
        }

        let name_len = name.len();
        let need = dirent_ideal_len(name_len);

        for bi in 0..blocks_used {
            let blk = dir_ptrs[bi];
            if blk == 0 {
                continue;
            }
            let mut buf = vec![0u8; bs];
            self.read_block(blk, &mut buf)?;

            let mut off = 0usize;
            while off + 8 <= bs {
                let _ino = rd32(&buf, off);
                let rec_len = rd16(&buf, off + 4) as usize;
                let nm_len = buf[off + 6] as usize;

                if rec_len == 0 || off + rec_len > bs {
                    break;
                }

                let ideal = dirent_ideal_len(nm_len);
                if rec_len >= ideal + need {
                    wr16(&mut buf, off + 4, ideal as u16);

                    let new_off = off + ideal;
                    wr32(&mut buf, new_off + 0, child_ino);
                    wr16(&mut buf, new_off + 4, (rec_len - ideal) as u16);
                    buf[new_off + 6] = name_len as u8;
                    buf[new_off + 7] = ftype;
                    buf[new_off + 8..new_off + 8 + name_len].copy_from_slice(name.as_bytes());

                    self.write_block(blk, &buf)?;
                    return Ok(());
                }

                if off + rec_len == bs {
                    break;
                }

                off += rec_len;
            }
        }

        if blocks_used >= 12 {
            return Err(Ext2Err::Unsupported);
        }

        let new_blk = self.alloc_block_group0()?;
        let mut z = vec![0u8; bs];
        z.fill(0);

        wr32(&mut z, 0, child_ino);
        wr16(&mut z, 4, bs as u16);
        z[6] = name_len as u8;
        z[7] = ftype;
        z[8..8 + name_len].copy_from_slice(name.as_bytes());

        self.write_block(new_blk, &z)?;

        Self::inode_set_block_ptr(&mut dir_raw, blocks_used, new_blk);
        let new_size = (blocks_used + 1) * bs;
        Self::inode_set_size(&mut dir_raw, new_size as u32);

        let sectors = ((new_size as u32) + 511) / 512;
        Self::inode_set_blocks_512(&mut dir_raw, sectors);

        self.write_inode_raw(dir_ino, &dir_raw)?;
        Ok(())
    }

    pub fn mkdir(&self, path: &str) -> Result<(), Ext2Err> {
        let comps = Self::split_path(path);
        if comps.is_empty() {
            return Ok(());
        }

        let mut cur = EXT2_ROOT_INO;
        for c in comps {
            match self.lookup_in_dir_ci(cur, c) {
                Ok(child) => {
                    let (m, _, _) = self.inode_mode_size_blocks(child)?;
                    if !is_dir(m) {
                        return Err(Ext2Err::Name);
                    }
                    cur = child;
                }
                Err(Ext2Err::NotFound) => {
                    let new_dir = self.mkdir_one(cur, c)?;
                    cur = new_dir;
                }
                Err(e) => return Err(e),
            }
        }

        Ok(())
    }

    fn mkdir_one(&self, parent_ino: u32, name: &str) -> Result<u32, Ext2Err> {
        if name.is_empty() {
            return Err(Ext2Err::Name);
        }

        let dir_ino = self.alloc_inode_group0(true)?;
        let blk = self.alloc_block_group0()?;
        let bs = self.block_size as usize;

        let mut b = vec![0u8; bs];
        b.fill(0);

        wr32(&mut b, 0, dir_ino);
        wr16(&mut b, 4, dirent_ideal_len(1) as u16);
        b[6] = 1;
        b[7] = FT_DIR;
        b[8] = b'.';

        let off2 = dirent_ideal_len(1);
        wr32(&mut b, off2 + 0, parent_ino);
        wr16(&mut b, off2 + 4, (bs - off2) as u16);
        b[off2 + 6] = 2;
        b[off2 + 7] = FT_DIR;
        b[off2 + 8] = b'.';
        b[off2 + 9] = b'.';

        self.write_block(blk, &b)?;

        let mut raw = vec![0u8; self.inode_size as usize];
        Self::inode_init_common(&mut raw, 0x4000 | 0o755, 2);
        Self::inode_set_size(&mut raw, bs as u32);
        Self::inode_set_blocks_512(&mut raw, ((bs as u32) + 511) / 512);
        Self::inode_set_block_ptr(&mut raw, 0, blk);

        self.write_inode_raw(dir_ino, &raw)?;

        self.add_dir_entry(parent_ino, dir_ino, name, FT_DIR)?;

        let mut p = self.read_inode_mutable(parent_ino)?;
        let l = Self::inode_get_links(&p);
        Self::inode_set_links(&mut p, l.wrapping_add(1));
        self.write_inode_raw(parent_ino, &p)?;

        Ok(dir_ino)
    }

    pub fn write_file(&self, path: &str, data: &[u8], overwrite: bool) -> Result<(), Ext2Err> {
        let (parent, leaf) = self.resolve_parent_dir_ci(path)?;

        let existing_ino = self.lookup_in_dir_ci(parent, leaf).ok();

        if let Some(ino) = existing_ino {
            let (mode, size, ptrs) = self.inode_mode_size_blocks(ino)?;
            if !is_reg(mode) {
                return Err(Ext2Err::Name);
            }

            if overwrite {
                self.truncate_file(ino, &ptrs)?;
                self.write_file_fresh_into_inode(ino, data)?;
                return Ok(());
            } else {
                self.append_to_file(ino, size, &ptrs, data)?;
                return Ok(());
            }
        }

        let ino = self.alloc_inode_group0(false)?;

        let mut raw = vec![0u8; self.inode_size as usize];
        Self::inode_init_common(&mut raw, 0x8000 | 0o644, 1);
        Self::inode_set_size(&mut raw, 0);
        Self::inode_set_blocks_512(&mut raw, 0);

        self.write_inode_raw(ino, &raw)?;

        self.add_dir_entry(parent, ino, leaf, FT_REG)?;

        self.write_file_fresh_into_inode(ino, data)?;
        Ok(())
    }

    fn truncate_file(&self, ino: u32, ptrs: &[u32; 15]) -> Result<(), Ext2Err> {
        for i in 0..12usize {
            let b = ptrs[i];
            if b != 0 {
                self.free_block_group0(b)?;
            }
        }

        let mut raw = self.read_inode_mutable(ino)?;
        for i in 0..12usize {
            Self::inode_set_block_ptr(&mut raw, i, 0);
        }
        Self::inode_set_size(&mut raw, 0);
        Self::inode_set_blocks_512(&mut raw, 0);

        self.write_inode_raw(ino, &raw)?;
        Ok(())
    }

    fn write_file_fresh_into_inode(&self, ino: u32, data: &[u8]) -> Result<(), Ext2Err> {
        let bs = self.block_size as usize;
        let need_blocks = (data.len() + bs - 1) / bs;
        if need_blocks > 12 {
            return Err(Ext2Err::Unsupported);
        }

        let mut raw = self.read_inode_mutable(ino)?;

        let mut off = 0usize;
        for i in 0..need_blocks {
            let blk = self.alloc_block_group0()?;
            Self::inode_set_block_ptr(&mut raw, i, blk);

            let mut buf = vec![0u8; bs];
            buf.fill(0);
            let take = min(bs, data.len().saturating_sub(off));
            if take > 0 {
                buf[..take].copy_from_slice(&data[off..off + take]);
            }
            self.write_block(blk, &buf)?;
            off += take;
        }

        Self::inode_set_size(&mut raw, data.len() as u32);
        let sectors = ((need_blocks * bs) as u32 + 511) / 512;
        Self::inode_set_blocks_512(&mut raw, sectors);

        self.write_inode_raw(ino, &raw)?;
        Ok(())
    }

    fn append_to_file(
        &self,
        ino: u32,
        cur_size: u32,
        _ptrs: &[u32; 15],
        data: &[u8],
    ) -> Result<(), Ext2Err> {
        if data.is_empty() {
            ksprintln!("[ext2][append] ino={} empty append, nothing to do", ino);
            return Ok(());
        }

        let bs = self.block_size as usize;

        let cur = cur_size as usize;
        let need_total = cur + data.len();
        let mut blocks_total = (need_total + bs - 1) / bs;
        if blocks_total == 0 {
            blocks_total = 1;
        }
        if blocks_total > 12 {
            ksprintln!(
                "[ext2][append][ERR] ino={} unsupported blocks_total={} cur_size={} add={} bs={}",
                ino,
                blocks_total,
                cur_size,
                data.len(),
                bs
            );
            return Err(Ext2Err::Unsupported);
        }

        ksprintln!(
            "[ext2][append] ino={} cur_size={} add={} bs={}",
            ino,
            cur_size,
            data.len(),
            bs
        );
        ksprintln!(
            "[ext2][append] ino={} need_total={} blocks_total={}",
            ino,
            need_total,
            blocks_total
        );

        let mut raw = self.read_inode_mutable(ino)?;
        let mut blkptrs = Self::inode_get_block_ptrs(&raw);

        let have_blocks = (cur + bs - 1) / bs;
        ksprintln!(
            "[ext2][append] ino={} have_blocks={} initial_ptrs=[{},{},{},{},{},{},{},{},{},{},{},{}]",
            ino,
            have_blocks,
            blkptrs[0],
            blkptrs[1],
            blkptrs[2],
            blkptrs[3],
            blkptrs[4],
            blkptrs[5],
            blkptrs[6],
            blkptrs[7],
            blkptrs[8],
            blkptrs[9],
            blkptrs[10],
            blkptrs[11],
        );

        for i in 0..blocks_total {
            ksprintln!("[ext2][append] blkptr[{}]={}", i, blkptrs[i]);

            if blkptrs[i] == 0 {
                let nb = self.alloc_block_group0()?;
                ksprintln!("[ext2][append] alloc new block for index {} -> {}", i, nb);

                blkptrs[i] = nb;
                Self::inode_set_block_ptr(&mut raw, i, nb);

                let mut z = vec![0u8; bs];
                z.fill(0);

                ksprintln!("[ext2][append] zero-init block {}", nb);
                self.write_block(nb, &z)?;
            }
        }

        let mut write_off = 0usize;
        let mut file_off = cur;

        while write_off < data.len() {
            let bi = file_off / bs;
            let in_blk = file_off % bs;

            ksprintln!(
                "[ext2][append] write_off={} file_off={} bi={} in_blk={}",
                write_off,
                file_off,
                bi,
                in_blk
            );

            if bi >= blkptrs.len() {
                ksprintln!(
                    "[ext2][append][ERR] bi out of range: bi={} blkptrs_len={}",
                    bi,
                    blkptrs.len()
                );
                return Err(Ext2Err::Unsupported);
            }

            let blk = blkptrs[bi];
            ksprintln!("[ext2][append] selected blkptr[{}]={}", bi, blk);

            if blk == 0 {
                ksprintln!("[ext2][append][ERR] blkptr[{}] is zero during append", bi);
                return Err(Ext2Err::BadSuperblock);
            }

            let mut buf = vec![0u8; bs];
            ksprintln!("[ext2][append] read block {}", blk);
            self.read_block(blk, &mut buf)?;

            let space = bs - in_blk;
            let take = min(space, data.len() - write_off);

            ksprintln!(
                "[ext2][append] patch block {} at off {} take {}",
                blk,
                in_blk,
                take
            );

            buf[in_blk..in_blk + take].copy_from_slice(&data[write_off..write_off + take]);

            ksprintln!("[ext2][append] write block {}", blk);
            self.write_block(blk, &buf)?;

            write_off += take;
            file_off += take;
        }

        let new_size = (cur + data.len()) as u32;
        ksprintln!("[ext2][append] ino={} final new_size={}", ino, new_size);
        Self::inode_set_size(&mut raw, new_size);

        let sectors = ((blocks_total * bs) as u32 + 511) / 512;
        ksprintln!("[ext2][append] ino={} final sectors512={}", ino, sectors);
        Self::inode_set_blocks_512(&mut raw, sectors);

        ksprintln!("[ext2][append] write inode {}", ino);
        self.write_inode_raw(ino, &raw)?;

        ksprintln!("[ext2][append] ino={} done", ino);
        Ok(())
    }
}
