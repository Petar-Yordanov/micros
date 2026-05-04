extern crate alloc;

use alloc::vec::Vec;
use core::sync::atomic::{AtomicU16, AtomicU32, AtomicU64, Ordering};
use spin::{Mutex, Once};

use crate::kernel::net::{arp, ethernet, iface, ipv4};
use crate::ksprintln;

const TCP_HEADER_LEN: usize = 20;

const TCP_FIN: u16 = 0x01;
const TCP_SYN: u16 = 0x02;
const TCP_RST: u16 = 0x04;
const TCP_PSH: u16 = 0x08;
const TCP_ACK: u16 = 0x10;

const TCP_WINDOW: u16 = 4096;
const MAX_TCP_SEGMENT: usize = 1200;
const DEFAULT_CONNECT_POLLS: u32 = 120_000;
const DEFAULT_IO_POLLS: u32 = 40_000;
const SYN_RETRANSMIT_EVERY_POLLS: u32 = 4096;

static NEXT_FD: AtomicU64 = AtomicU64::new(1);
static NEXT_PORT: AtomicU16 = AtomicU16::new(49152);
static NEXT_SEQ: AtomicU32 = AtomicU32::new(0x1000_0000);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TcpState {
    SynSent,
    Established,
    CloseWait,
    Closed,
}

struct TcpConnection {
    fd: u64,
    state: TcpState,

    local_port: u16,
    remote_port: u16,
    remote_ip: [u8; 4],
    remote_mac: [u8; 6],

    iss: u32,
    snd_nxt: u32,
    rcv_nxt: u32,

    pending_ack: bool,
    rx: Vec<u8>,
}

struct TcpStack {
    conns: Vec<TcpConnection>,
}

static TCP_STACK: Once<Mutex<TcpStack>> = Once::new();

fn stack() -> &'static Mutex<TcpStack> {
    TCP_STACK.call_once(|| Mutex::new(TcpStack { conns: Vec::new() }))
}

#[derive(Clone, Copy)]
struct TcpHeader {
    src_port: u16,
    dst_port: u16,
    seq: u32,
    ack: u32,
    data_offset: usize,
    flags: u16,
    window: u16,
}

#[derive(Clone, Copy)]
struct TcpTxInfo {
    remote_mac: [u8; 6],
    remote_ip: [u8; 4],
    local_port: u16,
    remote_port: u16,
    seq: u32,
    ack: u32,
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

fn parse_tcp(buf: &[u8]) -> Option<(TcpHeader, &[u8])> {
    if buf.len() < TCP_HEADER_LEN {
        return None;
    }

    let src_port = u16::from_be_bytes([buf[0], buf[1]]);
    let dst_port = u16::from_be_bytes([buf[2], buf[3]]);
    let seq = u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]);
    let ack = u32::from_be_bytes([buf[8], buf[9], buf[10], buf[11]]);
    let data_offset = ((buf[12] >> 4) as usize) * 4;
    let flags = buf[13] as u16;
    let window = u16::from_be_bytes([buf[14], buf[15]]);

    if data_offset < TCP_HEADER_LEN || data_offset > buf.len() {
        return None;
    }

    Some((
        TcpHeader {
            src_port,
            dst_port,
            seq,
            ack,
            data_offset,
            flags,
            window,
        },
        &buf[data_offset..],
    ))
}

