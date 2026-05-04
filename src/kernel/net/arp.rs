extern crate alloc;

use alloc::vec::Vec;
use spin::{Mutex, Once};

use crate::kernel::net::{ethernet, iface};
use crate::ksprintln;

const ARP_HTYPE_ETHERNET: u16 = 1;
const ARP_PTYPE_IPV4: u16 = 0x0800;
const ARP_HLEN_ETHERNET: u8 = 6;
const ARP_PLEN_IPV4: u8 = 4;
const ARP_OP_REQUEST: u16 = 1;
const ARP_OP_REPLY: u16 = 2;

#[derive(Clone, Copy)]
struct ArpEntry {
    ip: [u8; 4],
    mac: [u8; 6],
}

static ARP_CACHE: Once<Mutex<Vec<ArpEntry>>> = Once::new();

fn cache() -> &'static Mutex<Vec<ArpEntry>> {
    ARP_CACHE.call_once(|| Mutex::new(Vec::new()))
}

pub fn lookup(ip: [u8; 4]) -> Option<[u8; 6]> {
    let guard = cache().lock();
    guard.iter().find(|e| e.ip == ip).map(|e| e.mac)
}

pub fn update(ip: [u8; 4], mac: [u8; 6]) {
    if ip == [0, 0, 0, 0] || mac == [0; 6] || mac == [0xff; 6] {
        ksprintln!(
            "[net][arp] ignore invalid cache update ip={}.{}.{}.{} mac={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            ip[0],
            ip[1],
            ip[2],
            ip[3],
            mac[0],
            mac[1],
            mac[2],
            mac[3],
            mac[4],
            mac[5]
        );
        return;
    }

    let mut guard = cache().lock();

    if let Some(e) = guard.iter_mut().find(|e| e.ip == ip) {
        e.mac = mac;

        ksprintln!(
            "[net][arp] cache update {}.{}.{}.{} -> {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            ip[0],
            ip[1],
            ip[2],
            ip[3],
            mac[0],
            mac[1],
            mac[2],
            mac[3],
            mac[4],
            mac[5]
        );

        return;
    }

    if guard.len() >= 32 {
        guard.remove(0);
    }

    guard.push(ArpEntry { ip, mac });

    ksprintln!(
        "[net][arp] cache insert {}.{}.{}.{} -> {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        ip[0],
        ip[1],
        ip[2],
        ip[3],
        mac[0],
        mac[1],
        mac[2],
        mac[3],
        mac[4],
        mac[5]
    );
}

pub fn handle_packet(frame: &ethernet::EthernetFrame<'_>) {
    let p = frame.payload;

    if p.len() < 28 {
        ksprintln!("[net][arp] drop short payload len={}", p.len());
        return;
    }

    let htype = u16::from_be_bytes([p[0], p[1]]);
    let ptype = u16::from_be_bytes([p[2], p[3]]);
    let hlen = p[4];
    let plen = p[5];
    let oper = u16::from_be_bytes([p[6], p[7]]);

    if htype != ARP_HTYPE_ETHERNET {
        ksprintln!("[net][arp] drop bad htype={:#06x}", htype);
        return;
    }

    if ptype != ARP_PTYPE_IPV4 {
        ksprintln!("[net][arp] drop bad ptype={:#06x}", ptype);
        return;
    }

    if hlen != ARP_HLEN_ETHERNET || plen != ARP_PLEN_IPV4 {
        ksprintln!("[net][arp] drop bad sizes hlen={} plen={}", hlen, plen);
        return;
    }

    let mut sha = [0u8; 6];
    let mut spa = [0u8; 4];
    let mut tha = [0u8; 6];
    let mut tpa = [0u8; 4];

    sha.copy_from_slice(&p[8..14]);
    spa.copy_from_slice(&p[14..18]);
    tha.copy_from_slice(&p[18..24]);
    tpa.copy_from_slice(&p[24..28]);

    ksprintln!(
        "[net][arp] rx op={} spa={}.{}.{}.{} sha={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x} tpa={}.{}.{}.{} tha={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        oper,
        spa[0],
        spa[1],
        spa[2],
        spa[3],
        sha[0],
        sha[1],
        sha[2],
        sha[3],
        sha[4],
        sha[5],
        tpa[0],
        tpa[1],
        tpa[2],
        tpa[3],
        tha[0],
        tha[1],
        tha[2],
        tha[3],
        tha[4],
        tha[5]
    );

    update(spa, sha);

    match oper {
        ARP_OP_REQUEST => {
            if iface::is_our_ipv4(tpa) {
                send_reply(sha, spa);
            }
        }
        ARP_OP_REPLY => {
            ksprintln!(
                "[net][arp] reply accepted from {}.{}.{}.{}",
                spa[0],
                spa[1],
                spa[2],
                spa[3]
            );
        }
        _ => {
            ksprintln!("[net][arp] ignored op={}", oper);
        }
    }
}

