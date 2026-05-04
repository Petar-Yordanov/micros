use micros_abi::sysnr;
use micros_abi::types::{
    NetInfo, NetIoArgs, TcpConnectArgs, TcpIoArgs, UdpRecvFromArgs, UdpSendToArgs,
};

use crate::errno::{cvt, Errno};
use crate::syscall::syscall1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PingGatewayResult {
    EchoRequestSent,
    ArpRequestSent,
}

pub struct TcpStream {
    fd: u64,
}

impl TcpStream {
    pub fn connect_ipv4(dst_ip: [u8; 4], dst_port: u16) -> Result<Self, Errno> {
        Self::connect_ipv4_with_timeout(dst_ip, dst_port, 0)
    }

    pub fn connect_ipv4_with_timeout(
        dst_ip: [u8; 4],
        dst_port: u16,
        timeout_polls: u32,
    ) -> Result<Self, Errno> {
        let args = TcpConnectArgs {
            dst_ip,
            dst_port,
            _pad0: 0,
            timeout_polls,
        };

        let fd = cvt(syscall1(sysnr::SYS_TCP_CONNECT, &args as *const _ as u64))?;

        Ok(Self { fd: fd as u64 })
    }

    pub fn send(&mut self, buf: &[u8]) -> Result<usize, Errno> {
        let args = TcpIoArgs {
            fd: self.fd,
            buf_ptr: buf.as_ptr() as u64,
            buf_len: buf.len() as u64,
        };

        let n = cvt(syscall1(sysnr::SYS_TCP_SEND, &args as *const _ as u64))?;
        Ok(n as usize)
    }

    pub fn recv(&mut self, buf: &mut [u8]) -> Result<usize, Errno> {
        let args = TcpIoArgs {
            fd: self.fd,
            buf_ptr: buf.as_mut_ptr() as u64,
            buf_len: buf.len() as u64,
        };

        let n = cvt(syscall1(sysnr::SYS_TCP_RECV, &args as *const _ as u64))?;
        Ok(n as usize)
    }

    pub fn close(&mut self) {
        let _ = cvt(syscall1(sysnr::SYS_TCP_CLOSE, self.fd));
        self.fd = 0;
    }
}

impl Drop for TcpStream {
    fn drop(&mut self) {
        if self.fd != 0 {
            self.close();
        }
    }
}

#[inline(always)]
pub fn info() -> Result<NetInfo, Errno> {
    let mut out = NetInfo::default();
    cvt(syscall1(sysnr::SYS_NET_INFO, &mut out as *mut _ as u64))?;
    Ok(out)
}

#[inline(always)]
pub fn recv(buf: &mut [u8]) -> Result<usize, Errno> {
    let args = NetIoArgs {
        buf_ptr: buf.as_mut_ptr() as u64,
        buf_len: buf.len() as u64,
    };
    let n = cvt(syscall1(sysnr::SYS_NET_RECV, &args as *const _ as u64))?;
    Ok(n as usize)
}

#[inline(always)]
pub fn send(frame: &[u8]) -> Result<usize, Errno> {
    let args = NetIoArgs {
        buf_ptr: frame.as_ptr() as u64,
        buf_len: frame.len() as u64,
    };
    let n = cvt(syscall1(sysnr::SYS_NET_SEND, &args as *const _ as u64))?;
    Ok(n as usize)
}

#[inline(always)]
pub fn ping_gateway() -> Result<PingGatewayResult, Errno> {
    let r = cvt(syscall1(sysnr::SYS_NET_PING_GATEWAY, 0))?;

    if r == 0 {
        Ok(PingGatewayResult::EchoRequestSent)
    } else {
        Ok(PingGatewayResult::ArpRequestSent)
    }
}

pub fn udp_send_to(
    dst_ip: [u8; 4],
    dst_port: u16,
    src_port: u16,
    data: &[u8],
) -> Result<usize, Errno> {
    let args = UdpSendToArgs {
        dst_ip,
        dst_port,
        src_port,
        buf_ptr: data.as_ptr() as u64,
        buf_len: data.len() as u64,
        timeout_polls: 0,
        _pad0: 0,
    };

    let n = cvt(syscall1(sysnr::SYS_UDP_SEND_TO, &args as *const _ as u64))?;
    Ok(n as usize)
}

pub fn udp_recv_from(local_port: u16, out: &mut [u8]) -> Result<([u8; 4], u16, usize), Errno> {
    let mut args = UdpRecvFromArgs {
        local_port,
        _pad0: 0,
        timeout_polls: 0,
        out_ptr: out.as_mut_ptr() as u64,
        out_cap: out.len() as u64,
        src_ip: [0; 4],
        src_port: 0,
        _pad1: 0,
    };

    let n = cvt(syscall1(
        sysnr::SYS_UDP_RECV_FROM,
        &mut args as *mut _ as u64,
    ))?;

    Ok((args.src_ip, args.src_port, n as usize))
}

#[inline(always)]
pub fn ipv4_addr() -> Result<[u8; 4], Errno> {
    Ok(info()?.ipv4)
}

#[inline(always)]
pub fn gateway() -> Result<[u8; 4], Errno> {
    Ok(info()?.gateway)
}

#[inline(always)]
pub fn netmask() -> Result<[u8; 4], Errno> {
    Ok(info()?.netmask)
}
