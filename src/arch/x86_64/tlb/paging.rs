use crate::platform::limine::hhdm::HHDM_REQ;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use spin::Mutex;
use x86_64::{
    registers::control::{Cr3, Cr3Flags},
    structures::paging::{
        mapper::MapToError, FrameAllocator, Mapper, OffsetPageTable, Page, PageTable,
        PageTableFlags as F, PhysFrame, Size4KiB, Translate,
    },
    PhysAddr, VirtAddr,
};

#[derive(Debug)]
pub enum MapErr {
    NoFrames,
    Map(MapToError<Size4KiB>),
}

#[derive(Debug)]
pub enum UnmapErr {
    NotMapped,
    Other,
}

pub enum ProtErr {
    NotMapped,
}

#[derive(Copy, Clone)]
pub struct AddressSpace {
    pub root: PhysFrame,
    pub flags: Cr3Flags,
}

impl AddressSpace {
    pub fn from_current() -> Self {
        let (frame, flags) = Cr3::read();
        AddressSpace { root: frame, flags }
    }

    pub unsafe fn activate(&self) {
        Cr3::write(self.root, self.flags);

        let phys_off = VirtAddr::new(HHDM_REQ.get_response().unwrap().offset());
        let virt = phys_off + self.root.start_address().as_u64();
        let l4: &mut PageTable = &mut *virt.as_mut_ptr();

        *MAPPER.lock() = Some(OffsetPageTable::new(l4, phys_off));
    }
}

#[derive(Copy, Clone)]
pub enum Prot {
    RW,
    RO,
    RX,
    RWX,
    MMIO,
    UserRW,
    UserRX,
}

impl Prot {
    fn flags(self) -> F {
        match self {
            Prot::RW => F::PRESENT | F::WRITABLE | F::NO_EXECUTE,
            Prot::RO => F::PRESENT | F::NO_EXECUTE,
            Prot::RX => F::PRESENT,
            Prot::RWX => F::PRESENT | F::WRITABLE,
            Prot::MMIO => F::PRESENT | F::WRITABLE | F::NO_EXECUTE | F::WRITE_THROUGH | F::NO_CACHE,
            Prot::UserRW => F::PRESENT | F::WRITABLE | F::NO_EXECUTE | F::USER_ACCESSIBLE,
            Prot::UserRX => F::PRESENT | F::USER_ACCESSIBLE,
        }
    }
}

static INITIALIZED: AtomicBool = AtomicBool::new(false);
static MAPPER: Mutex<Option<OffsetPageTable<'static>>> = Mutex::new(None);

pub fn init() {
    if INITIALIZED.swap(true, Ordering::SeqCst) {
        return;
    }
    let phys_off = VirtAddr::new(HHDM_REQ.get_response().expect("no HHDM").offset());
    let l4 = unsafe { active_l4(phys_off) };
    let mapper = unsafe { OffsetPageTable::new(l4, phys_off) };
    *MAPPER.lock() = Some(mapper);
}

pub fn translate(va: VirtAddr) -> Option<PhysAddr> {
    let guard = MAPPER.lock();
    let mapper = guard.as_ref().expect("paging::init() not called");
    mapper.translate_addr(va)
}

pub fn map_fixed(va: VirtAddr, frame: PhysFrame, prot: Prot) -> Result<(), MapErr> {
    let mut guard = MAPPER.lock();
    let mapper = guard.as_mut().expect("paging::init() not called");

    let page = Page::<Size4KiB>::containing_address(align4k(va));
    let mut tbl_alloc = TableAlloc;

    let flush =
        unsafe { mapper.map_to(page, frame, prot.flags(), &mut tbl_alloc) }.map_err(MapErr::Map)?;
    flush.flush();
    Ok(())
}

