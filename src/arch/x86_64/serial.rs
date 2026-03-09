use core::fmt::{self, Write};
use x86_64::instructions::port::Port;
use crate::bootlog::bootlog_push_line;
const COM1: u16 = 0x3F8;
const KERNEL_LINE_BUF: usize = 256;

static mut KLINE_BUF: [u8; KERNEL_LINE_BUF] = [0; KERNEL_LINE_BUF];
static mut KLINE_LEN: usize = 0;

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
fn write_byte_raw(b: u8) {
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
        write_byte_raw(b);
    }
}

#[inline(always)]
fn kflush_line_buf() {
    unsafe {
        if KLINE_LEN == 0 {
            return;
        }

        let s = core::str::from_utf8(&KLINE_BUF[..KLINE_LEN]).unwrap_or("[err] utf8");
        bootlog_push_line(s);
        KLINE_LEN = 0;
    }
}

#[inline(always)]
fn kfeed_bootlog_byte(b: u8) {
    unsafe {
        match b {
            b'\n' => kflush_line_buf(),
            b'\r' => {}
            _ => {
                if KLINE_LEN < KERNEL_LINE_BUF {
                    KLINE_BUF[KLINE_LEN] = b;
                    KLINE_LEN += 1;
                } else {
                    kflush_line_buf();
                    KLINE_BUF[0] = b;
                    KLINE_LEN = 1;
                }
            }
        }
    }
}

#[inline(always)]
fn kwrite_str_bytes(s: &str) {
    for &b in s.as_bytes() {
        write_byte_raw(b);
        kfeed_bootlog_byte(b);
    }
}

struct SerialWriter;
struct KernelSerialWriter;

impl Write for SerialWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        write_str_bytes(s);
        Ok(())
    }
}

impl Write for KernelSerialWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        kwrite_str_bytes(s);
        Ok(())
    }
}

#[inline(always)]
pub fn print_args_core(args: fmt::Arguments) {
    init();
    let _ = SerialWriter.write_fmt(args);
}

#[inline(always)]
pub fn kprint_args_core(args: fmt::Arguments) {
    init();
    let _ = KernelSerialWriter.write_fmt(args);
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

#[macro_export]
macro_rules! ksprint {
    ($($arg:tt)*) => ({
        $crate::serial::kprint_args_core(core::format_args!($($arg)*));
    })
}

#[macro_export]
macro_rules! ksprintln {
    () => ($crate::serial::kprint_args_core(core::format_args!("\n")));
    ($($arg:tt)*) => ($crate::serial::kprint_args_core(core::format_args!("{}\n", core::format_args!($($arg)*))));
}
