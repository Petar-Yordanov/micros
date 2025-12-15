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
#[allow(dead_code)]
pub enum MapErr {
    NoFrames,
    Map(MapToError<Size4KiB>),
}

#[derive(Debug)]
pub enum UnmapErr {
    NotMapped,
    Other,
}

#[allow(unused)]
pub enum ProtErr {
    NotMapped,
}

#[derive(Copy, Clone)]
#[allow(dead_code)]
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
    unsafe { reset_for_current_cr3() };
}

pub unsafe fn reset_for_current_cr3() {
    let phys_off = VirtAddr::new(HHDM_REQ.get_response().expect("no HHDM").offset());
    let l4 = active_l4(phys_off);
    let mapper = OffsetPageTable::new(l4, phys_off);
    *MAPPER.lock() = Some(mapper);
}

pub fn translate(va: VirtAddr) -> Option<PhysAddr> {
    let guard = MAPPER.lock();
    let mapper = guard.as_ref().expect("mapper::init() not called");
    mapper.translate_addr(va)
}

pub fn map_fixed(va: VirtAddr, frame: PhysFrame<Size4KiB>, prot: Prot) -> Result<(), MapErr> {
    let mut guard = MAPPER.lock();
    let mapper = guard.as_mut().expect("mapper::init() not called");

    let page = Page::<Size4KiB>::containing_address(align4k(va));
    let mut tbl_alloc = TableAlloc;
    let flush =
        unsafe { mapper.map_to(page, frame, prot.flags(), &mut tbl_alloc) }.map_err(MapErr::Map)?;
    flush.flush();
    Ok(())
}

pub fn unmap(va: VirtAddr) -> Result<PhysFrame<Size4KiB>, UnmapErr> {
    let mut guard = MAPPER.lock();
    let mapper = guard.as_mut().expect("mapper::init() not called");

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

#[allow(unused)]
pub fn protect(va: VirtAddr, prot: Prot) -> Result<(), ProtErr> {
    let mut guard = MAPPER.lock();
    let mapper = guard.as_mut().expect("mapper::init() not called");

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

struct TableAlloc;

unsafe impl FrameAllocator<Size4KiB> for TableAlloc {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        let f = crate::kernel::mm::phys::frame::alloc()?;

        let hhdm = HHDM_REQ.get_response().unwrap().offset();
        let pa = f.start_address().as_u64();
        let va = VirtAddr::new(hhdm + pa);
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

#[allow(dead_code)]
pub fn zero_phys_via_scratch(f: PhysFrame<Size4KiB>) {
    let va = VirtAddr::new(SCRATCH_VA.load(Ordering::SeqCst));
    map_fixed(va, f, Prot::RW).unwrap();
    unsafe {
        core::ptr::write_bytes(va.as_mut_ptr::<u8>(), 0, 4096);
    }
    let _ = unmap(va).unwrap();
}
