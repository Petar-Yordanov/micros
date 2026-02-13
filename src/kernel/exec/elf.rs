extern crate alloc;

use alloc::vec::Vec;

use x86_64::{
    structures::paging::{
        page_table::PageTableEntry,
        FrameAllocator, PageTable, PageTableFlags, PhysFrame, Size4KiB,
    },
    PhysAddr, VirtAddr,
};

use crate::kernel::mm::aspace::address_space::AddressSpace;
use crate::kernel::mm::phys::frame;
use crate::platform::limine::hhdm::HHDM_REQ;
use crate::sprintln;

#[derive(Debug)]
pub enum ElfError {
    BadMagic,
    Not64,
    BadPhdr,
    Unsupported,
    NoMem,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Elf64Ehdr {
    e_ident: [u8; 16],
    e_type: u16,
    e_machine: u16,
    e_version: u32,
    e_entry: u64,
    e_phoff: u64,
    e_shoff: u64,
    e_flags: u32,
    e_ehsize: u16,
    e_phentsize: u16,
    e_phnum: u16,
    e_shentsize: u16,
    e_shnum: u16,
    e_shstrndx: u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Elf64Phdr {
    p_type: u32,
    p_flags: u32,
    p_offset: u64,
    p_vaddr: u64,
    p_paddr: u64,
    p_filesz: u64,
    p_memsz: u64,
    p_align: u64,
}

const PT_LOAD: u32 = 1;

const PF_X: u32 = 1;
const PF_W: u32 = 2;
const PF_R: u32 = 4;

fn phys_off() -> VirtAddr {
    VirtAddr::new(HHDM_REQ.get_response().unwrap().offset())
}

fn read_ehdr(img: &[u8]) -> Result<Elf64Ehdr, ElfError> {
    if img.len() < core::mem::size_of::<Elf64Ehdr>() {
        return Err(ElfError::BadMagic);
    }
    let eh = unsafe { *(img.as_ptr() as *const Elf64Ehdr) };
    if &eh.e_ident[0..4] != b"\x7fELF" {
        return Err(ElfError::BadMagic);
    }

    if eh.e_ident[4] != 2 {
        return Err(ElfError::Not64);
    }
    Ok(eh)
}

fn phdr_at(img: &[u8], off: usize) -> Result<Elf64Phdr, ElfError> {
    if off + core::mem::size_of::<Elf64Phdr>() > img.len() {
        return Err(ElfError::BadPhdr);
    }
    Ok(unsafe { *(img.as_ptr().add(off) as *const Elf64Phdr) })
}

fn map_page(aspace: &AddressSpace, va: u64, frame: PhysFrame<Size4KiB>, flags: PageTableFlags) -> Result<(), ElfError> {
    let off = phys_off();

    let pml4_pa = aspace.root.start_address().as_u64();
    let pml4_va = off + pml4_pa;
    let pml4: &mut PageTable = unsafe { &mut *pml4_va.as_mut_ptr() };

    let v = VirtAddr::new(va);

    let p4i = v.p4_index();
    let p3i = v.p3_index();
    let p2i = v.p2_index();
    let p1i = v.p1_index();

    fn ensure_table<'a>(
        off: VirtAddr,
        ent: &mut PageTableEntry,
    ) -> Result<&'a mut PageTable, ElfError> {
        if ent.is_unused() {
            let new = frame::alloc().ok_or(ElfError::NoMem)?;
            let new_pa = new.start_address().as_u64();
            let new_va = off + new_pa;
            unsafe { core::ptr::write_bytes(new_va.as_mut_ptr::<u8>(), 0, 4096) };

            let mut e = PageTableEntry::new();
            e.set_addr(
                PhysAddr::new(new_pa),
                PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE,
            );
            *ent = e;
        }
        let pa = ent.addr().as_u64();
        let va = off + pa;
        Ok(unsafe { &mut *va.as_mut_ptr::<PageTable>() })
    }

    let p3 = ensure_table(off, &mut pml4[p4i])?;
    let p2 = ensure_table(off, &mut p3[p3i])?;
    let p1 = ensure_table(off, &mut p2[p2i])?;

    let mut e = PageTableEntry::new();
    e.set_addr(frame.start_address(), flags | PageTableFlags::PRESENT);
    p1[p1i] = e;

    Ok(())
}

