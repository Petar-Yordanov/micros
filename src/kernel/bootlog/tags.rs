pub fn progress_pct(done: usize, total: usize) -> usize {
    if total == 0 {
        0
    } else {
        core::cmp::min(100, (done.saturating_mul(100)) / total)
    }
}

pub fn progress_label(done: usize, total: usize, out: &mut [u8; 4]) -> &str {
    let pct = progress_pct(done, total);

    if pct == 100 {
        out[0] = b'1';
        out[1] = b'0';
        out[2] = b'0';
        out[3] = b'%';
        return core::str::from_utf8(&out[..4]).unwrap();
    }

    if pct >= 10 {
        out[0] = b'0' + ((pct / 10) as u8);
        out[1] = b'0' + ((pct % 10) as u8);
        out[2] = b'%';
        return core::str::from_utf8(&out[..3]).unwrap();
    }

    out[0] = b'0' + (pct as u8);
    out[1] = b'%';
    core::str::from_utf8(&out[..2]).unwrap()
}

pub fn split_tag(s: &str) -> (&str, &str) {
    if let Some(end) = s.find(']') {
        if s.starts_with('[') {
            let tag = &s[..=end];
            let rest = s[end + 1..].trim_start();
            return (tag, rest);
        }
    }
    ("[boot]", s)
}

pub fn tag_color(tag: &str) -> u32 {
    match tag {
        "[boot]" => 0x7AA2F7,
        "[mm]" => 0x9ECE6A,
        "[fs]" => 0x7DCFFF,
        "[virtio]" => 0xBB9AF7,
        "[sched]" => 0xE0AF68,
        "[init]" => 0x2AC3DE,
        "[warn]" => 0xEBCB8B,
        "[err]" => 0xF7768E,
        "[cpu]" => 0x73DACA,
        "[rtc]" => 0xC0CAF5,
        "[test]" => 0xA3BE8C,
        "[apic]" => 0xB48EAD,
        "[handoff]" => 0xF0A6CA,
        _ => 0x89DDFF,
    }
}
