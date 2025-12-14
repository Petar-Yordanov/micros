use crate::kernel::mm::map::mapper::{self, MapErr, Prot};
use x86_64::structures::paging::{Page, PhysFrame, Size4KiB};
use x86_64::{PhysAddr, VirtAddr};

#[allow(unused)]
pub fn map_mmio(va: VirtAddr, pa: PhysAddr, bytes: u64) -> Result<(), MapErr> {
    let start_va = (va.as_u64() & !0xfff) as u64;
    let end_va = (va.as_u64() + bytes + 0xfff) & !0xfff;

    let mut v = start_va;
    let mut a = pa.as_u64() & !0xfff;

    while v < end_va {
        let _page = Page::<Size4KiB>::containing_address(VirtAddr::new(v));
        let frame = PhysFrame::<Size4KiB>::containing_address(PhysAddr::new(a));
        mapper::map_fixed(VirtAddr::new(v), frame, Prot::MMIO)?;
        v += 0x1000;
        a += 0x1000;
    }
    Ok(())
}
