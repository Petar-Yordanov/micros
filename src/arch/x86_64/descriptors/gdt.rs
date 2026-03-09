#![allow(dead_code)]

use spin::Once;
use x86_64::{
    instructions::{
        segmentation::{Segment, CS, DS, ES, SS},
        tables::load_tss,
    },
    structures::gdt::{Descriptor, DescriptorFlags, GlobalDescriptorTable, SegmentSelector},
};

use crate::arch::x86_64::descriptors::tss;

pub struct Selectors {
    pub kcode: SegmentSelector,
    pub kdata: SegmentSelector,
    pub ucode: SegmentSelector,
    pub udata: SegmentSelector,
    pub tss: SegmentSelector,
}

static GDT: Once<(GlobalDescriptorTable, Selectors)> = Once::new();

pub unsafe fn init(kernel_rsp0_top: u64) {
    tss::configure_rsp0_and_ists(kernel_rsp0_top);

    let tss_ref = tss::tss_ref();

    let mut tmp = GlobalDescriptorTable::new();

    let kcode = tmp.append(Descriptor::kernel_code_segment());
    let kdata = tmp.append(Descriptor::kernel_data_segment());

    let ucode_flags = DescriptorFlags::USER_SEGMENT
        | DescriptorFlags::PRESENT
        | DescriptorFlags::EXECUTABLE
        | DescriptorFlags::LONG_MODE
        | DescriptorFlags::DPL_RING_3;
    let ucode = tmp.append(Descriptor::UserSegment(ucode_flags.bits()));

    let udata_flags = DescriptorFlags::USER_SEGMENT
        | DescriptorFlags::PRESENT
        | DescriptorFlags::WRITABLE
        | DescriptorFlags::DPL_RING_3;
    let udata = tmp.append(Descriptor::UserSegment(udata_flags.bits()));

    let tss_sel = tmp.append(Descriptor::tss_segment(tss_ref));

    GDT.call_once(|| {
        (
            tmp,
            Selectors {
                kcode,
                kdata,
                ucode,
                udata,
                tss: tss_sel,
            },
        )
    });

    let (gdt, sels) = GDT.get().unwrap();

    gdt.load_unsafe();
    CS::set_reg(sels.kcode);
    DS::set_reg(sels.kdata);
    ES::set_reg(sels.kdata);
    SS::set_reg(sels.kdata);
    load_tss(sels.tss);

    debug_assert_eq!(
        CS::get_reg().0,
        sels.kcode.0,
        "CS != kernel code selector (0x{:04x} != 0x{:04x})",
        CS::get_reg().0,
        sels.kcode.0
    );
}

pub fn user_segments() -> (SegmentSelector, SegmentSelector) {
    let (_, sels) = GDT.get().expect("GDT not initialized");
    (sels.ucode, sels.udata)
}
