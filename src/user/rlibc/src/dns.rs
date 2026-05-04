extern crate alloc;

use alloc::vec::Vec;
use core::sync::atomic::{AtomicU16, Ordering};

use crate::errno::Errno;
use crate::net;

const DNS_SERVER_QEMU_USER: [u8; 4] = [10, 0, 2, 3];
const DNS_PORT: u16 = 53;
const DNS_LOCAL_PORT_BASE: u16 = 53000;
const DNS_TYPE_A: u16 = 1;
const DNS_CLASS_IN: u16 = 1;

static NEXT_DNS_ID: AtomicU16 = AtomicU16::new(1);

pub fn resolve_a(name: &str) -> Result<[u8; 4], Errno> {
    let ips = resolve_a_all(name)?;

    if ips.is_empty() {
        return Err(Errno(micros_abi::errno::EAGAIN as i32));
    }

    Ok(ips[0])
}

pub fn resolve_a_all(name: &str) -> Result<Vec<[u8; 4]>, Errno> {
    resolve_a_all_with_server(name, DNS_SERVER_QEMU_USER)
}

pub fn resolve_a_with_server(name: &str, dns_server: [u8; 4]) -> Result<[u8; 4], Errno> {
    let ips = resolve_a_all_with_server(name, dns_server)?;

    if ips.is_empty() {
        return Err(Errno(micros_abi::errno::EAGAIN as i32));
    }

    Ok(ips[0])
}

pub fn resolve_a_all_with_server(name: &str, dns_server: [u8; 4]) -> Result<Vec<[u8; 4]>, Errno> {
    let id = NEXT_DNS_ID.fetch_add(1, Ordering::Relaxed);
    let local_port = DNS_LOCAL_PORT_BASE.wrapping_add(id % 1000);

    let query = build_query(name, id)?;
    net::udp_send_to(dns_server, DNS_PORT, local_port, &query)?;

    let mut buf = [0u8; 512];

    loop {
        let (src_ip, src_port, n) = net::udp_recv_from(local_port, &mut buf)?;

        if src_ip != dns_server || src_port != DNS_PORT {
            continue;
        }

        let ips = parse_response_all(&buf[..n], id)?;

        if ips.is_empty() {
            return Err(Errno(micros_abi::errno::EAGAIN as i32));
        }

        return Ok(ips);
    }
}

fn build_query(name: &str, id: u16) -> Result<Vec<u8>, Errno> {
    if name.is_empty() || name.len() > 253 {
        return Err(Errno(micros_abi::errno::EINVAL as i32));
    }

    let mut out = Vec::new();

    out.extend_from_slice(&id.to_be_bytes());
    out.extend_from_slice(&0x0100u16.to_be_bytes());
    out.extend_from_slice(&1u16.to_be_bytes());
    out.extend_from_slice(&0u16.to_be_bytes());
    out.extend_from_slice(&0u16.to_be_bytes());
    out.extend_from_slice(&0u16.to_be_bytes());

    for label in name.split('.') {
        if label.is_empty() || label.len() > 63 {
            return Err(Errno(micros_abi::errno::EINVAL as i32));
        }

        out.push(label.len() as u8);
        out.extend_from_slice(label.as_bytes());
    }

    out.push(0);
    out.extend_from_slice(&DNS_TYPE_A.to_be_bytes());
    out.extend_from_slice(&DNS_CLASS_IN.to_be_bytes());

    Ok(out)
}

fn parse_response_all(buf: &[u8], expected_id: u16) -> Result<Vec<[u8; 4]>, Errno> {
    if buf.len() < 12 {
        return Err(Errno(micros_abi::errno::EINVAL as i32));
    }

    let id = u16::from_be_bytes([buf[0], buf[1]]);
    if id != expected_id {
        return Err(Errno(micros_abi::errno::EINVAL as i32));
    }

    let flags = u16::from_be_bytes([buf[2], buf[3]]);
    let is_response = (flags & 0x8000) != 0;
    let rcode = flags & 0x000f;

    if !is_response || rcode != 0 {
        return Err(Errno(micros_abi::errno::EINVAL as i32));
    }

    let qdcount = u16::from_be_bytes([buf[4], buf[5]]) as usize;
    let ancount = u16::from_be_bytes([buf[6], buf[7]]) as usize;

    let mut off = 12usize;

    for _ in 0..qdcount {
        off = skip_name(buf, off).ok_or(Errno(micros_abi::errno::EINVAL as i32))?;

        if off + 4 > buf.len() {
            return Err(Errno(micros_abi::errno::EINVAL as i32));
        }

        off += 4;
    }

    let mut ips = Vec::new();

    for _ in 0..ancount {
        off = skip_name(buf, off).ok_or(Errno(micros_abi::errno::EINVAL as i32))?;

        if off + 10 > buf.len() {
            return Err(Errno(micros_abi::errno::EINVAL as i32));
        }

        let rr_type = u16::from_be_bytes([buf[off], buf[off + 1]]);
        let rr_class = u16::from_be_bytes([buf[off + 2], buf[off + 3]]);
        let rdlen = u16::from_be_bytes([buf[off + 8], buf[off + 9]]) as usize;

        off += 10;

        if off + rdlen > buf.len() {
            return Err(Errno(micros_abi::errno::EINVAL as i32));
        }

        if rr_type == DNS_TYPE_A && rr_class == DNS_CLASS_IN && rdlen == 4 {
            let ip = [buf[off], buf[off + 1], buf[off + 2], buf[off + 3]];

            if !contains_ip(&ips, ip) {
                ips.push(ip);
            }
        }

        off += rdlen;
    }

    Ok(ips)
}

fn contains_ip(ips: &[[u8; 4]], ip: [u8; 4]) -> bool {
    for existing in ips {
        if *existing == ip {
            return true;
        }
    }

    false
}

fn skip_name(buf: &[u8], mut off: usize) -> Option<usize> {
    let mut jumps = 0usize;

    loop {
        if off >= buf.len() {
            return None;
        }

        let len = buf[off];

        if (len & 0xc0) == 0xc0 {
            if off + 1 >= buf.len() {
                return None;
            }

            off += 2;
            return Some(off);
        }

        if len == 0 {
            return Some(off + 1);
        }

        if (len & 0xc0) != 0 {
            return None;
        }

        off += 1 + len as usize;

        jumps += 1;
        if jumps > 128 {
            return None;
        }
    }
}
