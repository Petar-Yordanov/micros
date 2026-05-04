use core::sync::atomic::{AtomicU16, Ordering};

use crate::kernel::net::{checksum, ipv4};
use crate::ksprintln;

const ICMP_ECHO_REPLY: u8 = 0;
const ICMP_ECHO_REQUEST: u8 = 8;
const ICMP_PROTO: u8 = 1;

const PING_ID: u16 = 0x4d53;
static NEXT_SEQ: AtomicU16 = AtomicU16::new(1);

pub fn handle_packet(src_mac: [u8; 6], packet: &ipv4::Ipv4Packet<'_>) {
    let p = packet.payload;

    if p.len() < 8 {
        return;
    }

    let kind = p[0];
    let code = p[1];

    if checksum::internet_checksum(p) != 0 {
        return;
    }

    match (kind, code) {
        (ICMP_ECHO_REQUEST, 0) => {
            send_echo_reply(src_mac, packet.src, p);
        }
        (ICMP_ECHO_REPLY, 0) => {
            let id = u16::from_be_bytes([p[4], p[5]]);
            let seq = u16::from_be_bytes([p[6], p[7]]);

            ksprintln!(
                "[net][icmp] echo reply from {}.{}.{}.{} id={:#06x} seq={}",
                packet.src[0],
                packet.src[1],
                packet.src[2],
                packet.src[3],
                id,
                seq
            );
        }
        _ => {}
    }
}

pub fn send_echo_request(dst_mac: [u8; 6], dst_ip: [u8; 4]) -> bool {
    let seq = NEXT_SEQ.fetch_add(1, Ordering::Relaxed);

    let payload = b"MicrOS64 ping";
    let len = 8 + payload.len();

    let mut req = [0u8; 64];

    req[0] = ICMP_ECHO_REQUEST;
    req[1] = 0;
    req[2] = 0;
    req[3] = 0;
    req[4..6].copy_from_slice(&PING_ID.to_be_bytes());
    req[6..8].copy_from_slice(&seq.to_be_bytes());
    req[8..len].copy_from_slice(payload);

    let csum = checksum::internet_checksum(&req[..len]);
    req[2..4].copy_from_slice(&csum.to_be_bytes());

    ksprintln!(
        "[net][icmp] echo request to {}.{}.{}.{} id={:#06x} seq={}",
        dst_ip[0],
        dst_ip[1],
        dst_ip[2],
        dst_ip[3],
        PING_ID,
        seq
    );

    ipv4::send(dst_mac, dst_ip, ICMP_PROTO, &req[..len])
}

fn send_echo_reply(dst_mac: [u8; 6], dst_ip: [u8; 4], req: &[u8]) -> bool {
    if req.len() > 1400 {
        return false;
    }

    let mut reply = [0u8; 1500];
    let len = req.len();

    reply[..len].copy_from_slice(req);
    reply[0] = ICMP_ECHO_REPLY;
    reply[1] = 0;
    reply[2] = 0;
    reply[3] = 0;

    let csum = checksum::internet_checksum(&reply[..len]);
    reply[2..4].copy_from_slice(&csum.to_be_bytes());

    ksprintln!(
        "[net][icmp] echo reply to {}.{}.{}.{}",
        dst_ip[0],
        dst_ip[1],
        dst_ip[2],
        dst_ip[3]
    );

    ipv4::send(dst_mac, dst_ip, ICMP_PROTO, &reply[..len])
}