pub fn send_request(target_ip: [u8; 4]) -> bool {
    let src_mac = match iface::mac_addr() {
        Some(v) => v,
        None => {
            ksprintln!("[net][arp] cannot send request: no source MAC");
            return false;
        }
    };

    let src_ip = iface::ipv4_addr();
    let mut arp = [0u8; 28];

    arp[0..2].copy_from_slice(&ARP_HTYPE_ETHERNET.to_be_bytes());
    arp[2..4].copy_from_slice(&ARP_PTYPE_IPV4.to_be_bytes());
    arp[4] = ARP_HLEN_ETHERNET;
    arp[5] = ARP_PLEN_IPV4;
    arp[6..8].copy_from_slice(&ARP_OP_REQUEST.to_be_bytes());
    arp[8..14].copy_from_slice(&src_mac);
    arp[14..18].copy_from_slice(&src_ip);
    arp[18..24].copy_from_slice(&[0u8; 6]);
    arp[24..28].copy_from_slice(&target_ip);

    ksprintln!(
        "[net][arp] request who-has {}.{}.{}.{} tell {}.{}.{}.{} src_mac={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        target_ip[0],
        target_ip[1],
        target_ip[2],
        target_ip[3],
        src_ip[0],
        src_ip[1],
        src_ip[2],
        src_ip[3],
        src_mac[0],
        src_mac[1],
        src_mac[2],
        src_mac[3],
        src_mac[4],
        src_mac[5]
    );

    ethernet::send([0xff; 6], ethernet::ETHERTYPE_ARP, &arp)
}

fn send_reply(dst_mac: [u8; 6], dst_ip: [u8; 4]) -> bool {
    let src_mac = match iface::mac_addr() {
        Some(v) => v,
        None => return false,
    };

    let src_ip = iface::ipv4_addr();
    let mut arp = [0u8; 28];

    arp[0..2].copy_from_slice(&ARP_HTYPE_ETHERNET.to_be_bytes());
    arp[2..4].copy_from_slice(&ARP_PTYPE_IPV4.to_be_bytes());
    arp[4] = ARP_HLEN_ETHERNET;
    arp[5] = ARP_PLEN_IPV4;
    arp[6..8].copy_from_slice(&ARP_OP_REPLY.to_be_bytes());
    arp[8..14].copy_from_slice(&src_mac);
    arp[14..18].copy_from_slice(&src_ip);
    arp[18..24].copy_from_slice(&dst_mac);
    arp[24..28].copy_from_slice(&dst_ip);

    ksprintln!(
        "[net][arp] reply to {}.{}.{}.{} mac={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        dst_ip[0],
        dst_ip[1],
        dst_ip[2],
        dst_ip[3],
        dst_mac[0],
        dst_mac[1],
        dst_mac[2],
        dst_mac[3],
        dst_mac[4],
        dst_mac[5]
    );

    ethernet::send(dst_mac, ethernet::ETHERTYPE_ARP, &arp)
}
