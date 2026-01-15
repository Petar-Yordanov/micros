#![allow(dead_code)]

extern crate alloc;
use alloc::string::{String, ToString};

use super::error::VfsError;

pub fn normalize_path(p: &str) -> Result<String, VfsError> {
    if p.is_empty() {
        return Err(VfsError::BadPath);
    }

    let mut out = String::new();
    if !p.starts_with('/') {
        out.push('/');
    }
    out.push_str(p);

    let mut collapsed = String::new();
    let mut last_is_slash = false;
    for b in out.bytes() {
        let is_slash = b == b'/';
        if is_slash {
            if !last_is_slash {
                collapsed.push('/');
            }
        } else {
            collapsed.push(b as char);
        }
        last_is_slash = is_slash;
    }

    if collapsed.len() > 1 && collapsed.ends_with('/') {
        collapsed.pop();
    }
    Ok(collapsed)
}

pub fn normalize_mountpoint(mp: &str) -> Result<String, VfsError> {
    let mut s = normalize_path(mp)?;
    if s.len() > 1 && s.ends_with('/') {
        s.pop();
    }
    Ok(s)
}

pub fn strip_mount_prefix(path: &str, mp: &str) -> String {
    if mp == "/" {
        return path.to_string();
    }
    let tail = &path[mp.len()..];
    if tail.is_empty() {
        "/".to_string()
    } else if tail.starts_with('/') {
        tail.to_string()
    } else {
        let mut s = String::from("/");
        s.push_str(tail);
        s
    }
}
