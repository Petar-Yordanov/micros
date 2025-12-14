#![allow(dead_code)]

extern crate alloc;
use alloc::{string::String, vec::Vec};

use core::cmp::min;

use crate::kernel::drivers::virtio::blk::{read_at, write_at};
use crate::sprintln;

const SEC: usize = 512;

#[derive(Debug)]
pub enum FatErr {
    Io,
    BadBootSig,
    NotFat16,
    BadBpb,
    NotFound,
    Name,
    Full,
}

#[repr(C, packed)]
struct Bpb16 {
    jmp_boot: [u8; 3],
    oem_name: [u8; 8],
    bytes_per_sec: u16,
    sec_per_clus: u8,
    rsvd_sec_cnt: u16,
    num_fats: u8,
    root_ent_cnt: u16,
    tot_sec16: u16,
    media: u8,
    fat_sz16: u16,
    sec_per_trk: u16,
    num_heads: u16,
    hidd_sec: u32,
    tot_sec32: u32,
    drv_num: u8,
    reserved1: u8,
    boot_sig: u8,
    vol_id: u32,
    vol_lab: [u8; 11],
    fs_type: [u8; 8],
}

#[repr(C, packed)]
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

const ATTR_READONLY: u8 = 0x01;
const ATTR_HIDDEN: u8 = 0x02;
const ATTR_SYSTEM: u8 = 0x04;
const ATTR_VOLUME: u8 = 0x08;
const ATTR_DIR: u8 = 0x10;
const ATTR_ARCHIVE: u8 = 0x20;

fn le16(b: u16) -> u16 {
    u16::from_le(b)
}
fn le32(b: u32) -> u32 {
    u32::from_le(b)
}

#[inline]
fn is_lfn(attr: u8) -> bool {
    (attr & 0x0F) == 0x0F
}

pub struct Fat16 {
    base_off_bytes: u64,
    bps: u16,
    spc: u8,
    rsvd: u16,
    nfats: u8,
    root_ent: u16,
    fatsz: u16,
    totsec: u32,

    fat_start_sec: u32,
    root_start_sec: u32,
    root_sectors: u32,
    data_start_sec: u32,
    data_clusters: u32,
}

impl Fat16 {
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

        let bpb: &Bpb16 = unsafe { &*(bs.as_ptr() as *const Bpb16) };

        let bps = le16(bpb.bytes_per_sec);
        if bps != 512 && bps != 1024 && bps != 2048 && bps != 4096 {
            return Err(FatErr::BadBpb);
        }
        let spc = bpb.sec_per_clus;
        let rsvd = le16(bpb.rsvd_sec_cnt);
        let nf = bpb.num_fats;
        let root_ent = le16(bpb.root_ent_cnt);
        let fatsz = le16(bpb.fat_sz16);
        if fatsz == 0 || nf == 0 {
            return Err(FatErr::BadBpb);
        }

        let tot = if le16(bpb.tot_sec16) != 0 {
            le16(bpb.tot_sec16) as u32
        } else {
            le32(bpb.tot_sec32)
        };

        let fat_start_sec = rsvd as u32;
        let root_start_sec = fat_start_sec + (nf as u32) * (fatsz as u32);
        let root_bytes = (root_ent as u32) * 32;
        let root_sectors = (root_bytes + (bps as u32) - 1) / (bps as u32);
        let data_start_sec = root_start_sec + root_sectors;

        let data_secs = tot.saturating_sub(data_start_sec);
        let data_clusters = data_secs / (spc as u32);
        if !(data_clusters >= 4086 && data_clusters < 65525) {
            return Err(FatErr::NotFat16);
        }

