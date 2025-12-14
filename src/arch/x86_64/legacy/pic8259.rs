use x86_64::instructions::port::Port;

pub unsafe fn disable_8259_pic() {
    Port::<u8>::new(0x21).write(0xFF);
    Port::<u8>::new(0xA1).write(0xFF);
}
