use crate::platform::limine::hhdm::HHDM_REQ;
use x86_64::{
    registers::control::{Cr3, Cr3Flags},
    structures::paging::{PageTable, PhysFrame, Size4KiB},
    VirtAddr,
};

#[derive(Copy, Clone)]
pub struct AddressSpace {
    pub root: PhysFrame<Size4KiB>,
    pub flags: Cr3Flags,
}

impl AddressSpace {
    #[allow(unused)]
    pub fn from_current() -> Self {
        let (frame, flags) = Cr3::read();
        AddressSpace { root: frame, flags }
    }

    #[allow(unused)]
    pub unsafe fn activate(&self) {
        Cr3::write(self.root, self.flags);
        crate::kernel::mm::map::mapper::reset_for_current_cr3();
    }
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
