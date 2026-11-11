use crate::platform::limine::hhdm::HHDM_REQ;
use alloc::collections::BTreeSet;
use x86_64::PhysAddr;
use x86_64::structures::paging::PageTableFlags;
use crate::kernel::mm::phys::frame;

use spin::Once;
use x86_64::registers::control::{Cr3, Cr3Flags};
use x86_64::structures::paging::PageTable;
use x86_64::structures::paging::{PhysFrame, Size4KiB};
use x86_64::VirtAddr;

#[derive(Copy, Clone)]
pub struct AddressSpace {
    pub root: PhysFrame<Size4KiB>,
    pub flags: Cr3Flags,
}

impl AddressSpace {
    pub fn from_current() -> Self {
        let (frame, flags) = Cr3::read();
        AddressSpace { root: frame, flags }
    }

    pub unsafe fn activate(&self) {
        Cr3::write(self.root, self.flags);
        crate::kernel::mm::map::mapper::reset_for_current_cr3();
    }
}

pub static KERNEL_ASPACE: Once<AddressSpace> = Once::new();

pub fn init_kernel_aspace_snapshot() {
    KERNEL_ASPACE.call_once(AddressSpace::from_current);
}

pub fn new_user_address_space() -> AddressSpace {
    let phys_off = VirtAddr::new(HHDM_REQ.get_response().unwrap().offset());

    let (cur_root, flags) = Cr3::read();
    let cur_l4_virt = phys_off + cur_root.start_address().as_u64();
    let cur_l4: &PageTable = unsafe { &*cur_l4_virt.as_ptr() };

    let new_root = crate::kernel::mm::phys::frame::alloc().expect("alloc PML4");
    let new_virt = phys_off + new_root.start_address().as_u64();
    unsafe {
        core::ptr::write_bytes(new_virt.as_mut_ptr::<u8>(), 0, 4096);
    }
    let new_l4: &mut PageTable = unsafe { &mut *new_virt.as_mut_ptr() };

    for i in 256..512 {
        new_l4[i] = cur_l4[i].clone();
    }

    AddressSpace {
        root: new_root,
        flags,
    }
}

/// Progress of an off-CPU address-space teardown.  Indices always refer to
/// the next entry to visit, allowing cleanup to be spread over many syscalls.
#[derive(Default)]
pub struct ReclaimCursor {
    pub p4: usize,
    pub p3: usize,
    pub p2: usize,
    pub p1: usize,
    pub pages_freed: u64,
    pub finished: bool,
}

/// Free at most `budget` mapped 4-KiB leaves in one invocation.
///
/// NEVER call on the active CR3; the caller must have marked the process
/// zombie and removed it from scheduling.  Keeping the cursor after a partial
/// step is mandatory, since the processed table entries are cleared in place.
///
/// Shared-memory frames are owned by SHM, not the process. The framebuffer
/// is MMIO mapped at user PML4 index 0xc0 and must never be freed as RAM.
pub fn reclaim_inactive_step(
    space: &AddressSpace,
    shared: &BTreeSet<u64>,
    cursor: &mut ReclaimCursor,
    budget: usize,
) -> Result<bool, ()> {
    if cursor.finished { return Ok(true); }
    if budget == 0 { return Ok(false); }
    let (active, _) = Cr3::read();
    if active == space.root { return Err(()); }

    let off = HHDM_REQ.get_response().ok_or(())?.offset();
    let root_pa = space.root.start_address().as_u64();
    let mut remaining = budget;
    unsafe {
        let p4_table = &mut *((off + root_pa) as *mut PageTable);
        while cursor.p4 < 256 {
            let p4_entry = &mut p4_table[cursor.p4];
            if !p4_entry.flags().contains(PageTableFlags::PRESENT)
                || p4_entry.flags().contains(PageTableFlags::HUGE_PAGE) {
                cursor.p4 += 1;
                cursor.p3 = 0;
                continue;
            }
            let p3_pa = p4_entry.addr().as_u64();
            let p3_table = &mut *((off + p3_pa) as *mut PageTable);
            while cursor.p3 < 512 {
                let p3_entry = &mut p3_table[cursor.p3];
                if !p3_entry.flags().contains(PageTableFlags::PRESENT)
                    || p3_entry.flags().contains(PageTableFlags::HUGE_PAGE) {
                    cursor.p3 += 1;
                    cursor.p2 = 0;
                    continue;
                }
                let p2_pa = p3_entry.addr().as_u64();
                let p2_table = &mut *((off + p2_pa) as *mut PageTable);
                while cursor.p2 < 512 {
                    let p2_entry = &mut p2_table[cursor.p2];
                    if !p2_entry.flags().contains(PageTableFlags::PRESENT)
                        || p2_entry.flags().contains(PageTableFlags::HUGE_PAGE) {
                        cursor.p2 += 1;
                        cursor.p1 = 0;
                        continue;
                    }
                    let p1_pa = p2_entry.addr().as_u64();
                    let p1_table = &mut *((off + p1_pa) as *mut PageTable);
                    while cursor.p1 < 512 {
                        let leaf = &mut p1_table[cursor.p1];
                        let flags = leaf.flags();
                        if flags.contains(PageTableFlags::PRESENT) {
                            let pa = leaf.addr().as_u64();
                            if cursor.p4 != 0xc0
                                && flags.contains(PageTableFlags::USER_ACCESSIBLE)
                                && !shared.contains(&pa) {
                                frame::free(PhysFrame::containing_address(PhysAddr::new(pa)));
                            }
                            leaf.set_unused();
                            cursor.pages_freed += 1;
                            remaining -= 1;
                        }
                        cursor.p1 += 1;
                        if remaining == 0 { return Ok(false); }
                    }
                    frame::free(PhysFrame::containing_address(PhysAddr::new(p1_pa)));
                    p2_entry.set_unused();
                    cursor.p2 += 1;
                    cursor.p1 = 0;
                }
                frame::free(PhysFrame::containing_address(PhysAddr::new(p2_pa)));
                p3_entry.set_unused();
                cursor.p3 += 1;
                cursor.p2 = 0;
            }
            frame::free(PhysFrame::containing_address(PhysAddr::new(p3_pa)));
            p4_entry.set_unused();
            cursor.p4 += 1;
            cursor.p3 = 0;
        }
    }
    frame::free(space.root);
    cursor.finished = true;
    Ok(true)
}
