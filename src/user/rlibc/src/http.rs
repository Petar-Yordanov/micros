extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::dns;
use crate::errno::Errno;
use crate::net::TcpStream;

pub fn get(host: &str, path: &str) -> Result<Vec<u8>, Errno> {
    let ip = dns::resolve_a(host)?;
    get_ipv4(host, ip, path)
}

pub fn get_url(url: &str) -> Result<Vec<u8>, Errno> {
    let mut host = String::new();
    let mut path = String::new();

    parse_http_url(url, &mut host, &mut path);

    if host.is_empty() {
        return Err(Errno(micros_abi::errno::EINVAL as i32));
    }

    get(&host, &path)
}

pub fn get_ipv4(host: &str, dst_ip: [u8; 4], path: &str) -> Result<Vec<u8>, Errno> {
    let mut stream = TcpStream::connect_ipv4(dst_ip, 80)?;

    let req = format!(
        "GET {} HTTP/1.0\r\nHost: {}\r\nUser-Agent: MicrOS64\r\nAccept: text/html,*/*\r\nConnection: close\r\n\r\n",
        normalize_path(path),
        host
    );

    let bytes = req.as_bytes();
    let mut sent = 0usize;

    while sent < bytes.len() {
        let n = stream.send(&bytes[sent..])?;
        if n == 0 {
            break;
        }

        sent += n;
    }

    let mut out = Vec::new();
    let mut buf = [0u8; 1024];

    loop {
        match stream.recv(&mut buf) {
            Ok(0) => break,
            Ok(n) => out.extend_from_slice(&buf[..n]),
            Err(e) => {
                if !out.is_empty() {
                    break;
                }

                return Err(e);
            }
        }

        if out.len() >= 32 * 1024 {
            break;
        }
    }

    stream.close();
    Ok(out)
}

pub fn get_gateway_root() -> Result<Vec<u8>, Errno> {
    get_ipv4("10.0.2.2", [10, 0, 2, 2], "/")
}

fn parse_http_url(url: &str, host_out: &mut String, path_out: &mut String) {
    host_out.clear();
    path_out.clear();

    let mut s = trim_ascii(url);

    if starts_with_ci(s.as_bytes(), b"http://") {
        s = &s[7..];
    } else if starts_with_ci(s.as_bytes(), b"https://") {
        return;
    }

    let bytes = s.as_bytes();
    let mut slash = bytes.len();

    for (i, b) in bytes.iter().enumerate() {
        if *b == b'/' {
            slash = i;
            break;
        }
    }

    let host_part = &s[..slash];
    let path_part = if slash < bytes.len() {
        &s[slash..]
    } else {
        "/"
    };

    for b in host_part.bytes() {
        if b == b':' {
            break;
        }

        if is_host_char(b) {
            host_out.push(to_ascii_lower(b) as char);
        }
    }

    if path_part.is_empty() {
        path_out.push('/');
    } else {
        path_out.push_str(path_part);
    }
}

fn normalize_path(path: &str) -> &str {
    if path.is_empty() {
        "/"
    } else {
        path
    }
}

fn trim_ascii(s: &str) -> &str {
    let bytes = s.as_bytes();
    let mut start = 0usize;
    let mut end = bytes.len();

    while start < end && is_ascii_space(bytes[start]) {
        start += 1;
    }

    while end > start && is_ascii_space(bytes[end - 1]) {
        end -= 1;
    }

    &s[start..end]
}

fn is_ascii_space(b: u8) -> bool {
    b == b' ' || b == b'\n' || b == b'\r' || b == b'\t'
}

fn is_host_char(b: u8) -> bool {
    (b'a' <= b && b <= b'z')
        || (b'A' <= b && b <= b'Z')
        || (b'0' <= b && b <= b'9')
        || b == b'.'
        || b == b'-'
}

fn starts_with_ci(haystack: &[u8], needle: &[u8]) -> bool {
    if haystack.len() < needle.len() {
        return false;
    }

    for i in 0..needle.len() {
        if to_ascii_lower(haystack[i]) != to_ascii_lower(needle[i]) {
            return false;
        }
    }

    true
}

fn to_ascii_lower(b: u8) -> u8 {
    if b'A' <= b && b <= b'Z' {
        b + 32
    } else {
        b
    }
}
