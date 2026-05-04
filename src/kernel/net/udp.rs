extern crate alloc;

use alloc::vec::Vec;
use spin::{Mutex, Once};

use crate::kernel::net::{arp, checksum, ethernet, iface, ipv4};
use crate::ksprintln;

const UDP_HEADER_LEN: usize = 8;
const MAX_UDP_PAYLOAD: usize = 1472;
const MAX_UDP_QUEUE: usize = 32;
const DEFAULT_POLLS: u32 = 20_000;

#[derive(Clone)]
struct UdpDatagram {
    src_ip: [u8; 4],
    src_port: u16,
    dst_port: u16,
    data: Vec<u8>,
}

struct UdpState {
    rx: Vec<UdpDatagram>,
}

static UDP_STATE: Once<Mutex<UdpState>> = Once::new();

fn state() -> &'static Mutex<UdpState> {
    UDP_STATE.call_once(|| Mutex::new(UdpState { rx: Vec::new() }))
}

fn same_subnet(a: [u8; 4], b: [u8; 4], mask: [u8; 4]) -> bool {
    for i in 0..4 {
        if (a[i] & mask[i]) != (b[i] & mask[i]) {
            return false;
        }
    }

    true
}

fn next_hop_for(dst_ip: [u8; 4]) -> [u8; 4] {
    let ours = iface::ipv4_addr();
    let mask = iface::netmask();

    if same_subnet(ours, dst_ip, mask) {
        dst_ip
    } else {
        iface::gateway()
    }
}

fn resolve_mac(ip: [u8; 4], polls: u32) -> Option<[u8; 6]> {
    if let Some(mac) = arp::lookup(ip) {
        return Some(mac);
    }

    if !arp::send_request(ip) {
        return None;
    }

    for _ in 0..polls {
        ethernet::poll_once();

        if let Some(mac) = arp::lookup(ip) {
            return Some(mac);
        }

        core::hint::spin_loop();
    }

    None
}

fn add_words(sum: &mut u32, data: &[u8]) {
    let mut i = 0usize;

    while i + 1 < data.len() {
        let word = ((data[i] as u16) << 8) | data[i + 1] as u16;
        *sum = sum.wrapping_add(word as u32);
        i += 2;
    }

    if i < data.len() {
        *sum = sum.wrapping_add((data[i] as u32) << 8);
    }
}

fn udp_checksum(src_ip: [u8; 4], dst_ip: [u8; 4], segment: &[u8]) -> u16 {
    let mut pseudo = [0u8; 12];

    pseudo[0..4].copy_from_slice(&src_ip);
    pseudo[4..8].copy_from_slice(&dst_ip);
    pseudo[8] = 0;
    pseudo[9] = ipv4::IPV4_PROTO_UDP;
    pseudo[10..12].copy_from_slice(&(segment.len() as u16).to_be_bytes());

    let mut sum: u32 = 0;

    add_words(&mut sum, &pseudo);
    add_words(&mut sum, segment);

    while (sum >> 16) != 0 {
        sum = (sum & 0xffff).wrapping_add(sum >> 16);
    }

    !(sum as u16)
}

