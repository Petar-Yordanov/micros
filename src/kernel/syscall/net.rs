extern crate alloc;

use alloc::vec::Vec;

use micros_abi::errno;
use micros_abi::types::{
    NetInfo, NetIoArgs, ABI_NET_INFO_F_HAS_IPV4, ABI_NET_INFO_F_HAS_MAC, ABI_NET_INFO_F_LINK_UP,
};

use crate::kernel::drivers::virtio::net;
use crate::kernel::mm::aspace::user_copy::{copy_from_user, copy_to_user};
use crate::kernel::net::{arp, ethernet, icmpv4, iface};
use crate::ksprintln;

use super::util::copy_user_struct;

const PING_RESULT_SENT: i64 = 0;
const PING_RESULT_ARP_SENT_ONLY: i64 = 1;

pub(super) fn sys_net_info(out_ptr: u64) -> i64 {
    if out_ptr == 0 {
        return -errno::EFAULT;
    }

    if !net::is_ready() {
        return -errno::ENODEV;
    }

    let mut info = NetInfo::default();
    info.mtu = net::mtu().unwrap_or(0) as u32;

    if net::link_up() {
        info.flags |= ABI_NET_INFO_F_LINK_UP;
    }

    if let Some(mac) = net::mac_addr() {
        info.flags |= ABI_NET_INFO_F_HAS_MAC;
        info.mac = mac;
    }

    info.flags |= ABI_NET_INFO_F_HAS_IPV4;
    info.ipv4 = iface::ipv4_addr();
    info.netmask = iface::netmask();
    info.gateway = iface::gateway();

    unsafe {
        if copy_to_user(
            out_ptr as *mut u8,
            &info as *const _ as *const u8,
            core::mem::size_of::<NetInfo>(),
        )
        .is_err()
        {
            return -errno::EFAULT;
        }
    }

    0
}

pub(super) fn sys_net_recv(args_ptr: u64) -> i64 {
    let args: NetIoArgs = match copy_user_struct(args_ptr) {
        Ok(v) => v,
        Err(e) => return e,
    };

    if args.buf_ptr == 0 {
        return -errno::EFAULT;
    }
    if args.buf_len == 0 {
        return -errno::EINVAL;
    }
    if !net::is_ready() {
        return -errno::ENODEV;
    }

    let cap = core::cmp::min(args.buf_len as usize, net::max_frame_len());
    if cap == 0 {
        return -errno::EINVAL;
    }

    let mut buf = Vec::<u8>::with_capacity(cap);
    unsafe { buf.set_len(cap) };

    let n = match net::recv_frame(&mut buf) {
        Some(v) => v,
        None => return -errno::EAGAIN,
    };

    if n == 0 {
        return 0;
    }

    unsafe {
        if copy_to_user(args.buf_ptr as *mut u8, buf.as_ptr(), n).is_err() {
            return -errno::EFAULT;
        }
    }

    n as i64
}

pub(super) fn sys_net_send(args_ptr: u64) -> i64 {
    let args: NetIoArgs = match copy_user_struct(args_ptr) {
        Ok(v) => v,
        Err(e) => return e,
    };

    if args.buf_ptr == 0 {
        return -errno::EFAULT;
    }
    if args.buf_len == 0 {
        return -errno::EINVAL;
    }
    if !net::is_ready() {
        return -errno::ENODEV;
    }

    let len = args.buf_len as usize;
    if len > net::max_frame_len() {
        return -errno::EINVAL;
    }

    let mut buf = Vec::<u8>::with_capacity(len);
    unsafe { buf.set_len(len) };

    unsafe {
        if copy_from_user(buf.as_mut_ptr(), args.buf_ptr as *const u8, len).is_err() {
            return -errno::EFAULT;
        }
    }

    if !net::send_frame(&buf) {
        return -errno::EIO;
    }

    len as i64
}

pub(super) fn sys_net_ping_gateway() -> i64 {
    if !net::is_ready() {
        return -errno::ENODEV;
    }

    if !iface::link_up() {
        return -errno::EIO;
    }

    let gateway_ip = iface::gateway();

    if gateway_ip == [0, 0, 0, 0] {
        return -errno::EINVAL;
    }

    if let Some(gateway_mac) = arp::lookup(gateway_ip) {
        ksprintln!(
            "[net][ping] gateway ARP cached: {}.{}.{}.{} -> {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            gateway_ip[0],
            gateway_ip[1],
            gateway_ip[2],
            gateway_ip[3],
            gateway_mac[0],
            gateway_mac[1],
            gateway_mac[2],
            gateway_mac[3],
            gateway_mac[4],
            gateway_mac[5]
        );

        return send_gateway_echo_and_poll(gateway_mac, gateway_ip);
    }

    ksprintln!(
        "[net][ping] gateway ARP miss, sending request for {}.{}.{}.{}",
        gateway_ip[0],
        gateway_ip[1],
        gateway_ip[2],
        gateway_ip[3],
    );

    if !arp::send_request(gateway_ip) {
        return -errno::EIO;
    }

    for poll in 0..512 {
        ethernet::poll_once();

        if let Some(gateway_mac) = arp::lookup(gateway_ip) {
            ksprintln!(
                "[net][ping] gateway ARP resolved after poll {}: {}.{}.{}.{} -> {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
                poll,
                gateway_ip[0],
                gateway_ip[1],
                gateway_ip[2],
                gateway_ip[3],
                gateway_mac[0],
                gateway_mac[1],
                gateway_mac[2],
                gateway_mac[3],
                gateway_mac[4],
                gateway_mac[5]
            );

            return send_gateway_echo_and_poll(gateway_mac, gateway_ip);
        }

        core::hint::spin_loop();
    }

    ksprintln!(
        "[net][ping] ARP request sent, gateway not resolved yet: {}.{}.{}.{}",
        gateway_ip[0],
        gateway_ip[1],
        gateway_ip[2],
        gateway_ip[3],
    );

    PING_RESULT_ARP_SENT_ONLY
}