        let fs = Self {
            base_off_bytes,
            bps,
            spc,
            rsvd,
            nfats: nf,
            root_ent,
            fatsz,
            totsec: tot,
            fat_start_sec,
            root_start_sec,
            root_sectors,
            data_start_sec,
            data_clusters,
        };
        sprintln!(
            "[fat16] mount: bps={} spc={} fatsz={} nfats={} totsec={} rootsec={} first_data={} clusters={}",
            bps, spc, fatsz, nf, tot, root_sectors, data_start_sec, data_clusters
        );
        Ok(fs)
    }

    fn fat_entry_read(&self, clus: u16) -> Result<u16, FatErr> {
        let idx = clus as u32 * 2;
        let sec_ofs = idx / (self.bps as u32);
        let off_in_sec = (idx % (self.bps as u32)) as usize;

        let fat_sec = self.fat_start_sec + sec_ofs;
        let mut sec = [0u8; SEC];
        self.read_sector(fat_sec, &mut sec)?;
        let val = u16::from_le_bytes([sec[off_in_sec], sec[off_in_sec + 1]]);
        Ok(val)
    }

    fn fat_entry_write(&self, clus: u16, val: u16) -> Result<(), FatErr> {
        let idx = clus as u32 * 2;
        let sec_ofs = idx / (self.bps as u32);
        let off_in_sec = (idx % (self.bps as u32)) as usize;

        let fat_sec0 = self.fat_start_sec + sec_ofs;
        let bytes = val.to_le_bytes();

        for f in 0..self.nfats {
            let secno = fat_sec0 + (f as u32) * (self.fatsz as u32);
            let mut sec = [0u8; SEC];
            self.read_sector(secno, &mut sec)?;
            sec[off_in_sec] = bytes[0];
            sec[off_in_sec + 1] = bytes[1];
            self.write_sector(secno, &sec)?;
        }
        Ok(())
    }

    #[inline]
    fn is_eoc(v: u16) -> bool {
        v >= 0xFFF8
    }

    #[inline]
    fn clus_to_first_sector(&self, clus: u16) -> u32 {
        self.data_start_sec + ((clus as u32 - 2) * self.spc as u32)
    }

    fn read_cluster_chain(&self, start_clus: u16, mut max_bytes: usize) -> Result<Vec<u8>, FatErr> {
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

    fn write_cluster_chain_from(&self, start_clus: u16, data: &[u8]) -> Result<(), FatErr> {
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
                    for i in 0..n {
                        secbuf[i] = data[off + i];
                    }
                } else {
                    for b in secbuf.iter_mut() {
                        *b = 0;
                    }
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
                self.fat_entry_write(newc, 0xFFFF)?;
                cur = newc;
            } else {
                cur = next;
            }
        }

        self.fat_entry_write(cur, 0xFFFF)?;
        Ok(())
    }

    fn alloc_free_cluster(&self) -> Result<Option<u16>, FatErr> {
        let max = (self.data_clusters + 2).min(0xFFEF);
        let mut clus = 2u16;
        while (clus as u32) < max {
            if self.fat_entry_read(clus)? == 0x0000 {
                self.fat_entry_write(clus, 0xFFF7)?;
                return Ok(Some(clus));
            }
            clus = clus.wrapping_add(1);
        }
        Ok(None)
    }

    fn name83(c: &str) -> Result<[u8; 11], FatErr> {
        if c.is_empty() {
            return Err(FatErr::Name);
        }
        let mut name = [b' '; 11];
        let mut base = String::new();
        let mut ext = String::new();

        let parts: Vec<&str> = c.split('.').collect();
        if parts.len() > 2 {
            return Err(FatErr::Name);
        }
        base.push_str(parts[0]);
        if parts.len() == 2 {
            ext.push_str(parts[1]);
        }

        if base.is_empty() || base.len() > 8 || ext.len() > 3 {
            return Err(FatErr::Name);
        }

        let upb = base.to_ascii_uppercase();
        let upe = ext.to_ascii_uppercase();
        for (i, ch) in upb.bytes().enumerate() {
            if ch <= 0x20 || ch >= 0x7F || ch == b'.' {
                return Err(FatErr::Name);
            }
            name[i] = ch;
        }

        for (i, ch) in upe.bytes().enumerate() {
            if ch <= 0x20 || ch >= 0x7F || ch == b'.' {
                return Err(FatErr::Name);
            }
            name[8 + i] = ch;
        }

        Ok(name)
    }

    fn dir_iter_root(&self) -> Result<Vec<DirEnt>, FatErr> {
        let mut out = Vec::new();
        let mut buf = [0u8; SEC];
        for s in 0..self.root_sectors {
            self.read_sector(self.root_start_sec + s, &mut buf)?;
            for i in 0..(SEC / 32) {
                let chunk = &buf[i * 32..i * 32 + 32];
                let first = chunk[0];
                if first == 0x00 {
                    return Ok(out);
                }

                if first == 0xE5 {
                    continue;
                }

                let ent: DirEnt =
                    unsafe { core::ptr::read_unaligned(chunk.as_ptr() as *const DirEnt) };
                if is_lfn(ent.attr) || (ent.attr & ATTR_VOLUME) != 0 {
                    continue;
                }

                out.push(ent);
            }
        }
        Ok(out)
    }

    fn dir_find_in_root(&self, name11: [u8; 11]) -> Result<Option<DirEnt>, FatErr> {
        let entries = self.dir_iter_root()?;
        for e in entries {
            if e.name == name11 {
                return Ok(Some(e));
            }
        }
        Ok(None)
    }

    fn root_write_entry(&self, ent: &DirEnt) -> Result<(), FatErr> {
        let mut buf = [0u8; SEC];
        for s in 0..self.root_sectors {
            self.read_sector(self.root_start_sec + s, &mut buf)?;
            for i in 0..(SEC / 32) {
                let p = &mut buf[i * 32..i * 32 + 32];
                let b0 = p[0];
                if b0 == 0x00 || b0 == 0xE5 {
                    unsafe {
                        let src = ent as *const DirEnt as *const u8;
                        for k in 0..32 {
                            p[k] = core::ptr::read(src.add(k));
                        }
                    }
                    self.write_sector(self.root_start_sec + s, &buf)?;
                    return Ok(());
                }
            }
        }
        Err(FatErr::Full)
    }

    fn dir_list_from(&self, start_clus: u16) -> Result<Vec<DirEnt>, FatErr> {
        let bytes = (self.data_clusters as usize).min(1 << 24) * 32;
        let raw = self.read_cluster_chain(start_clus, bytes)?;
        let mut out = Vec::new();
        for chunk in raw.chunks_exact(32) {
            let first = chunk[0];
            if first == 0x00 {
                break;
            }

            if first == 0xE5 {
                continue;
            }

            let ent: DirEnt = unsafe { core::ptr::read_unaligned(chunk.as_ptr() as *const DirEnt) };
            if is_lfn(ent.attr) || (ent.attr & ATTR_VOLUME) != 0 {
                continue;
            }

            out.push(ent);
        }
        Ok(out)
    }

    fn dir_find_in_dir(&self, dirclus: u16, name11: [u8; 11]) -> Result<Option<DirEnt>, FatErr> {
        for e in self.dir_list_from(dirclus)? {
            if e.name == name11 {
                return Ok(Some(e));
            }
        }
        Ok(None)
    }

    fn dir_write_entry(
        &self,
        dirclus: u16,
        ent: &DirEnt,
        match_name11: Option<[u8; 11]>,
    ) -> Result<(), FatErr> {
        if dirclus < 2 {
            return Err(FatErr::BadBpb);
        }
        let mut cur = dirclus;
        loop {
            let first_sec = self.clus_to_first_sector(cur);
            for s in 0..(self.spc as u32) {
                let secno = first_sec + s;
                let mut buf = [0u8; SEC];
                self.read_sector(secno, &mut buf)?;
                for i in 0..(SEC / 32) {
                    let p = &mut buf[i * 32..i * 32 + 32];
                    let first = p[0];
                    if let Some(name11) = match_name11 {
                        if first != 0xE5 && first != 0x00 {
                            let attr = p[11];
                            if !is_lfn(attr) && &p[0..11] == &name11 {
                                unsafe {
                                    let src = ent as *const DirEnt as *const u8;
                                    for k in 0..32 {
                                        p[k] = core::ptr::read(src.add(k));
                                    }
                                }
                                self.write_sector(secno, &buf)?;
                                return Ok(());
                            }
                        }
                    } else {
                        if first == 0x00 || first == 0xE5 {
                            unsafe {
                                let src = ent as *const DirEnt as *const u8;
                                for k in 0..32 {
                                    p[k] = core::ptr::read(src.add(k));
                                }
                            }
                            self.write_sector(secno, &buf)?;
                            return Ok(());
                        }
                    }
                }
            }

            let nxt = self.fat_entry_read(cur)?;
            if Self::is_eoc(nxt) {
                let newc = self.alloc_free_cluster()?.ok_or(FatErr::Full)?;
                self.fat_entry_write(cur, newc)?;
                self.fat_entry_write(newc, 0xFFFF)?;

                let zero = [0u8; SEC];
                let first_sec_new = self.clus_to_first_sector(newc);
                for s in 0..(self.spc as u32) {
                    self.write_sector(first_sec_new + s, &zero)?;
                }

                let mut buf = [0u8; SEC];
                self.read_sector(first_sec_new, &mut buf)?;
                let p = &mut buf[0..32];
                unsafe {
                    let src = ent as *const DirEnt as *const u8;
                    for k in 0..32 {
                        p[k] = core::ptr::read(src.add(k));
                    }
                }

                self.write_sector(first_sec_new, &buf)?;
                return Ok(());
            } else {
                cur = nxt;
            }
        }
    }

    fn split_path(p: &str) -> Vec<&str> {
        p.split('/').filter(|c| !c.is_empty()).collect()
    }

    fn resolve_dir(&self, path: &str) -> Result<Option<u16>, FatErr> {
        let comps = Self::split_path(path);
        if comps.is_empty() {
            return Ok(None);
        }

        let mut cur_dir: Option<u16> = None;
        for c in comps {
            let name11 = Self::name83(c)?;
            let ent = if let Some(dc) = cur_dir {
                self.dir_find_in_dir(dc, name11)?
            } else {
                self.dir_find_in_root(name11)?
            }
            .ok_or(FatErr::NotFound)?;
            if (ent.attr & ATTR_DIR) == 0 {
                return Err(FatErr::NotFound);
            }
            let cl = le16(ent.fst_clus_lo);
            if cl < 2 {
                return Err(FatErr::BadBpb);
            }
            cur_dir = Some(cl);
        }
        Ok(cur_dir)
    }

    pub fn read_file(&self, path: &str) -> Result<Vec<u8>, FatErr> {
        let comps = Self::split_path(path);
        if comps.is_empty() {
            return Err(FatErr::Name);
        }

        let mut cur_dir: Option<u16> = None;
        for (i, c) in comps.iter().enumerate() {
            let name11 = Self::name83(c)?;
            if i + 1 < comps.len() {
                let ent = if let Some(dc) = cur_dir {
                    self.dir_find_in_dir(dc, name11)?
                } else {
                    self.dir_find_in_root(name11)?
                }
                .ok_or(FatErr::NotFound)?;
                if (ent.attr & ATTR_DIR) == 0 {
                    return Err(FatErr::NotFound);
                }
                let cl = le16(ent.fst_clus_lo);
                if cl < 2 {
                    return Err(FatErr::BadBpb);
                }
                cur_dir = Some(cl);
            } else {
                let ent = if let Some(dc) = cur_dir {
                    self.dir_find_in_dir(dc, name11)?
                } else {
                    self.dir_find_in_root(name11)?
                }
                .ok_or(FatErr::NotFound)?;
                if (ent.attr & ATTR_DIR) != 0 {
                    return Err(FatErr::NotFound);
                }
                let size = le32(ent.file_size) as usize;
                let start = le16(ent.fst_clus_lo);
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

    pub fn mkdir_root(&self, name_83: &str) -> Result<(), FatErr> {
        self.mkdir_at(None, name_83)
    }

    pub fn mkdir_at(&self, parent_dir: Option<u16>, name_83: &str) -> Result<(), FatErr> {
        let name = Self::name83(name_83)?;
        if let Some(dc) = parent_dir {
            if self.dir_find_in_dir(dc, name)?.is_some() {
                return Ok(());
            }
        } else if self.dir_find_in_root(name)?.is_some() {
            return Ok(());
        }

        let cl = self.alloc_free_cluster()?.ok_or(FatErr::Full)?;
        self.fat_entry_write(cl, 0xFFFF)?;

        let zero = [0u8; SEC];
        let first_sec = self.clus_to_first_sector(cl);
        for s in 0..(self.spc as u32) {
            self.write_sector(first_sec + s, &zero)?;
        }

        let mut dot_name = [b' '; 11];
        dot_name[0] = b'.';
        let mut dotdot_name = [b' '; 11];
        dotdot_name[0] = b'.';
        dotdot_name[1] = b'.';

        let dot = DirEnt {
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
            fst_clus_lo: cl.to_le(),
            file_size: 0,
        };
        let parent_cl = parent_dir.unwrap_or(0);
        let dotdot = DirEnt {
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
            fst_clus_lo: parent_cl.to_le(),
            file_size: 0,
        };

        let mut sec0 = [0u8; SEC];
        self.read_sector(first_sec, &mut sec0)?;
        unsafe {
            let src0 = &dot as *const DirEnt as *const u8;
            for k in 0..32 {
                sec0[k] = core::ptr::read(src0.add(k));
            }

            let src1 = &dotdot as *const DirEnt as *const u8;
            for k in 0..32 {
                sec0[32 + k] = core::ptr::read(src1.add(k));
            }
        }
        self.write_sector(first_sec, &sec0)?;

        let ent = DirEnt {
            name,
            attr: ATTR_DIR,
            ntres: 0,
            crt_time_tenth: 0,
            crt_time: 0,
            crt_date: 0,
            lst_acc_date: 0,
            fst_clus_hi: 0,
            wrt_time: 0,
            wrt_date: 0,
            fst_clus_lo: cl.to_le(),
            file_size: 0,
        };

        if let Some(dc) = parent_dir {
            self.dir_write_entry(dc, &ent, None)
        } else {
            self.root_write_entry(&ent)
        }
    }

    pub fn mkdir(&self, path: &str) -> Result<(), FatErr> {
        let comps = Self::split_path(path);
        if comps.is_empty() {
            return Ok(());
        }

        let mut cur_dir: Option<u16> = None;
        for c in comps {
            let name11 = Self::name83(c)?;
            let found = if let Some(dc) = cur_dir {
                self.dir_find_in_dir(dc, name11)?
            } else {
                self.dir_find_in_root(name11)?
            };
            if let Some(ent) = found {
                if (ent.attr & ATTR_DIR) == 0 {
                    return Err(FatErr::Name);
                }
                let cl = le16(ent.fst_clus_lo);
                if cl < 2 {
                    return Err(FatErr::BadBpb);
                }
                cur_dir = Some(cl);
            } else {
                self.mkdir_at(cur_dir, c)?;
                let ent = if let Some(dc) = cur_dir {
                    self.dir_find_in_dir(dc, Self::name83(c)?)?
                } else {
                    self.dir_find_in_root(Self::name83(c)?)?
                }
                .ok_or(FatErr::NotFound)?;
                cur_dir = Some(le16(ent.fst_clus_lo));
            }
        }
        Ok(())
    }

    pub fn write_file_at(
        &self,
        parent_dir: Option<u16>,
        name_83: &str,
        data: &[u8],
    ) -> Result<(), FatErr> {
        let name = Self::name83(name_83)?;
        let maybe = if let Some(dc) = parent_dir {
            self.dir_find_in_dir(dc, name)?
        } else {
            self.dir_find_in_root(name)?
        };

        let start_clus = if let Some(ent) = maybe {
            if (ent.attr & ATTR_DIR) != 0 {
                return Err(FatErr::Name);
            }
            le16(ent.fst_clus_lo)
        } else {
            let c = self.alloc_free_cluster()?.ok_or(FatErr::Full)?;
            self.fat_entry_write(c, 0xFFFF)?;
            c
        };

        self.write_cluster_chain_from(start_clus, data)?;

        let ent = DirEnt {
            name,
            attr: ATTR_ARCHIVE,
            ntres: 0,
            crt_time_tenth: 0,
            crt_time: 0,
            crt_date: 0,
            lst_acc_date: 0,
            fst_clus_hi: 0,
            wrt_time: 0,
            wrt_date: 0,
            fst_clus_lo: start_clus.to_le(),
            file_size: (data.len() as u32).to_le(),
        };

        if let Some(dc) = parent_dir {
            if maybe.is_some() {
                self.dir_write_entry(dc, &ent, Some(name))
            } else {
                self.dir_write_entry(dc, &ent, None)
            }
        } else {
            if maybe.is_some() {
                let mut buf = [0u8; SEC];
                for s in 0..self.root_sectors {
                    self.read_sector(self.root_start_sec + s, &mut buf)?;
                    for i in 0..(SEC / 32) {
                        if &buf[i * 32..i * 32 + 11] == &name {
                            unsafe {
                                let src = &ent as *const DirEnt as *const u8;
                                for k in 0..32 {
                                    buf[i * 32 + k] = core::ptr::read(src.add(k));
                                }
                            }
                            self.write_sector(self.root_start_sec + s, &buf)?;
                            return Ok(());
                        }
                    }
                }
                self.root_write_entry(&ent)
            } else {
                self.root_write_entry(&ent)
            }
        }
    }

    pub fn write_file(&self, path: &str, data: &[u8]) -> Result<(), FatErr> {
        let comps = Self::split_path(path);
        if comps.is_empty() {
            return Err(FatErr::Name);
        }

        let (parent, fname) = comps.split_at(comps.len() - 1);
        let parent_dir = if parent.is_empty() {
            None
        } else {
            let p = parent.join("/");
            self.resolve_dir(&p)?
        };
        self.write_file_at(parent_dir, fname[0], data)
    }

    pub fn list_root(&self) -> Result<Vec<String>, FatErr> {
        self.list_dir("/")
    }

    pub fn list_dir(&self, path: &str) -> Result<Vec<String>, FatErr> {
        if path == "/" || path.is_empty() {
            let ents = self.dir_iter_root()?;
            return Ok(Self::names_from_dirents(ents));
        }
        let dirclus = self.resolve_dir(path)?.ok_or(FatErr::BadBpb)?;
        let ents = self.dir_list_from(dirclus)?;
        Ok(Self::names_from_dirents(ents))
    }

    fn names_from_dirents(ents: Vec<DirEnt>) -> Vec<String> {
        let mut out = Vec::new();
        for e in ents {
            let base = core::str::from_utf8(&e.name[..8]).unwrap_or("").trim_end();
            let ext = core::str::from_utf8(&e.name[8..11])
                .unwrap_or("")
                .trim_end();
            let mut name = String::new();
            name.push_str(base);
            if (e.attr & ATTR_DIR) == 0 && !ext.is_empty() {
                name.push('.');
                name.push_str(ext);
            }
            if (e.attr & ATTR_DIR) != 0 {
                name.push('/');
            }
            out.push(name);
        }
        out
    }

    pub fn write_file_root(&self, name_83: &str, data: &[u8]) -> Result<(), FatErr> {
        self.write_file_at(None, name_83, data)
    }
}