fn tcp_checksum(src_ip: [u8; 4], dst_ip: [u8; 4], segment: &[u8]) -> u16 {
    let mut pseudo = [0u8; 12];

    pseudo[0..4].copy_from_slice(&src_ip);
    pseudo[4..8].copy_from_slice(&dst_ip);
    pseudo[8] = 0;
    pseudo[9] = ipv4::IPV4_PROTO_TCP;
    pseudo[10..12].copy_from_slice(&(segment.len() as u16).to_be_bytes());

    let mut sum: u32 = 0;

    add_words(&mut sum, &pseudo);
    add_words(&mut sum, segment);

    while (sum >> 16) != 0 {
        sum = (sum & 0xffff).wrapping_add(sum >> 16);
    }

    !(sum as u16)
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

fn send_tcp_packet(
    dst_mac: [u8; 6],
    dst_ip: [u8; 4],
    src_port: u16,
    dst_port: u16,
    seq: u32,
    ack: u32,
    flags: u16,
    payload: &[u8],
) -> bool {
    if payload.len() > MAX_TCP_SEGMENT {
        return false;
    }

    let src_ip = iface::ipv4_addr();
    let total_len = TCP_HEADER_LEN + payload.len();

    let mut tcp = [0u8; 1500];
    if total_len > tcp.len() {
        return false;
    }

    tcp[0..2].copy_from_slice(&src_port.to_be_bytes());
    tcp[2..4].copy_from_slice(&dst_port.to_be_bytes());
    tcp[4..8].copy_from_slice(&seq.to_be_bytes());
    tcp[8..12].copy_from_slice(&ack.to_be_bytes());
    tcp[12] = 5u8 << 4;
    tcp[13] = flags as u8;
    tcp[14..16].copy_from_slice(&TCP_WINDOW.to_be_bytes());
    tcp[16..18].copy_from_slice(&0u16.to_be_bytes());
    tcp[18..20].copy_from_slice(&0u16.to_be_bytes());

    tcp[TCP_HEADER_LEN..total_len].copy_from_slice(payload);

    let csum = tcp_checksum(src_ip, dst_ip, &tcp[..total_len]);
    tcp[16..18].copy_from_slice(&csum.to_be_bytes());

    ksprintln!(
        "[net][tcp][tx] {}.{}.{}.{}:{} flags={:#04x} seq={} ack={} len={}",
        dst_ip[0],
        dst_ip[1],
        dst_ip[2],
        dst_ip[3],
        dst_port,
        flags,
        seq,
        ack,
        payload.len()
    );

    ipv4::send(dst_mac, dst_ip, ipv4::IPV4_PROTO_TCP, &tcp[..total_len])
}

fn pending_ack_info(fd: u64) -> Option<TcpTxInfo> {
    let mut guard = stack().lock();

    let c = guard.conns.iter_mut().find(|c| c.fd == fd)?;

    if !c.pending_ack {
        return None;
    }

    c.pending_ack = false;

    Some(TcpTxInfo {
        remote_mac: c.remote_mac,
        remote_ip: c.remote_ip,
        local_port: c.local_port,
        remote_port: c.remote_port,
        seq: c.snd_nxt,
        ack: c.rcv_nxt,
    })
}

fn flush_pending_ack(fd: u64) -> bool {
    let tx = match pending_ack_info(fd) {
        Some(v) => v,
        None => return true,
    };

    send_tcp_packet(
        tx.remote_mac,
        tx.remote_ip,
        tx.local_port,
        tx.remote_port,
        tx.seq,
        tx.ack,
        TCP_ACK,
        &[],
    )
}

pub fn connect(dst_ip: [u8; 4], dst_port: u16, timeout_polls: u32) -> Result<u64, i64> {
    if dst_ip == [0, 0, 0, 0] || dst_port == 0 {
        return Err(-micros_abi::errno::EINVAL);
    }

    if !iface::is_ready() {
        return Err(-micros_abi::errno::ENODEV);
    }

    if !iface::link_up() {
        return Err(-micros_abi::errno::EIO);
    }

    let polls = if timeout_polls == 0 {
        DEFAULT_CONNECT_POLLS
    } else {
        timeout_polls
    };

    let hop_ip = next_hop_for(dst_ip);
    let dst_mac = match resolve_mac(hop_ip, polls) {
        Some(v) => v,
        None => return Err(-micros_abi::errno::EAGAIN),
    };

    let fd = NEXT_FD.fetch_add(1, Ordering::Relaxed);
    let local_port = NEXT_PORT.fetch_add(1, Ordering::Relaxed);
    let iss = NEXT_SEQ.fetch_add(0x1000, Ordering::Relaxed);
    let snd_nxt = iss.wrapping_add(1);

    {
        let mut guard = stack().lock();

        guard.conns.push(TcpConnection {
            fd,
            state: TcpState::SynSent,
            local_port,
            remote_port: dst_port,
            remote_ip: dst_ip,
            remote_mac: dst_mac,
            iss,
            snd_nxt,
            rcv_nxt: 0,
            pending_ack: false,
            rx: Vec::new(),
        });
    }

    if !send_tcp_packet(dst_mac, dst_ip, local_port, dst_port, iss, 0, TCP_SYN, &[]) {
        drop_conn(fd);
        return Err(-micros_abi::errno::EIO);
    }

    for poll in 0..polls {
        ethernet::poll_once();

        if is_established(fd) {
            if !flush_pending_ack(fd) {
                drop_conn(fd);
                return Err(-micros_abi::errno::EIO);
            }

            ksprintln!(
                "[net][tcp] connected fd={} {}.{}.{}.{}:{} local_port={}",
                fd,
                dst_ip[0],
                dst_ip[1],
                dst_ip[2],
                dst_ip[3],
                dst_port,
                local_port
            );

            return Ok(fd);
        }

        if is_closed(fd) {
            drop_conn(fd);
            return Err(-micros_abi::errno::EIO);
        }

        if poll != 0 && poll % SYN_RETRANSMIT_EVERY_POLLS == 0 {
            if let Some((mac, ip, port, seq)) = syn_retransmit_info(fd) {
                ksprintln!(
                    "[net][tcp] retransmit SYN fd={} {}.{}.{}.{}:{} local_port={}",
                    fd,
                    ip[0],
                    ip[1],
                    ip[2],
                    ip[3],
                    dst_port,
                    port
                );

                let _ = send_tcp_packet(mac, ip, port, dst_port, seq, 0, TCP_SYN, &[]);
            }
        }

        core::hint::spin_loop();
    }

    drop_conn(fd);
    Err(-micros_abi::errno::EAGAIN)
}

fn syn_retransmit_info(fd: u64) -> Option<([u8; 6], [u8; 4], u16, u32)> {
    let guard = stack().lock();

    let c = guard.conns.iter().find(|c| c.fd == fd)?;

    if c.state != TcpState::SynSent {
        return None;
    }

    Some((c.remote_mac, c.remote_ip, c.local_port, c.iss))
}

fn drop_conn(fd: u64) {
    let mut guard = stack().lock();

    if let Some(pos) = guard.conns.iter().position(|c| c.fd == fd) {
        guard.conns.remove(pos);
    }
}

fn is_established(fd: u64) -> bool {
    let guard = stack().lock();

    guard
        .conns
        .iter()
        .find(|c| c.fd == fd)
        .map(|c| c.state == TcpState::Established)
        .unwrap_or(false)
}

fn is_closed(fd: u64) -> bool {
    let guard = stack().lock();

    guard
        .conns
        .iter()
        .find(|c| c.fd == fd)
        .map(|c| c.state == TcpState::Closed)
        .unwrap_or(true)
}

pub fn send(fd: u64, data: &[u8]) -> Result<usize, i64> {
    if data.is_empty() {
        return Err(-micros_abi::errno::EINVAL);
    }

    let _ = flush_pending_ack(fd);

    let n = core::cmp::min(data.len(), MAX_TCP_SEGMENT);

    let (dst_mac, dst_ip, local_port, remote_port, seq, ack) = {
        let mut guard = stack().lock();

        let c = match guard.conns.iter_mut().find(|c| c.fd == fd) {
            Some(v) => v,
            None => return Err(-micros_abi::errno::EINVAL),
        };

        if c.state != TcpState::Established && c.state != TcpState::CloseWait {
            return Err(-micros_abi::errno::EIO);
        }

        let seq = c.snd_nxt;
        c.snd_nxt = c.snd_nxt.wrapping_add(n as u32);

        (
            c.remote_mac,
            c.remote_ip,
            c.local_port,
            c.remote_port,
            seq,
            c.rcv_nxt,
        )
    };

    if !send_tcp_packet(
        dst_mac,
        dst_ip,
        local_port,
        remote_port,
        seq,
        ack,
        TCP_PSH | TCP_ACK,
        &data[..n],
    ) {
        return Err(-micros_abi::errno::EIO);
    }

    for _ in 0..2048 {
        ethernet::poll_once();
        let _ = flush_pending_ack(fd);
        core::hint::spin_loop();
    }

    Ok(n)
}

pub fn recv(fd: u64, out: &mut [u8]) -> Result<usize, i64> {
    if out.is_empty() {
        return Err(-micros_abi::errno::EINVAL);
    }

    for _ in 0..DEFAULT_IO_POLLS {
        {
            let mut guard = stack().lock();

            let c = match guard.conns.iter_mut().find(|c| c.fd == fd) {
                Some(v) => v,
                None => return Err(-micros_abi::errno::EINVAL),
            };

            if !c.rx.is_empty() {
                let n = core::cmp::min(out.len(), c.rx.len());

                for i in 0..n {
                    out[i] = c.rx[i];
                }

                c.rx.drain(0..n);
                return Ok(n);
            }

            if c.state == TcpState::CloseWait || c.state == TcpState::Closed {
                return Ok(0);
            }
        }

        ethernet::poll_once();
        let _ = flush_pending_ack(fd);
        core::hint::spin_loop();
    }

    Err(-micros_abi::errno::EAGAIN)
}

pub fn close(fd: u64) {
    let maybe_fin = {
        let mut guard = stack().lock();

        if let Some(pos) = guard.conns.iter().position(|c| c.fd == fd) {
            let c = &guard.conns[pos];

            let fin = if c.state == TcpState::Established || c.state == TcpState::CloseWait {
                Some((
                    c.remote_mac,
                    c.remote_ip,
                    c.local_port,
                    c.remote_port,
                    c.snd_nxt,
                    c.rcv_nxt,
                ))
            } else {
                None
            };

            guard.conns.remove(pos);
            fin
        } else {
            None
        }
    };

    if let Some((mac, ip, local_port, remote_port, seq, ack)) = maybe_fin {
        let _ = send_tcp_packet(
            mac,
            ip,
            local_port,
            remote_port,
            seq,
            ack,
            TCP_FIN | TCP_ACK,
            &[],
        );
    }
}

pub fn handle_packet(src_mac: [u8; 6], packet: &ipv4::Ipv4Packet<'_>) {
    let (hdr, payload) = match parse_tcp(packet.payload) {
        Some(v) => v,
        None => return,
    };

    let src_ip = packet.src;
    let dst_ip = packet.dst;

    let expected = tcp_checksum(src_ip, dst_ip, packet.payload);
    if expected != 0 {
        ksprintln!(
            "[net][tcp][rx] bad checksum from {}.{}.{}.{}:{}",
            src_ip[0],
            src_ip[1],
            src_ip[2],
            src_ip[3],
            hdr.src_port
        );
        return;
    }

    ksprintln!(
        "[net][tcp][rx] {}.{}.{}.{}:{} -> local:{} flags={:#04x} seq={} ack={} len={} off={} win={}",
        src_ip[0],
        src_ip[1],
        src_ip[2],
        src_ip[3],
        hdr.src_port,
        hdr.dst_port,
        hdr.flags,
        hdr.seq,
        hdr.ack,
        payload.len(),
        hdr.data_offset,
        hdr.window
    );

    let mut guard = stack().lock();

    let conn = match guard.conns.iter_mut().find(|c| {
        c.local_port == hdr.dst_port
            && c.remote_port == hdr.src_port
            && (c.remote_ip == src_ip || c.state == TcpState::SynSent)
    }) {
        Some(v) => v,
        None => {
            ksprintln!(
                "[net][tcp] no conn for src={}.{}.{}.{}:{} dst_port={} flags={:#04x} seq={} ack={}",
                src_ip[0],
                src_ip[1],
                src_ip[2],
                src_ip[3],
                hdr.src_port,
                hdr.dst_port,
                hdr.flags,
                hdr.seq,
                hdr.ack
            );
            return;
        }
    };

    if conn.state == TcpState::SynSent {
        conn.remote_ip = src_ip;
    }

    conn.remote_mac = src_mac;

    if (hdr.flags & TCP_RST) != 0 {
        conn.state = TcpState::Closed;
        return;
    }

    match conn.state {
        TcpState::SynSent => {
            let syn_ack = (hdr.flags & TCP_SYN) != 0 && (hdr.flags & TCP_ACK) != 0;

            ksprintln!(
                "[net][tcp] SYN-SENT check fd={} local={} remote={}.{}.{}.{}:{} hdr_ack={} snd_nxt={} syn_ack={}",
                conn.fd,
                conn.local_port,
                conn.remote_ip[0],
                conn.remote_ip[1],
                conn.remote_ip[2],
                conn.remote_ip[3],
                conn.remote_port,
                hdr.ack,
                conn.snd_nxt,
                syn_ack
            );

            if syn_ack && hdr.ack == conn.snd_nxt {
                conn.rcv_nxt = hdr.seq.wrapping_add(1);
                conn.state = TcpState::Established;
                conn.pending_ack = true;

                ksprintln!(
                    "[net][tcp] fd={} SYN-ACK accepted snd_nxt={} rcv_nxt={}",
                    conn.fd,
                    conn.snd_nxt,
                    conn.rcv_nxt
                );
            }
        }

        TcpState::Established | TcpState::CloseWait => {
            if !payload.is_empty() && hdr.seq == conn.rcv_nxt {
                conn.rx.extend_from_slice(payload);
                conn.rcv_nxt = conn.rcv_nxt.wrapping_add(payload.len() as u32);
                conn.pending_ack = true;

                ksprintln!(
                    "[net][tcp] fd={} buffered {} bytes rcv_nxt={}",
                    conn.fd,
                    payload.len(),
                    conn.rcv_nxt
                );
            }

            if (hdr.flags & TCP_FIN) != 0 {
                conn.rcv_nxt = conn.rcv_nxt.wrapping_add(1);
                conn.state = TcpState::CloseWait;
                conn.pending_ack = true;

                ksprintln!("[net][tcp] fd={} FIN received", conn.fd);
            }
        }

        TcpState::Closed => {}
    }
}
