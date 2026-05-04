pub fn internet_checksum(data: &[u8]) -> u16 {
    let mut sum: u32 = 0;
    let mut i = 0usize;

    while i + 1 < data.len() {
        let word = ((data[i] as u16) << 8) | data[i + 1] as u16;
        sum = sum.wrapping_add(word as u32);
        i += 2;
    }

    if i < data.len() {
        sum = sum.wrapping_add((data[i] as u32) << 8);
    }

    while (sum >> 16) != 0 {
        sum = (sum & 0xffff).wrapping_add(sum >> 16);
    }

    !(sum as u16)
}
