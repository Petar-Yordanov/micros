use crate::kernel::net::{arp, iface, ipv4};
use crate::ksprintln;

pub const ETHERTYPE_IPV4: u16 = 0x0800;
pub const ETHERTYPE_ARP: u16 = 0x0806;

const ETH_HEADER_LEN: usize = 14;
const ETH_MIN_FRAME_LEN_NO_FCS: usize = 60;

#[derive(Clone, Copy)]
pub struct EthernetFrame<'a> {
    pub dst: [u8; 6],
    pub src: [u8; 6],
    pub ethertype: u16,
    pub payload: &'a [u8],
}

pub fn parse(frame: &[u8]) -> Option<EthernetFrame<'_>> {
    if frame.len() < ETH_HEADER_LEN {
        return None;
    }

    let mut dst = [0u8; 6];
    let mut src = [0u8; 6];

    dst.copy_from_slice(&frame[0..6]);
    src.copy_from_slice(&frame[6..12]);

    let ethertype = u16::from_be_bytes([frame[12], frame[13]]);

    Some(EthernetFrame {
        dst,
        src,
        ethertype,
        payload: &frame[ETH_HEADER_LEN..],
    })
}

pub fn build(
    dst: [u8; 6],
    src: [u8; 6],
    ethertype: u16,
    payload: &[u8],
    out: &mut [u8],
) -> Option<usize> {
    let raw_total = ETH_HEADER_LEN.checked_add(payload.len())?;
    let total = core::cmp::max(raw_total, ETH_MIN_FRAME_LEN_NO_FCS);

    if out.len() < total {
        return None;
    }

    out[0..6].copy_from_slice(&dst);
    out[6..12].copy_from_slice(&src);
    out[12..14].copy_from_slice(&ethertype.to_be_bytes());
    out[14..raw_total].copy_from_slice(payload);

    if total > raw_total {
        out[raw_total..total].fill(0);
    }

    Some(total)
}

pub fn send(dst: [u8; 6], ethertype: u16, payload: &[u8]) -> bool {
    let src = match iface::mac_addr() {
        Some(v) => v,
        None => {
            ksprintln!(
                "[net][eth][tx] no source MAC; drop ethertype={:#06x}",
                ethertype
            );
            return false;
        }
    };

    let mut frame = [0u8; 1600];
    let n = match build(dst, src, ethertype, payload, &mut frame) {
        Some(v) => v,
        None => {
            ksprintln!(
                "[net][eth][tx] build failed ethertype={:#06x} payload_len={}",
                ethertype,
                payload.len()
            );
            return false;
        }
    };

    ksprintln!(
        "[net][eth][tx] len={} payload_len={} ethertype={:#06x} dst={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x} src={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        n,
        payload.len(),
        ethertype,
        dst[0],
        dst[1],
        dst[2],
        dst[3],
        dst[4],
        dst[5],
        src[0],
        src[1],
        src[2],
        src[3],
        src[4],
        src[5]
    );

    iface::send_frame(&frame[..n])
}

pub fn poll_once() -> bool {
    if !iface::is_ready() {
        return false;
    }

    let mut buf = [0u8; 4096];
    let n = match iface::recv_frame(&mut buf) {
        Some(v) => v,
        None => return false,
    };

    if n == 0 {
        return true;
    }

    let frame = match parse(&buf[..n]) {
        Some(v) => v,
        None => {
            ksprintln!("[net][eth][rx] drop short frame len={}", n);
            return true;
        }
    };

    ksprintln!(
        "[net][eth][rx] len={} ethertype={:#06x} dst={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x} src={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        n,
        frame.ethertype,
        frame.dst[0],
        frame.dst[1],
        frame.dst[2],
        frame.dst[3],
        frame.dst[4],
        frame.dst[5],
        frame.src[0],
        frame.src[1],
        frame.src[2],
        frame.src[3],
        frame.src[4],
        frame.src[5]
    );

    if !iface::is_our_mac(frame.dst) && !iface::is_broadcast_mac(frame.dst) {
        ksprintln!(
            "[net][eth][rx] drop not-for-us dst={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            frame.dst[0],
            frame.dst[1],
            frame.dst[2],
            frame.dst[3],
            frame.dst[4],
            frame.dst[5]
        );
        return true;
    }

    match frame.ethertype {
        ETHERTYPE_ARP => arp::handle_packet(&frame),
        ETHERTYPE_IPV4 => ipv4::handle_packet(&frame),
        _ => {
            ksprintln!("[net][eth][rx] ignored ethertype={:#06x}", frame.ethertype);
        }
    }

    true
}
