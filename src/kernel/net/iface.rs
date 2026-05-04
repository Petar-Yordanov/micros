use crate::kernel::drivers::virtio::net;
use crate::ksprintln;

pub const IPV4_ADDR: [u8; 4] = [10, 0, 2, 15];
pub const IPV4_NETMASK: [u8; 4] = [255, 255, 255, 0];
pub const IPV4_GATEWAY: [u8; 4] = [10, 0, 2, 2];

pub fn init() {
    if !net::is_ready() {
        ksprintln!("[net] no NIC available");
        return;
    }

    if let Some(mac) = mac_addr() {
        ksprintln!(
            "[net] iface up ip={}.{}.{}.{} mac={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            IPV4_ADDR[0],
            IPV4_ADDR[1],
            IPV4_ADDR[2],
            IPV4_ADDR[3],
            mac[0],
            mac[1],
            mac[2],
            mac[3],
            mac[4],
            mac[5]
        );
    } else {
        ksprintln!(
            "[net] iface up ip={}.{}.{}.{} mac=none",
            IPV4_ADDR[0],
            IPV4_ADDR[1],
            IPV4_ADDR[2],
            IPV4_ADDR[3]
        );
    }
}

pub fn is_ready() -> bool {
    net::is_ready()
}

pub fn mac_addr() -> Option<[u8; 6]> {
    net::mac_addr()
}

pub fn ipv4_addr() -> [u8; 4] {
    IPV4_ADDR
}

pub fn netmask() -> [u8; 4] {
    IPV4_NETMASK
}

pub fn gateway() -> [u8; 4] {
    IPV4_GATEWAY
}

pub fn link_up() -> bool {
    net::link_up()
}

pub fn mtu() -> u16 {
    net::mtu().unwrap_or(1500)
}

pub fn recv_frame(out: &mut [u8]) -> Option<usize> {
    net::recv_frame(out)
}

pub fn send_frame(frame: &[u8]) -> bool {
    net::send_frame(frame)
}

pub fn is_our_ipv4(ip: [u8; 4]) -> bool {
    ip == IPV4_ADDR
}

pub fn is_our_mac(mac: [u8; 6]) -> bool {
    match mac_addr() {
        Some(ours) => mac == ours,
        None => false,
    }
}

pub fn is_broadcast_mac(mac: [u8; 6]) -> bool {
    mac == [0xff; 6]
}
