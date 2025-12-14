use x86_64::instructions::port::Port;

const PCI_CFG_ADDR: u16 = 0xCF8;
const PCI_CFG_DATA: u16 = 0xCFC;

#[inline(always)]
fn pci_cfg_addr(bus: u8, dev: u8, func: u8, off: u8) -> u32 {
    (1u32 << 31)
        | ((bus as u32) << 16)
        | ((dev as u32) << 11)
        | ((func as u32) << 8)
        | ((off as u32) & 0xFC)
}

#[inline(always)]
pub fn read_u32(bus: u8, dev: u8, func: u8, off: u8) -> u32 {
    unsafe {
        Port::<u32>::new(PCI_CFG_ADDR).write(pci_cfg_addr(bus, dev, func, off));
        Port::<u32>::new(PCI_CFG_DATA).read()
    }
}

#[inline(always)]
pub fn read_u16(bus: u8, dev: u8, func: u8, off: u8) -> u16 {
    let v = read_u32(bus, dev, func, off & !3);
    let shift = ((off & 3) as u32) * 8;
    ((v >> shift) & 0xFFFF) as u16
}

#[inline(always)]
pub fn read_u8(bus: u8, dev: u8, func: u8, off: u8) -> u8 {
    let v = read_u32(bus, dev, func, off & !3);
    let shift = ((off & 3) as u32) * 8;
    ((v >> shift) & 0xFF) as u8
}

#[inline(always)]
pub fn write_u32(bus: u8, dev: u8, func: u8, off: u8, val: u32) {
    unsafe {
        Port::<u32>::new(PCI_CFG_ADDR).write(pci_cfg_addr(bus, dev, func, off));
        Port::<u32>::new(PCI_CFG_DATA).write(val);
    }
}

#[inline(always)]
pub fn write_u16(bus: u8, dev: u8, func: u8, off: u8, val: u16) {
    let off_al = off & !3;
    let shift = ((off & 3) as u32) * 8;

    let mut d = read_u32(bus, dev, func, off_al);
    d &= !(0xFFFFu32 << shift);
    d |= (val as u32) << shift;

    write_u32(bus, dev, func, off_al, d);
}