fn send_gateway_echo_and_poll(gateway_mac: [u8; 6], gateway_ip: [u8; 4]) -> i64 {
    if !icmpv4::send_echo_request(gateway_mac, gateway_ip) {
        return -errno::EIO;
    }

    for poll in 0..1024 {
        if ethernet::poll_once() {
            if poll < 8 {
                ksprintln!("[net][ping] post-echo RX poll {}", poll);
            }
        }

        core::hint::spin_loop();
    }

    PING_RESULT_SENT
}

pub(super) fn sys_tcp_connect(args_ptr: u64) -> i64 {
    use micros_abi::types::TcpConnectArgs;

    let args: TcpConnectArgs = match copy_user_struct(args_ptr) {
        Ok(v) => v,
        Err(e) => return e,
    };

    match crate::kernel::net::tcp::connect(args.dst_ip, args.dst_port, args.timeout_polls) {
        Ok(fd) => fd as i64,
        Err(e) => e,
    }
}

pub(super) fn sys_tcp_send(args_ptr: u64) -> i64 {
    use micros_abi::types::TcpIoArgs;

    let args: TcpIoArgs = match copy_user_struct(args_ptr) {
        Ok(v) => v,
        Err(e) => return e,
    };

    if args.fd == 0 || args.buf_ptr == 0 || args.buf_len == 0 {
        return -errno::EINVAL;
    }

    let len = core::cmp::min(args.buf_len as usize, 1200);
    let mut buf = Vec::<u8>::with_capacity(len);
    unsafe { buf.set_len(len) };

    unsafe {
        if copy_from_user(buf.as_mut_ptr(), args.buf_ptr as *const u8, len).is_err() {
            return -errno::EFAULT;
        }
    }

    match crate::kernel::net::tcp::send(args.fd, &buf) {
        Ok(n) => n as i64,
        Err(e) => e,
    }
}

pub(super) fn sys_tcp_recv(args_ptr: u64) -> i64 {
    use micros_abi::types::TcpIoArgs;

    let args: TcpIoArgs = match copy_user_struct(args_ptr) {
        Ok(v) => v,
        Err(e) => return e,
    };

    if args.fd == 0 || args.buf_ptr == 0 || args.buf_len == 0 {
        return -errno::EINVAL;
    }

    let len = core::cmp::min(args.buf_len as usize, 4096);
    let mut buf = Vec::<u8>::with_capacity(len);
    unsafe { buf.set_len(len) };

    let n = match crate::kernel::net::tcp::recv(args.fd, &mut buf) {
        Ok(n) => n,
        Err(e) => return e,
    };

    if n == 0 {
        return 0;
    }

    unsafe {
        if copy_to_user(args.buf_ptr as *mut u8, buf.as_ptr(), n).is_err() {
            return -errno::EFAULT;
        }
    }

    n as i64
}

pub(super) fn sys_tcp_close(fd: u64) -> i64 {
    if fd == 0 {
        return -errno::EINVAL;
    }

    crate::kernel::net::tcp::close(fd);
    0
}

pub(super) fn sys_udp_send_to(args_ptr: u64) -> i64 {
    use micros_abi::types::UdpSendToArgs;

    let args: UdpSendToArgs = match copy_user_struct(args_ptr) {
        Ok(v) => v,
        Err(e) => return e,
    };

    if args.buf_ptr == 0 || args.buf_len == 0 {
        return -errno::EINVAL;
    }

    let len = core::cmp::min(args.buf_len as usize, 1472);
    let mut buf = Vec::<u8>::with_capacity(len);
    unsafe { buf.set_len(len) };

    unsafe {
        if copy_from_user(buf.as_mut_ptr(), args.buf_ptr as *const u8, len).is_err() {
            return -errno::EFAULT;
        }
    }

    match crate::kernel::net::udp::send_to(
        args.dst_ip,
        args.dst_port,
        args.src_port,
        &buf,
        args.timeout_polls,
    ) {
        Ok(n) => n as i64,
        Err(e) => e,
    }
}

pub(super) fn sys_udp_recv_from(args_ptr: u64) -> i64 {
    use micros_abi::types::UdpRecvFromArgs;

    let mut args: UdpRecvFromArgs = match copy_user_struct(args_ptr) {
        Ok(v) => v,
        Err(e) => return e,
    };

    if args.local_port == 0 || args.out_ptr == 0 || args.out_cap == 0 {
        return -errno::EINVAL;
    }

    let len = core::cmp::min(args.out_cap as usize, 4096);
    let mut buf = Vec::<u8>::with_capacity(len);
    unsafe { buf.set_len(len) };

    let (src_ip, src_port, n) =
        match crate::kernel::net::udp::recv_from(args.local_port, &mut buf, args.timeout_polls) {
            Ok(v) => v,
            Err(e) => return e,
        };

    args.src_ip = src_ip;
    args.src_port = src_port;

    unsafe {
        if copy_to_user(args.out_ptr as *mut u8, buf.as_ptr(), n).is_err() {
            return -errno::EFAULT;
        }

        if copy_to_user(
            args_ptr as *mut u8,
            &args as *const _ as *const u8,
            core::mem::size_of::<UdpRecvFromArgs>(),
        )
        .is_err()
        {
            return -errno::EFAULT;
        }
    }

    n as i64
}
