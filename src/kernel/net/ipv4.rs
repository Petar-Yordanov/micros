use core::sync::atomic::{AtomicU16, Ordering};

use crate::kernel::net::{checksum, ethernet, icmpv4, iface, tcp, udp};

const IPV4_VERSION: u8 = 4;
const IPV4_IHL_WORDS: u8 = 5;
const IPV4_HEADER_LEN: usize = 20;

pub const IPV4_PROTO_ICMP: u8 = 1;
pub const IPV4_PROTO_TCP: u8 = 6;
pub const IPV4_PROTO_UDP: u8 = 17;

static NEXT_ID: AtomicU16 = AtomicU16::new(1);

#[derive(Clone, Copy)]
pub struct Ipv4Packet<'a> {
    pub src: [u8; 4],
    pub dst: [u8; 4],
    pub proto: u8,
    pub ttl: u8,
    pub payload: &'a [u8],
}

pub fn handle_packet(frame: &ethernet::EthernetFrame<'_>) {
    let packet = match parse(frame.payload) {
        Some(v) => v,
        None => return,
    };

    if !iface::is_our_ipv4(packet.dst) && packet.dst != [255, 255, 255, 255] {
        return;
    }

    match packet.proto {
        IPV4_PROTO_ICMP => icmpv4::handle_packet(frame.src, &packet),
        IPV4_PROTO_TCP => tcp::handle_packet(frame.src, &packet),
        IPV4_PROTO_UDP => udp::handle_packet(frame.src, &packet),
        _ => {}
    }
}

pub fn parse(buf: &[u8]) -> Option<Ipv4Packet<'_>> {
    if buf.len() < IPV4_HEADER_LEN {
        return None;
    }

    let version = buf[0] >> 4;
    let ihl_words = buf[0] & 0x0f;

    if version != IPV4_VERSION {
        return None;
    }

    if ihl_words < IPV4_IHL_WORDS {
        return None;
    }

    let ihl = (ihl_words as usize) * 4;
    if buf.len() < ihl {
        return None;
    }

    let total_len = u16::from_be_bytes([buf[2], buf[3]]) as usize;
    if total_len < ihl || total_len > buf.len() {
        return None;
    }

    if checksum::internet_checksum(&buf[..ihl]) != 0 {
        return None;
    }

    let flags_frag = u16::from_be_bytes([buf[6], buf[7]]);
    let frag_offset = flags_frag & 0x1fff;
    let more_frags = (flags_frag & 0x2000) != 0;

    if frag_offset != 0 || more_frags {
        return None;
    }

    let mut src = [0u8; 4];
    let mut dst = [0u8; 4];

    src.copy_from_slice(&buf[12..16]);
    dst.copy_from_slice(&buf[16..20]);

    Some(Ipv4Packet {
        src,
        dst,
        proto: buf[9],
        ttl: buf[8],
        payload: &buf[ihl..total_len],
    })
}

pub fn send(dst_mac: [u8; 6], dst_ip: [u8; 4], proto: u8, payload: &[u8]) -> bool {
    if payload.len() + IPV4_HEADER_LEN > 0xffff {
        return false;
    }

    let src_ip = iface::ipv4_addr();
    let total_len = IPV4_HEADER_LEN + payload.len();

    let mut ip_packet = [0u8; 1600];

    if ip_packet.len() < total_len {
        return false;
    }

    ip_packet[0] = (IPV4_VERSION << 4) | IPV4_IHL_WORDS;
    ip_packet[1] = 0;
    ip_packet[2..4].copy_from_slice(&(total_len as u16).to_be_bytes());

    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    ip_packet[4..6].copy_from_slice(&id.to_be_bytes());
    ip_packet[6..8].copy_from_slice(&0u16.to_be_bytes());
    ip_packet[8] = 64;
    ip_packet[9] = proto;
    ip_packet[10..12].copy_from_slice(&0u16.to_be_bytes());
    ip_packet[12..16].copy_from_slice(&src_ip);
    ip_packet[16..20].copy_from_slice(&dst_ip);

    let csum = checksum::internet_checksum(&ip_packet[..IPV4_HEADER_LEN]);
    ip_packet[10..12].copy_from_slice(&csum.to_be_bytes());
    ip_packet[IPV4_HEADER_LEN..total_len].copy_from_slice(payload);

    ethernet::send(dst_mac, ethernet::ETHERTYPE_IPV4, &ip_packet[..total_len])
}