pub fn unmap(va: VirtAddr) -> Result<PhysFrame, UnmapErr> {
    let mut guard = MAPPER.lock();
    let mapper = guard.as_mut().expect("paging::init() not called");

    let page = Page::<Size4KiB>::containing_address(align4k(va));
    match mapper.unmap(page) {
        Ok((frame, flush)) => {
            flush.flush();
            Ok(frame)
        }
        Err(x86_64::structures::paging::mapper::UnmapError::PageNotMapped) => {
            Err(UnmapErr::NotMapped)
        }
        Err(_) => Err(UnmapErr::Other),
    }
}

pub fn protect(va: VirtAddr, prot: Prot) -> Result<(), ProtErr> {
    let mut guard = MAPPER.lock();
    let mapper = guard.as_mut().expect("paging::init() not called");

    let page = Page::<Size4KiB>::containing_address(align4k(va));
    let flags = prot.flags();

    match mapper.translate_page(page) {
        Ok(frame) => {
            let (_old_frame, old_flush) = mapper.unmap(page).map_err(|_| ProtErr::NotMapped)?;
            old_flush.flush();

            let mut tbl_alloc = TableAlloc;
            let new_flush = unsafe { mapper.map_to(page, frame, flags, &mut tbl_alloc) }
                .map_err(|_| ProtErr::NotMapped)?;
            new_flush.flush();
            Ok(())
        }
        Err(_) => Err(ProtErr::NotMapped),
    }
}

pub fn map_mmio(va: VirtAddr, pa: PhysAddr, bytes: u64) -> Result<(), MapErr> {
    let mut guard = MAPPER.lock();
    let mapper = guard.as_mut().expect("paging::init() not called");

    let start_va = align4k(va).as_u64();
    let end_va = (va.as_u64() + bytes + 0xfff) & !0xfff;
    let mut v = start_va;
    let mut a = pa.as_u64() & !0xfff;

    while v < end_va {
        let page = Page::<Size4KiB>::containing_address(VirtAddr::new(v));
        let frame = PhysFrame::<Size4KiB>::containing_address(PhysAddr::new(a));
        let mut tbl_alloc = TableAlloc;

        let flush = unsafe { mapper.map_to(page, frame, Prot::MMIO.flags(), &mut tbl_alloc) }
            .map_err(MapErr::Map)?;
        flush.flush();

        v += 0x1000;
        a += 0x1000;
    }
    Ok(())
}

struct TableAlloc;

unsafe impl FrameAllocator<Size4KiB> for TableAlloc {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        let f = crate::kernel::mm::phys::frame::alloc()?;

        let hhdm = crate::platform::limine::hhdm::HHDM_REQ
            .get_response()
            .unwrap()
            .offset();
        let pa = f.start_address().as_u64();
        let va = x86_64::VirtAddr::new(hhdm + pa);
        unsafe {
            core::ptr::write_bytes(va.as_mut_ptr::<u8>(), 0, 4096);
        }

        Some(f)
    }
}

#[inline]
fn align4k(va: VirtAddr) -> VirtAddr {
    VirtAddr::new(va.as_u64() & !0xfff)
}

unsafe fn active_l4(phys_off: VirtAddr) -> &'static mut PageTable {
    let (frame, _flags): (PhysFrame, Cr3Flags) = Cr3::read();
    let phys = frame.start_address().as_u64();
    let virt = phys_off + phys;
    &mut *virt.as_mut_ptr()
}

static SCRATCH_VA: AtomicU64 = AtomicU64::new(0);

pub fn init_scratch_va() {
    let va = crate::kernel::mm::virt::vmarena::alloc().expect("scratch va");
    SCRATCH_VA.store(va.as_u64(), Ordering::SeqCst);
}

fn zero_phys_via_scratch(f: PhysFrame<Size4KiB>) {
    let va = VirtAddr::new(SCRATCH_VA.load(Ordering::SeqCst));
    map_fixed(va, f, Prot::RW).unwrap();
    unsafe {
        core::ptr::write_bytes(va.as_mut_ptr::<u8>(), 0, 4096);
    }
    let _ = unmap(va).unwrap();
}
