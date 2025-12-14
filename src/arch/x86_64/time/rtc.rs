#![allow(dead_code)]

use core::fmt;
use x86_64::instructions::port::Port;

use limine::request::DateAtBootRequest;
static DATE_AT_BOOT_REQ: DateAtBootRequest = DateAtBootRequest::new();

#[derive(Clone, Copy, Debug)]
pub struct RtcDateTime {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub min: u8,
    pub sec: u8,
}

impl fmt::Display for RtcDateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC",
            self.year, self.month, self.day, self.hour, self.min, self.sec
        )
    }
}

pub fn wall_clock_epoch_secs() -> Option<u64> {
    if let Some(resp) = DATE_AT_BOOT_REQ.get_response() {
        return Some(resp.timestamp().as_secs());
    }
    read_cmos_epoch().ok()
}

pub fn read_cmos_datetime() -> Result<RtcDateTime, ()> {
    wait_uip_clear();
    let (mut sec, mut min, mut hour, mut day, mut mon, mut year, mut cent) = unsafe {
        (
            cmos_read(0x00),
            cmos_read(0x02),
            cmos_read(0x04),
            cmos_read(0x07),
            cmos_read(0x08),
            cmos_read(0x09),
            cmos_read(0x32),
        )
    };
    let reg_b = unsafe { cmos_read(0x0B) };
    let bcd = (reg_b & (1 << 2)) == 0;
    let twenty_four = (reg_b & (1 << 1)) != 0;

    if bcd {
        sec = bcd_to_bin(sec);
        min = bcd_to_bin(min);
        hour = bcd_to_bin(hour & 0x7F) | (hour & 0x80);
        day = bcd_to_bin(day);
        mon = bcd_to_bin(mon);
        year = bcd_to_bin(year);
        cent = bcd_to_bin(cent);
    }

    if !twenty_four {
        let pm = (hour & 0x80) != 0;
        hour &= 0x7F;
        if pm {
            if hour < 12 {
                hour += 12;
            }
        } else {
            if hour == 12 {
                hour = 0;
            }
        }
    }

    let full_year: u16 = if cent >= 19 && cent <= 21 {
        (cent as u16) * 100 + (year as u16)
    } else {
        2000 + (year as u16)
    };

    Ok(RtcDateTime {
        year: full_year,
        month: mon,
        day,
        hour,
        min,
        sec,
    })
}

pub fn read_cmos_epoch() -> Result<u64, ()> {
    let dt = read_cmos_datetime()?;
    Ok(datetime_to_unix(dt))
}

fn wait_uip_clear() {
    loop {
        let ra = unsafe { cmos_read(0x0A) };
        if (ra & 0x80) == 0 {
            break;
        }
        core::hint::spin_loop();
    }
}

#[inline(always)]
unsafe fn cmos_read(idx: u8) -> u8 {
    let mut sel = Port::<u8>::new(0x70);
    let mut dat = Port::<u8>::new(0x71);
    sel.write(0x80 | idx);
    dat.read()
}

#[inline(always)]
fn bcd_to_bin(x: u8) -> u8 {
    ((x >> 4) * 10) + (x & 0x0F)
}

fn is_leap(y: u16) -> bool {
    (y % 4 == 0) && ((y % 100 != 0) || (y % 400 == 0))
}

fn days_before_month(year: u16, month: u8) -> u32 {
    const CUM_DAYS: [u16; 12] = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
    let base = CUM_DAYS[(month as usize).saturating_sub(1)] as u32;
    if month >= 3 && is_leap(year) {
        base + 1
    } else {
        base
    }
}

fn days_since_unix_epoch(y: u16, m: u8, d: u8) -> i64 {
    let mut days = 0i64;
    let mut year = 1970u16;
    while year < y {
        days += if is_leap(year) { 366 } else { 365 };
        year += 1;
    }
    days += days_before_month(y, m) as i64;
    days += (d as i64) - 1;
    days
}

fn datetime_to_unix(dt: RtcDateTime) -> u64 {
    let days = days_since_unix_epoch(dt.year, dt.month, dt.day);
    let secs = days * 86_400 + (dt.hour as i64) * 3_600 + (dt.min as i64) * 60 + (dt.sec as i64);
    if secs < 0 {
        0
    } else {
        secs as u64
    }
}