pub fn send_to(
    dst_ip: [u8; 4],
    dst_port: u16,
    src_port: u16,
    data: &[u8],
    timeout_polls: u32,
) -> Result<usize, i64> {
    if dst_ip == [0, 0, 0, 0] || dst_port == 0 || src_port == 0 {
        return Err(-micros_abi::errno::EINVAL);
    }

    if data.is_empty() || data.len() > MAX_UDP_PAYLOAD {
        return Err(-micros_abi::errno::EINVAL);
    }

    if !iface::is_ready() {
        return Err(-micros_abi::errno::ENODEV);
    }

    if !iface::link_up() {
        return Err(-micros_abi::errno::EIO);
    }

    let polls = if timeout_polls == 0 {
        DEFAULT_POLLS
    } else {
        timeout_polls
    };

    let hop_ip = next_hop_for(dst_ip);
    let dst_mac = match resolve_mac(hop_ip, polls) {
        Some(v) => v,
        None => return Err(-micros_abi::errno::EAGAIN),
    };

    let udp_len = UDP_HEADER_LEN + data.len();
    let mut udp = [0u8; 1500];

    udp[0..2].copy_from_slice(&src_port.to_be_bytes());
    udp[2..4].copy_from_slice(&dst_port.to_be_bytes());
    udp[4..6].copy_from_slice(&(udp_len as u16).to_be_bytes());
    udp[6..8].copy_from_slice(&0u16.to_be_bytes());
    udp[UDP_HEADER_LEN..udp_len].copy_from_slice(data);

    let csum = udp_checksum(iface::ipv4_addr(), dst_ip, &udp[..udp_len]);
    let csum = if csum == 0 { 0xffff } else { csum };
    udp[6..8].copy_from_slice(&csum.to_be_bytes());

    ksprintln!(
        "[net][udp][tx] {}.{}.{}.{}:{} -> {}.{}.{}.{}:{} len={}",
        iface::ipv4_addr()[0],
        iface::ipv4_addr()[1],
        iface::ipv4_addr()[2],
        iface::ipv4_addr()[3],
        src_port,
        dst_ip[0],
        dst_ip[1],
        dst_ip[2],
        dst_ip[3],
        dst_port,
        data.len()
    );

    if !ipv4::send(dst_mac, dst_ip, ipv4::IPV4_PROTO_UDP, &udp[..udp_len]) {
        return Err(-micros_abi::errno::EIO);
    }

    Ok(data.len())
}

pub fn recv_from(
    local_port: u16,
    out: &mut [u8],
    timeout_polls: u32,
) -> Result<([u8; 4], u16, usize), i64> {
    if local_port == 0 || out.is_empty() {
        return Err(-micros_abi::errno::EINVAL);
    }

    let polls = if timeout_polls == 0 {
        DEFAULT_POLLS
    } else {
        timeout_polls
    };

    for _ in 0..polls {
        {
            let mut guard = state().lock();

            if let Some(pos) = guard.rx.iter().position(|d| d.dst_port == local_port) {
                let d = guard.rx.remove(pos);
                let n = core::cmp::min(out.len(), d.data.len());

                out[..n].copy_from_slice(&d.data[..n]);

                return Ok((d.src_ip, d.src_port, n));
            }
        }

        ethernet::poll_once();
        core::hint::spin_loop();
    }

    Err(-micros_abi::errno::EAGAIN)
}

pub fn handle_packet(_src_mac: [u8; 6], packet: &ipv4::Ipv4Packet<'_>) {
    let p = packet.payload;

    if p.len() < UDP_HEADER_LEN {
        return;
    }

    let src_port = u16::from_be_bytes([p[0], p[1]]);
    let dst_port = u16::from_be_bytes([p[2], p[3]]);
    let udp_len = u16::from_be_bytes([p[4], p[5]]) as usize;
    let recv_csum = u16::from_be_bytes([p[6], p[7]]);

    if udp_len < UDP_HEADER_LEN || udp_len > p.len() {
        return;
    }

    if recv_csum != 0 && udp_checksum(packet.src, packet.dst, &p[..udp_len]) != 0 {
        ksprintln!(
            "[net][udp][rx] bad checksum from {}.{}.{}.{}:{}",
            packet.src[0],
            packet.src[1],
            packet.src[2],
            packet.src[3],
            src_port
        );
        return;
    }

    let data = &p[UDP_HEADER_LEN..udp_len];

    ksprintln!(
        "[net][udp][rx] {}.{}.{}.{}:{} -> local:{} len={}",
        packet.src[0],
        packet.src[1],
        packet.src[2],
        packet.src[3],
        src_port,
        dst_port,
        data.len()
    );

    let mut guard = state().lock();

    if guard.rx.len() >= MAX_UDP_QUEUE {
        guard.rx.remove(0);
    }

    let mut copied = Vec::with_capacity(data.len());
    copied.extend_from_slice(data);

    guard.rx.push(UdpDatagram {
        src_ip: packet.src,
        src_port,
        dst_port,
        data: copied,
    });
}