pub fn map_user_zero(aspace: &AddressSpace, va: u64, len: u64, writable: bool) -> Result<(), ElfError> {
    let off = phys_off();
    let start = va & !0xfffu64;
    let end = (va + len + 0xfff) & !0xfffu64;

    let mut flags = PageTableFlags::USER_ACCESSIBLE;
    if writable {
        flags |= PageTableFlags::WRITABLE;
    } else {
        // If not writable, allow execute by default
    }

    for cur in (start..end).step_by(4096) {
        let fr = frame::alloc().ok_or(ElfError::NoMem)?;
        let pa = fr.start_address().as_u64();
        let kva = off + pa;
        unsafe { core::ptr::write_bytes(kva.as_mut_ptr::<u8>(), 0, 4096) };

        map_page(aspace, cur, fr, flags)?;
    }

    Ok(())
}

pub fn load_elf64_user(aspace: &AddressSpace, img: &[u8]) -> Result<u64, ElfError> {
    let eh = read_ehdr(img)?;

    let phoff = eh.e_phoff as usize;
    let entsz = eh.e_phentsize as usize;
    let phnum = eh.e_phnum as usize;

    if entsz < core::mem::size_of::<Elf64Phdr>() {
        return Err(ElfError::BadPhdr);
    }

    for i in 0..phnum {
        let off = phoff + i * entsz;
        let ph = phdr_at(img, off)?;

        if ph.p_type != PT_LOAD {
            continue;
        }

        if ph.p_memsz == 0 {
            continue;
        }

        let seg_va = ph.p_vaddr;
        let seg_filesz = ph.p_filesz;
        let seg_memsz = ph.p_memsz;

        let seg_start = seg_va & !0xfffu64;
        let seg_end = (seg_va + seg_memsz + 0xfff) & !0xfffu64;

        let is_w = (ph.p_flags & PF_W) != 0;
        let is_x = (ph.p_flags & PF_X) != 0;
        let _is_r = (ph.p_flags & PF_R) != 0;

        let mut flags = PageTableFlags::USER_ACCESSIBLE;
        if is_w {
            flags |= PageTableFlags::WRITABLE;
        }
        if !is_x {
            flags |= PageTableFlags::NO_EXECUTE;
        }

        for cur in (seg_start..seg_end).step_by(4096) {
            let fr = frame::alloc().ok_or(ElfError::NoMem)?;
            map_page(aspace, cur, fr, flags)?;

            let pa = fr.start_address().as_u64();
            let kva = phys_off() + pa;
            unsafe { core::ptr::write_bytes(kva.as_mut_ptr::<u8>(), 0, 4096) };
        }

        let file_off = ph.p_offset as usize;
        let file_end = file_off.checked_add(seg_filesz as usize).ok_or(ElfError::BadPhdr)?;
        if file_end > img.len() {
            return Err(ElfError::BadPhdr);
        }

        copy_into_user(aspace, seg_va, &img[file_off..file_end])?;

        sprintln!(
            "[exec] PT_LOAD vaddr={:#x} filesz={:#x} memsz={:#x} flags={:#x}",
            seg_va, seg_filesz, seg_memsz, ph.p_flags
        );
    }

    Ok(eh.e_entry)
}

fn va_to_pa(aspace: &AddressSpace, va: u64) -> Option<u64> {
    let off = phys_off();
    let pml4_pa = aspace.root.start_address().as_u64();
    let pml4: &PageTable = unsafe { &*((off + pml4_pa).as_ptr()) };

    let v = VirtAddr::new(va);
    let p4e = pml4[v.p4_index()].frame().ok()?;
    let p3: &PageTable = unsafe { &* ( (off + p4e.start_address().as_u64()).as_ptr() ) };

    let p3e = p3[v.p3_index()].frame().ok()?;
    let p2: &PageTable = unsafe { &* ( (off + p3e.start_address().as_u64()).as_ptr() ) };

    let p2e = p2[v.p2_index()].frame().ok()?;
    let p1: &PageTable = unsafe { &* ( (off + p2e.start_address().as_u64()).as_ptr() ) };

    let pte = &p1[v.p1_index()];
    let fr = pte.frame().ok()?;
    Some(fr.start_address().as_u64() + (va & 0xfff))
}

fn copy_into_user(aspace: &AddressSpace, dst_va: u64, src: &[u8]) -> Result<(), ElfError> {
    let mut off = 0usize;
    while off < src.len() {
        let va = dst_va + off as u64;
        let pa = va_to_pa(aspace, va).ok_or(ElfError::BadPhdr)?;
        let kptr = (phys_off() + pa).as_mut_ptr::<u8>();
        unsafe {
            *kptr = src[off];
        }
        off += 1;
    }
    Ok(())
}
