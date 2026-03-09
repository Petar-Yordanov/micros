#![allow(dead_code)]

extern crate alloc;
use alloc::vec;
use alloc::{string::String, vec::Vec};

use core::cmp::min;

use crate::kernel::drivers::virtio::blk::{read_at, write_at};
use crate::ksprintln;

const SEC: usize = 512;

#[derive(Debug)]
pub enum FatErr {
    Io,
    BadBootSig,
    NotFat32,
    BadBpb,
    NotFound,
    Name,
    Full,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct DirEnt {
    name: [u8; 11],
    attr: u8,
    ntres: u8,
    crt_time_tenth: u8,
    crt_time: u16,
    crt_date: u16,
    lst_acc_date: u16,
    fst_clus_hi: u16,
    wrt_time: u16,
    wrt_date: u16,
    fst_clus_lo: u16,
    file_size: u32,
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
struct LfnEnt {
    ord: u8,
    name1: [u16; 5],
    attr: u8,     // 0x0F
    lfn_type: u8, // 0
    checksum: u8,
    name2: [u16; 6],
    fst_clus_lo: u16, // 0
    name3: [u16; 2],
}

const ATTR_VOLUME: u8 = 0x08;
const ATTR_DIR: u8 = 0x10;
const ATTR_ARCHIVE: u8 = 0x20;

#[inline]
fn is_lfn(attr: u8) -> bool {
    (attr & 0x0F) == 0x0F
}

#[inline]
fn le16(x: u16) -> u16 {
    u16::from_le(x)
}
#[inline]
fn le32(x: u32) -> u32 {
    u32::from_le(x)
}

#[inline]
fn dirent_first_clus(e: &DirEnt) -> u32 {
    ((le16(e.fst_clus_hi) as u32) << 16) | (le16(e.fst_clus_lo) as u32)
}
#[inline]
fn set_dirent_first_clus(e: &mut DirEnt, cl: u32) {
    e.fst_clus_lo = (cl as u16).to_le();
    e.fst_clus_hi = ((cl >> 16) as u16).to_le();
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
fn eq_ci(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes()
        .zip(b.bytes())
        .all(|(x, y)| x.to_ascii_lowercase() == y.to_ascii_lowercase())
}

fn short_name_to_string(name11: &[u8; 11], is_dir: bool) -> String {
    let base = core::str::from_utf8(&name11[..8]).unwrap_or("").trim_end();
    let ext = core::str::from_utf8(&name11[8..11])
        .unwrap_or("")
        .trim_end();
    let mut s = String::new();
    s.push_str(base);
    if !is_dir && !ext.is_empty() {
        s.push('.');
        s.push_str(ext);
    }
    if is_dir {
        s.push('/');
    }
    s
}

fn lfn_checksum(short: &[u8; 11]) -> u8 {
    let mut sum: u8 = 0;
    for &b in short.iter() {
        sum = sum.rotate_right(1).wrapping_add(b);
    }
    sum
}

fn lfn_push_chars(out: &mut Vec<u16>, ent: &LfnEnt) {
    use core::ptr::{addr_of, read_unaligned};

    for i in 0..5 {
        let p = addr_of!((*ent).name1[i]);
        let v = unsafe { read_unaligned(p) };
        out.push(u16::from_le(v));
    }
    for i in 0..6 {
        let p = addr_of!((*ent).name2[i]);
        let v = unsafe { read_unaligned(p) };
        out.push(u16::from_le(v));
    }
    for i in 0..2 {
        let p = addr_of!((*ent).name3[i]);
        let v = unsafe { read_unaligned(p) };
        out.push(u16::from_le(v));
    }
}

fn lfn_utf16_to_string(chars: &[u16]) -> String {
    let mut s = String::new();
    for &u in chars {
        if u == 0x0000 {
            break;
        }
        if u == 0xFFFF {
            continue;
        }
        let ch = (u & 0x00FF) as u8;
        if ch == 0 || ch < 0x20 || ch >= 0x7F {
            s.push('?');
        } else {
            s.push(ch as char);
        }
    }
    s
}

fn string_to_lfn_utf16(s: &str) -> Result<Vec<u16>, FatErr> {
    if s.is_empty() {
        return Err(FatErr::Name);
    }
    let mut v = Vec::with_capacity(s.len() + 1);
    for b in s.bytes() {
        if b == b'/' || b == 0 {
            return Err(FatErr::Name);
        }
        if b < 0x20 || b >= 0x7F {
            return Err(FatErr::Name);
        }
        v.push(b as u16);
    }
    v.push(0u16);
    Ok(v)
}

fn is_valid_short_char(b: u8) -> bool {
    matches!(
        b,
        b'A'..=b'Z'
            | b'0'..=b'9'
            | b'_'
            | b'$'
            | b'~'
            | b'!'
            | b'#'
            | b'%'
            | b'&'
            | b'\''
            | b'('
            | b')'
            | b'-'
            | b'@'
            | b'^'
            | b'`'
            | b'{'
            | b'}'
    )
}

fn make_short_candidate(long: &str) -> Result<[u8; 11], FatErr> {
    let mut name = [b' '; 11];
    let parts: Vec<&str> = long.split('.').collect();
    if parts.is_empty() || parts.len() > 2 {
        return Err(FatErr::Name);
    }
    let base = parts[0];
    let ext = if parts.len() == 2 { parts[1] } else { "" };
    if base.is_empty() {
        return Err(FatErr::Name);
    }

    let upb = base.to_ascii_uppercase();
    let upe = ext.to_ascii_uppercase();

    let mut bi = 0usize;
    for ch in upb.bytes() {
        if bi >= 8 {
            break;
        }
        if ch <= 0x20 || ch >= 0x7F || ch == b'.' || ch == b'/' {
            continue;
        }
        if !is_valid_short_char(ch) {
            continue;
        }
        name[bi] = ch;
        bi += 1;
    }

    let mut ei = 0usize;
    for ch in upe.bytes() {
        if ei >= 3 {
            break;
        }
        if ch <= 0x20 || ch >= 0x7F || ch == b'.' || ch == b'/' {
            continue;
        }
        if !is_valid_short_char(ch) {
            continue;
        }
        name[8 + ei] = ch;
        ei += 1;
    }

    if name[0] == b' ' {
        return Err(FatErr::Name);
    }
    Ok(name)
}

pub struct Fat32 {
    base_off_bytes: u64,
    bps: u16,
    spc: u8,
    rsvd: u16,
    nfats: u8,
    fatsz: u32,
    totsec: u32,

    fat_start_sec: u32,
    data_start_sec: u32,
    data_clusters: u32,

    root_clus: u32,
}

impl Fat32 {
    #[inline]
    fn sec_off(&self, lba_sec: u32) -> u64 {
        self.base_off_bytes + (lba_sec as u64) * (self.bps as u64)
    }

    fn read_sector(&self, lba_sec: u32, buf: &mut [u8; SEC]) -> Result<(), FatErr> {
        if !read_at(self.sec_off(lba_sec), buf) {
            return Err(FatErr::Io);
        }
        Ok(())
    }

    fn write_sector(&self, lba_sec: u32, buf: &[u8; SEC]) -> Result<(), FatErr> {
        if !write_at(self.sec_off(lba_sec), buf) {
            return Err(FatErr::Io);
        }
        Ok(())
    }

    pub fn mount(base_off_bytes: u64) -> Result<Self, FatErr> {
        let mut bs = [0u8; SEC];
        if !read_at(base_off_bytes, &mut bs) {
            return Err(FatErr::Io);
        }

        if bs[510] != 0x55 || bs[511] != 0xAA {
            return Err(FatErr::BadBootSig);
        }

        let bps = rd16(&bs, 11);
        if bps != 512 && bps != 1024 && bps != 2048 && bps != 4096 {
            return Err(FatErr::BadBpb);
        }
        let spc = bs[13];
        let rsvd = rd16(&bs, 14);
        let nf = bs[16];

        let root_ent_cnt = rd16(&bs, 17);
        let fat_sz16 = rd16(&bs, 22);

        let tot16 = rd16(&bs, 19);
        let tot32 = rd32(&bs, 32);
        let tot = if tot16 != 0 { tot16 as u32 } else { tot32 };

        let fatsz32 = rd32(&bs, 36);
        let root_clus = rd32(&bs, 44);

        if root_ent_cnt != 0 {
            return Err(FatErr::NotFat32);
        }
        if fat_sz16 != 0 {
            return Err(FatErr::NotFat32);
        }

        if fatsz32 == 0 || nf == 0 || rsvd == 0 || spc == 0 || tot == 0 {
            return Err(FatErr::BadBpb);
        }
        if root_clus < 2 {
            return Err(FatErr::BadBpb);
        }

        let fat_start_sec = rsvd as u32;
        let data_start_sec = fat_start_sec + (nf as u32) * fatsz32;

        let data_secs = tot.saturating_sub(data_start_sec);
        let data_clusters = data_secs / (spc as u32);

        if data_clusters < 65525 {
            return Err(FatErr::NotFat32);
        }

        let fs = Self {
            base_off_bytes,
            bps,
            spc,
            rsvd,
            nfats: nf,
            fatsz: fatsz32,
            totsec: tot,
            fat_start_sec,
            data_start_sec,
            data_clusters,
            root_clus,
        };

        ksprintln!(
            "[fat32] mount: bps={} spc={} rsvd={} nfats={} fatsz={} totsec={} first_data={} clusters={} root_clus={}",
            bps, spc, rsvd, nf, fatsz32, tot, data_start_sec, data_clusters, root_clus
        );

        Ok(fs)
    }

    #[inline]
    fn clus_to_first_sector(&self, clus: u32) -> u32 {
        self.data_start_sec + ((clus - 2) * self.spc as u32)
    }

    #[inline]
    fn is_eoc(v: u32) -> bool {
        v >= 0x0FFF_FFF8
    }

    fn fat_entry_read(&self, clus: u32) -> Result<u32, FatErr> {
        let idx = clus * 4;
        let sec_ofs = idx / (self.bps as u32);
        let off_in_sec = (idx % (self.bps as u32)) as usize;

        let fat_sec = self.fat_start_sec + sec_ofs;
        let mut sec = [0u8; SEC];
        self.read_sector(fat_sec, &mut sec)?;

        let raw = u32::from_le_bytes([
            sec[off_in_sec],
            sec[off_in_sec + 1],
            sec[off_in_sec + 2],
            sec[off_in_sec + 3],
        ]);

        Ok(raw & 0x0FFF_FFFF)
    }

    fn fat_entry_write(&self, clus: u32, val: u32) -> Result<(), FatErr> {
        let idx = clus * 4;
        let sec_ofs = idx / (self.bps as u32);
        let off_in_sec = (idx % (self.bps as u32)) as usize;

        let fat_sec0 = self.fat_start_sec + sec_ofs;
        let bytes = (val & 0x0FFF_FFFF).to_le_bytes();

        for f in 0..self.nfats {
            let secno = fat_sec0 + (f as u32) * self.fatsz;
            let mut sec = [0u8; SEC];
            self.read_sector(secno, &mut sec)?;
            sec[off_in_sec] = bytes[0];
            sec[off_in_sec + 1] = bytes[1];
            sec[off_in_sec + 2] = bytes[2];
            sec[off_in_sec + 3] = bytes[3];
            self.write_sector(secno, &sec)?;
        }
        Ok(())
    }

    fn alloc_free_cluster(&self) -> Result<Option<u32>, FatErr> {
        let max = self.data_clusters + 2;
        let mut clus = 2u32;
        while clus < max {
            if self.fat_entry_read(clus)? == 0 {
                self.fat_entry_write(clus, 0x0FFF_FFF7)?;
                return Ok(Some(clus));
            }
            clus += 1;
        }
        Ok(None)
    }

    fn free_cluster_chain(&self, start: u32) -> Result<(), FatErr> {
        if start < 2 {
            return Ok(());
        }
        let mut cur = start;
        loop {
            let nxt = self.fat_entry_read(cur)?;
            self.fat_entry_write(cur, 0)?;
            if nxt < 2 || Self::is_eoc(nxt) {
                break;
            }
            cur = nxt;
        }
        Ok(())
    }

    fn read_cluster_chain(&self, start_clus: u32, mut max_bytes: usize) -> Result<Vec<u8>, FatErr> {
        if start_clus < 2 || max_bytes == 0 {
            return Ok(Vec::new());
        }
        let mut out = Vec::with_capacity(max_bytes);
        let mut cur = start_clus;

        loop {
            let first_sec = self.clus_to_first_sector(cur);
            for s in 0..(self.spc as u32) {
                let mut secbuf = [0u8; SEC];
                self.read_sector(first_sec + s, &mut secbuf)?;
                let take = min(SEC, max_bytes);
                out.extend_from_slice(&secbuf[..take]);
                if take < SEC {
                    max_bytes = 0;
                    break;
                } else {
                    max_bytes = max_bytes.saturating_sub(SEC);
                }
            }
            if max_bytes == 0 {
                break;
            }

            let nxt = self.fat_entry_read(cur)?;
            if Self::is_eoc(nxt) || nxt < 2 {
                break;
            }
            cur = nxt;
        }
        Ok(out)
    }

    fn write_cluster_chain_from(&self, start_clus: u32, data: &[u8]) -> Result<u32, FatErr> {
        let mut cur = start_clus;
        let mut off = 0usize;
        let clus_bytes = (self.spc as usize) * (self.bps as usize);

        loop {
            let first_sec = self.clus_to_first_sector(cur);
            let chunk = min(clus_bytes, data.len().saturating_sub(off));
            let mut left = chunk;

            for s in 0..(self.spc as u32) {
                let mut secbuf = [0u8; SEC];
                let n = min(SEC, left);
                if n > 0 {
                    secbuf[..n].copy_from_slice(&data[off..off + n]);
                } else {
                    secbuf.fill(0);
                }
                self.write_sector(first_sec + s, &secbuf)?;
                if left > 0 {
                    left = left.saturating_sub(n);
                    off += n;
                }
            }

            if off >= data.len() {
                break;
            }

            let next = self.fat_entry_read(cur)?;
            if Self::is_eoc(next) {
                let newc = self.alloc_free_cluster()?.ok_or(FatErr::Full)?;
                self.fat_entry_write(cur, newc)?;
                self.fat_entry_write(newc, 0x0FFF_FFFF)?;
                cur = newc;
            } else {
                cur = next;
            }
        }

        self.fat_entry_write(cur, 0x0FFF_FFFF)?;
        Ok(cur)
    }

    fn append_to_chain(
        &self,
        start_clus: u32,
        cur_size: u32,
        data: &[u8],
    ) -> Result<(u32, u32), FatErr> {
        if data.is_empty() {
            return Ok((start_clus, cur_size));
        }

        let mut start = start_clus;
        if start < 2 {
            start = self.alloc_free_cluster()?.ok_or(FatErr::Full)?;
            self.fat_entry_write(start, 0x0FFF_FFFF)?;
        }

        let clus_bytes = (self.spc as u32) * (self.bps as u32);

        let mut cur = start;
        let mut remaining = cur_size;
        while remaining >= clus_bytes {
            remaining -= clus_bytes;
            let nxt = self.fat_entry_read(cur)?;
            if nxt < 2 || Self::is_eoc(nxt) {
                let newc = self.alloc_free_cluster()?.ok_or(FatErr::Full)?;
                self.fat_entry_write(cur, newc)?;
                self.fat_entry_write(newc, 0x0FFF_FFFF)?;
                cur = newc;
            } else {
                cur = nxt;
            }
        }

        let mut write_off = 0usize;
        let mut file_off = remaining as usize;

        loop {
            let first_sec = self.clus_to_first_sector(cur);

            let clus_total = clus_bytes as usize;
            let mut tmpclus = vec![0u8; clus_total];

            // read whole cluster
            for s in 0..(self.spc as u32) {
                let mut secbuf = [0u8; SEC];
                self.read_sector(first_sec + s, &mut secbuf)?;
                let base = (s as usize) * SEC;
                tmpclus[base..base + SEC].copy_from_slice(&secbuf);
            }

            let space = clus_total.saturating_sub(file_off);
            let take = min(space, data.len().saturating_sub(write_off));
            tmpclus[file_off..file_off + take].copy_from_slice(&data[write_off..write_off + take]);

            // write cluster back
            for s in 0..(self.spc as u32) {
                let base = (s as usize) * SEC;
                let mut secbuf = [0u8; SEC];
                secbuf.copy_from_slice(&tmpclus[base..base + SEC]);
                self.write_sector(first_sec + s, &secbuf)?;
            }

            write_off += take;
            if write_off >= data.len() {
                break;
            }

            let nxt = self.fat_entry_read(cur)?;
            if nxt < 2 || Self::is_eoc(nxt) {
                let newc = self.alloc_free_cluster()?.ok_or(FatErr::Full)?;
                self.fat_entry_write(cur, newc)?;
                self.fat_entry_write(newc, 0x0FFF_FFFF)?;
                cur = newc;
            } else {
                cur = nxt;
            }
            file_off = 0;
        }

        Ok((start, cur_size.wrapping_add(data.len() as u32)))
    }

    fn dir_list_from_with_names(&self, start_clus: u32) -> Result<Vec<(String, DirEnt)>, FatErr> {
        let bytes = 256 * 1024;
        let raw = self.read_cluster_chain(start_clus, bytes)?;

        let mut out: Vec<(String, DirEnt)> = Vec::new();

        let mut lfn_ents: Vec<LfnEnt> = Vec::new();
        let mut lfn_chk: Option<u8> = None;

        for chunk in raw.chunks_exact(32) {
            let first = chunk[0];
            if first == 0x00 {
                break;
            }
            if first == 0xE5 {
                lfn_ents.clear();
                lfn_chk = None;
                continue;
            }

            let attr = chunk[11];
            if is_lfn(attr) {
                let ent: LfnEnt =
                    unsafe { core::ptr::read_unaligned(chunk.as_ptr() as *const LfnEnt) };
                if (ent.ord & 0x40) != 0 {
                    lfn_ents.clear();
                    lfn_chk = Some(ent.checksum);
                }
                if lfn_chk.is_some() {
                    lfn_ents.push(ent);
                }
                continue;
            }

            let ent: DirEnt = unsafe { core::ptr::read_unaligned(chunk.as_ptr() as *const DirEnt) };

            if (ent.attr & ATTR_VOLUME) != 0 {
                lfn_ents.clear();
                lfn_chk = None;
                continue;
            }

            let mut name = String::new();

            if let Some(exp) = lfn_chk {
                if lfn_checksum(&ent.name) == exp && !lfn_ents.is_empty() {
                    lfn_ents.sort_by(|a, b| (b.ord & 0x1F).cmp(&(a.ord & 0x1F)));
                    let mut chars_all: Vec<u16> = Vec::new();
                    for e in lfn_ents.iter() {
                        lfn_push_chars(&mut chars_all, e);
                    }
                    name = lfn_utf16_to_string(&chars_all);
                }
            }

            lfn_ents.clear();
            lfn_chk = None;

            if name.is_empty() {
                name = short_name_to_string(&ent.name, (ent.attr & ATTR_DIR) != 0);
            } else if (ent.attr & ATTR_DIR) != 0 && !name.ends_with('/') {
                name.push('/');
            }

            out.push((name, ent));
        }

        Ok(out)
    }

    fn dir_find_in_dir(&self, dirclus: u32, want: &str) -> Result<Option<DirEnt>, FatErr> {
        for (name, ent) in self.dir_list_from_with_names(dirclus)? {
            let n = if name.ends_with('/') {
                &name[..name.len() - 1]
            } else {
                &name
            };
            if eq_ci(n, want) {
                return Ok(Some(ent));
            }
        }
        Ok(None)
    }

    fn split_path(p: &str) -> Vec<&str> {
        p.split('/').filter(|c| !c.is_empty()).collect()
    }

    fn resolve_dir(&self, path: &str) -> Result<Option<u32>, FatErr> {
        let comps = Self::split_path(path);
        if comps.is_empty() {
            return Ok(None);
        }

        let mut cur: u32 = self.root_clus;
        for c in comps {
            let ent = self.dir_find_in_dir(cur, c)?.ok_or(FatErr::NotFound)?;
            if (ent.attr & ATTR_DIR) == 0 {
                return Err(FatErr::NotFound);
            }
            let cl = dirent_first_clus(&ent);
            if cl < 2 {
                return Err(FatErr::BadBpb);
            }
            cur = cl;
        }
        Ok(Some(cur))
    }

    fn dir_find_free_run(
        &self,
        dirclus: u32,
        need_entries: usize,
    ) -> Result<(u32, u32, usize), FatErr> {
        let mut cur = dirclus;
        loop {
            let first_sec = self.clus_to_first_sector(cur);
            for s in 0..(self.spc as u32) {
                let secno = first_sec + s;
                let mut buf = [0u8; SEC];
                self.read_sector(secno, &mut buf)?;

                let mut run = 0usize;
                let mut run_start = 0usize;

                for i in 0..(SEC / 32) {
                    let p = &buf[i * 32..i * 32 + 32];
                    let b0 = p[0];
                    let free = b0 == 0x00 || b0 == 0xE5;
                    if free {
                        if run == 0 {
                            run_start = i;
                        }
                        run += 1;
                        if run >= need_entries {
                            return Ok((cur, s, run_start));
                        }
                    } else {
                        run = 0;
                    }
                }
            }

            let nxt = self.fat_entry_read(cur)?;
            if nxt < 2 || Self::is_eoc(nxt) {
                let newc = self.alloc_free_cluster()?.ok_or(FatErr::Full)?;
                self.fat_entry_write(cur, newc)?;
                self.fat_entry_write(newc, 0x0FFF_FFFF)?;
                let zero = [0u8; SEC];
                let first_sec_new = self.clus_to_first_sector(newc);
                for s in 0..(self.spc as u32) {
                    self.write_sector(first_sec_new + s, &zero)?;
                }
                cur = newc;
            } else {
                cur = nxt;
            }
        }
    }

    fn write_dir_entries_at(&self, dirclus: u32, entries: &[[u8; 32]]) -> Result<(), FatErr> {
        let need = entries.len();
        let (clus, sec_in_clus, entry_start) = self.dir_find_free_run(dirclus, need)?;
        let first_sec = self.clus_to_first_sector(clus);
        let secno = first_sec + sec_in_clus;

        let mut buf = [0u8; SEC];
        self.read_sector(secno, &mut buf)?;

        for (k, raw) in entries.iter().enumerate() {
            let idx = entry_start + k;
            let dst = &mut buf[idx * 32..idx * 32 + 32];
            dst.copy_from_slice(raw);
        }

        self.write_sector(secno, &buf)?;
        Ok(())
    }

    fn build_lfn_entries(short: &[u8; 11], long: &str) -> Result<Vec<[u8; 32]>, FatErr> {
        let utf16 = string_to_lfn_utf16(long)?;
        if utf16.is_empty() {
            return Err(FatErr::Name);
        }

        let mut chunks: Vec<&[u16]> = Vec::new();
        let mut i = 0usize;
        while i < utf16.len() {
            let end = (i + 13).min(utf16.len());
            chunks.push(&utf16[i..end]);
            i = end;
        }
        let total = chunks.len();
        if total == 0 {
            return Err(FatErr::Name);
        }

        let chk = lfn_checksum(short);

        #[inline]
        fn put_u16_le(dst: &mut [u8; 32], off: usize, v: u16) {
            let b = v.to_le_bytes();
            dst[off] = b[0];
            dst[off + 1] = b[1];
        }

        const OFFS: [usize; 13] = [1, 3, 5, 7, 9, 14, 16, 18, 20, 22, 24, 28, 30];

        let mut out: Vec<[u8; 32]> = Vec::with_capacity(total);

        for idx in 0..total {
            let chunk = chunks[idx];
            let ord_num = (total - idx) as u8;
            let ord = if idx == 0 { ord_num | 0x40 } else { ord_num };

            let mut raw = [0u8; 32];

            raw[0] = ord;
            raw[11] = 0x0F; // attr
            raw[12] = 0x00; // type
            raw[13] = chk; // checksum
            raw[26] = 0x00;
            raw[27] = 0x00;

            // Fill all 13 slots with 0xFFFF padding
            for &o in OFFS.iter() {
                put_u16_le(&mut raw, o, 0xFFFF);
            }

            // Copy chunk data into slots
            for j in 0..chunk.len().min(13) {
                put_u16_le(&mut raw, OFFS[j], chunk[j]);
            }

            out.push(raw);
        }

        Ok(out)
    }

    fn make_unique_short_name_in_dir(&self, dirclus: u32, long: &str) -> Result<[u8; 11], FatErr> {
        let mut base = make_short_candidate(long)?;
        let mut n = 1u8;

        loop {
            let mut exists = false;
            for (_nm, ent) in self.dir_list_from_with_names(dirclus)? {
                if ent.name == base {
                    exists = true;
                    break;
                }
            }
            if !exists {
                return Ok(base);
            }

            if n == 0 || n > 9 {
                return Err(FatErr::Full);
            }

            let mut b = [b' '; 11];
            b[8..11].copy_from_slice(&base[8..11]);

            let mut stem: [u8; 6] = [b' '; 6];
            let mut si = 0usize;
            for i in 0..8 {
                let c = base[i];
                if c == b' ' {
                    break;
                }
                if si < 6 {
                    stem[si] = c;
                    si += 1;
                }
            }
            b[0..6].copy_from_slice(&stem);
            b[6] = b'~';
            b[7] = b'0' + n;

            base = b;
            n += 1;
        }
    }

    fn dir_write_file_entry(
        &self,
        dirclus: u32,
        long_name: &str,
        start_clus: u32,
        size: u32,
        overwrite_existing_short: Option<[u8; 11]>,
        is_dir: bool,
    ) -> Result<(), FatErr> {
        if let Some(short) = overwrite_existing_short {
            let mut cur = dirclus;
            loop {
                let first_sec = self.clus_to_first_sector(cur);
                for s in 0..(self.spc as u32) {
                    let secno = first_sec + s;
                    let mut buf = [0u8; SEC];
                    self.read_sector(secno, &mut buf)?;
                    for i in 0..(SEC / 32) {
                        let p = &mut buf[i * 32..i * 32 + 32];
                        if p[0] == 0x00 || p[0] == 0xE5 {
                            continue;
                        }
                        if is_lfn(p[11]) {
                            continue;
                        }
                        if &p[0..11] == &short {
                            let mut ent: DirEnt =
                                unsafe { core::ptr::read_unaligned(p.as_ptr() as *const DirEnt) };
                            ent.attr = if is_dir { ATTR_DIR } else { ATTR_ARCHIVE };
                            set_dirent_first_clus(&mut ent, start_clus);
                            ent.file_size = size.to_le();
                            unsafe {
                                core::ptr::copy_nonoverlapping(
                                    (&ent as *const DirEnt) as *const u8,
                                    p.as_mut_ptr(),
                                    32,
                                );
                            }
                            self.write_sector(secno, &buf)?;
                            return Ok(());
                        }
                    }
                }
                let nxt = self.fat_entry_read(cur)?;
                if nxt < 2 || Self::is_eoc(nxt) {
                    break;
                }
                cur = nxt;
            }
            return Err(FatErr::NotFound);
        }

        let short = self.make_unique_short_name_in_dir(dirclus, long_name)?;
        let lfn_raws = Self::build_lfn_entries(&short, long_name)?;

        let mut short_ent = DirEnt {
            name: short,
            attr: if is_dir { ATTR_DIR } else { ATTR_ARCHIVE },
            ntres: 0,
            crt_time_tenth: 0,
            crt_time: 0,
            crt_date: 0,
            lst_acc_date: 0,
            fst_clus_hi: 0,
            wrt_time: 0,
            wrt_date: 0,
            fst_clus_lo: 0,
            file_size: 0,
        };
        set_dirent_first_clus(&mut short_ent, start_clus);
        short_ent.file_size = size.to_le();

        let mut short_raw = [0u8; 32];
        unsafe {
            core::ptr::copy_nonoverlapping(
                (&short_ent as *const DirEnt) as *const u8,
                short_raw.as_mut_ptr(),
                32,
            );
        }

        let mut entries: Vec<[u8; 32]> = Vec::new();
        for raw in lfn_raws {
            entries.push(raw);
        }
        entries.push(short_raw);

        self.write_dir_entries_at(dirclus, &entries)?;
        Ok(())
    }

    pub fn read_file(&self, path: &str) -> Result<Vec<u8>, FatErr> {
        let comps = Self::split_path(path);
        if comps.is_empty() {
            return Err(FatErr::Name);
        }

        let mut cur_dir: u32 = self.root_clus;
        for (i, c) in comps.iter().enumerate() {
            let ent = self.dir_find_in_dir(cur_dir, c)?.ok_or(FatErr::NotFound)?;
            if i + 1 < comps.len() {
                if (ent.attr & ATTR_DIR) == 0 {
                    return Err(FatErr::NotFound);
                }
                let cl = dirent_first_clus(&ent);
                if cl < 2 {
                    return Err(FatErr::BadBpb);
                }
                cur_dir = cl;
            } else {
                if (ent.attr & ATTR_DIR) != 0 {
                    return Err(FatErr::NotFound);
                }
                let size = le32(ent.file_size) as usize;
                let start = dirent_first_clus(&ent);
                if start < 2 && size > 0 {
                    return Err(FatErr::BadBpb);
                }
                let mut data = self.read_cluster_chain(start, size)?;
                data.truncate(size);
                return Ok(data);
            }
        }
        Err(FatErr::NotFound)
    }

    pub fn mkdir(&self, path: &str) -> Result<(), FatErr> {
        let comps = Self::split_path(path);
        if comps.is_empty() {
            return Ok(());
        }

        let mut cur_dir: u32 = self.root_clus;
        for c in comps {
            if let Some(ent) = self.dir_find_in_dir(cur_dir, c)? {
                if (ent.attr & ATTR_DIR) == 0 {
                    return Err(FatErr::Name);
                }
                let cl = dirent_first_clus(&ent);
                if cl < 2 {
                    return Err(FatErr::BadBpb);
                }
                cur_dir = cl;
            } else {
                self.mkdir_at(Some(cur_dir), c)?;
                let ent = self.dir_find_in_dir(cur_dir, c)?.ok_or(FatErr::NotFound)?;
                cur_dir = dirent_first_clus(&ent);
            }
        }

        Ok(())
    }

    pub fn mkdir_at(&self, parent_dir: Option<u32>, name: &str) -> Result<(), FatErr> {
        let parent = parent_dir.unwrap_or(self.root_clus);

        if self.dir_find_in_dir(parent, name)?.is_some() {
            return Ok(());
        }

        let cl = self.alloc_free_cluster()?.ok_or(FatErr::Full)?;
        self.fat_entry_write(cl, 0x0FFF_FFFF)?;

        let zero = [0u8; SEC];
        let first_sec = self.clus_to_first_sector(cl);
        for s in 0..(self.spc as u32) {
            self.write_sector(first_sec + s, &zero)?;
        }

        // "." and ".." short entries
        let mut dot_name = [b' '; 11];
        dot_name[0] = b'.';
        let mut dotdot_name = [b' '; 11];
        dotdot_name[0] = b'.';
        dotdot_name[1] = b'.';

        let mut dot = DirEnt {
            name: dot_name,
            attr: ATTR_DIR,
            ntres: 0,
            crt_time_tenth: 0,
            crt_time: 0,
            crt_date: 0,
            lst_acc_date: 0,
            fst_clus_hi: 0,
            wrt_time: 0,
            wrt_date: 0,
            fst_clus_lo: 0,
            file_size: 0,
        };
        set_dirent_first_clus(&mut dot, cl);

        let mut dotdot = DirEnt {
            name: dotdot_name,
            attr: ATTR_DIR,
            ntres: 0,
            crt_time_tenth: 0,
            crt_time: 0,
            crt_date: 0,
            lst_acc_date: 0,
            fst_clus_hi: 0,
            wrt_time: 0,
            wrt_date: 0,
            fst_clus_lo: 0,
            file_size: 0,
        };
        set_dirent_first_clus(&mut dotdot, parent);

        let mut sec0 = [0u8; SEC];
        self.read_sector(first_sec, &mut sec0)?;
        unsafe {
            core::ptr::copy_nonoverlapping(
                (&dot as *const DirEnt) as *const u8,
                sec0.as_mut_ptr(),
                32,
            );
            core::ptr::copy_nonoverlapping(
                (&dotdot as *const DirEnt) as *const u8,
                sec0.as_mut_ptr().add(32),
                32,
            );
        }
        self.write_sector(first_sec, &sec0)?;

        self.dir_write_file_entry(parent, name, cl, 0, None, true)?;
        Ok(())
    }

    pub fn list_root(&self) -> Result<Vec<String>, FatErr> {
        self.list_dir("/")
    }

    pub fn list_dir(&self, path: &str) -> Result<Vec<String>, FatErr> {
        let dirclus = if path == "/" || path.is_empty() {
            self.root_clus
        } else {
            self.resolve_dir(path)?.ok_or(FatErr::BadBpb)?
        };

        let ents = self.dir_list_from_with_names(dirclus)?;
        let mut out = Vec::new();
        for (name, _ent) in ents {
            out.push(name);
        }
        Ok(out)
    }

    pub fn write_file(&self, path: &str, data: &[u8], overwrite: bool) -> Result<(), FatErr> {
        let comps = Self::split_path(path);
        if comps.is_empty() {
            return Err(FatErr::Name);
        }

        let (parent, fname) = comps.split_at(comps.len() - 1);
        let parent_dir = if parent.is_empty() {
            Some(self.root_clus)
        } else {
            let p = parent.join("/");
            self.resolve_dir(&p)?.or(Some(self.root_clus))
        };
        self.write_file_at(parent_dir, fname[0], data, overwrite)
    }

    fn write_file_at(
        &self,
        parent_dir: Option<u32>,
        name: &str,
        data: &[u8],
        overwrite: bool,
    ) -> Result<(), FatErr> {
        let dirclus = parent_dir.unwrap_or(self.root_clus);

        let mut existing: Option<DirEnt> = None;
        let mut existing_short: Option<[u8; 11]> = None;

        for (nm, ent) in self.dir_list_from_with_names(dirclus)? {
            let nn = if nm.ends_with('/') {
                &nm[..nm.len() - 1]
            } else {
                &nm
            };
            if eq_ci(nn, name) {
                if (ent.attr & ATTR_DIR) != 0 {
                    return Err(FatErr::Name);
                }
                existing_short = Some(ent.name);
                existing = Some(ent);
                break;
            }
        }

        if let Some(ent) = existing {
            let old_size = le32(ent.file_size);
            let old_start = dirent_first_clus(&ent);

            if overwrite {
                if old_start >= 2 {
                    self.free_cluster_chain(old_start)?;
                }

                let start = if data.is_empty() {
                    0
                } else {
                    let c = self.alloc_free_cluster()?.ok_or(FatErr::Full)?;
                    self.fat_entry_write(c, 0x0FFF_FFFF)?;
                    self.write_cluster_chain_from(c, data)?;
                    c
                };

                self.dir_write_file_entry(
                    dirclus,
                    name,
                    start,
                    data.len() as u32,
                    Some(existing_short.unwrap()),
                    false,
                )?;
                return Ok(());
            } else {
                let (start, new_size) = self.append_to_chain(old_start, old_size, data)?;
                self.dir_write_file_entry(
                    dirclus,
                    name,
                    start,
                    new_size,
                    Some(existing_short.unwrap()),
                    false,
                )?;
                return Ok(());
            }
        }

        let start = if data.is_empty() {
            0
        } else {
            let c = self.alloc_free_cluster()?.ok_or(FatErr::Full)?;
            self.fat_entry_write(c, 0x0FFF_FFFF)?;
            self.write_cluster_chain_from(c, data)?;
            c
        };

        self.dir_write_file_entry(dirclus, name, start, data.len() as u32, None, false)?;
        Ok(())
    }
}
