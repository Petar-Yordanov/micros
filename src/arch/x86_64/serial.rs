use core::fmt::{self, Write};
use x86_64::instructions::port::Port;

const COM1: u16 = 0x3F8;

#[inline(always)]
pub fn init() {
    unsafe {
        Port::<u8>::new(COM1 + 1).write(0x00);
        Port::<u8>::new(COM1 + 3).write(0x80);
        Port::<u8>::new(COM1 + 0).write(0x01);
        Port::<u8>::new(COM1 + 1).write(0x00);
        Port::<u8>::new(COM1 + 3).write(0x03);
        Port::<u8>::new(COM1 + 2).write(0xC7);
        Port::<u8>::new(COM1 + 4).write(0x0B);
    }
}

#[inline(always)]
fn write_byte(b: u8) {
    unsafe {
        let mut lsr: Port<u8> = Port::new(COM1 + 5);
        let mut data: Port<u8> = Port::new(COM1 + 0);
        while (lsr.read() & 0x20) == 0 {}
        data.write(b);
    }
}

#[inline(always)]
fn write_str_bytes(s: &str) {
    for &b in s.as_bytes() {
        write_byte(b);
    }
}

struct SerialWriter;

impl Write for SerialWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        write_str_bytes(s);
        Ok(())
    }
}

#[inline(always)]
pub fn print_args_core(args: fmt::Arguments) {
    init();
    let _ = SerialWriter.write_fmt(args);
}

#[macro_export]
macro_rules! sprint {
    ($($arg:tt)*) => ({
        $crate::serial::print_args_core(core::format_args!($($arg)*));
    })
}

#[macro_export]
macro_rules! sprintln {
    () => ($crate::serial::print_args_core(core::format_args!("\n")));
    ($($arg:tt)*) => ($crate::serial::print_args_core(core::format_args!("{}\n", core::format_args!($($arg)*))));
}
