extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum IcoError {
    TooShort,
    BadHeader,
    BadEntry,
    UnsupportedBmpHeader,
    UnsupportedBitDepth,
    UnsupportedCompression,
    BadDimensions,
    OutOfBounds,
    NoSupportedEntry,
}

pub struct DecodedIcon {
    pub width: usize,
    pub height: usize,
    pub pixels_rgba: Vec<u8>,
}

#[derive(Clone, Copy)]
struct DirEntry {
    width: usize,
    bpp: u16,
    bytes_in_res: usize,
    image_offset: usize,
}

const PNG_SIG: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

pub fn decode_best_ico(data: &[u8], preferred_size: usize) -> Result<DecodedIcon, IcoError> {
    let entries = parse_dir_entries(data)?;
    let mut best: Option<(usize, DirEntry)> = None;

    for entry in entries.iter().copied() {
        let blob = slice_at(data, entry.image_offset, entry.bytes_in_res)?;
        if blob.len() >= 8 && &blob[..8] == PNG_SIG {
            continue;
        }

        if !is_supported_bpp(entry.bpp) {
            continue;
        }

        let score = score_entry(entry, preferred_size);
        match best {
            None => best = Some((score, entry)),
            Some((best_score, _)) if score > best_score => best = Some((score, entry)),
            _ => {}
        }
    }

    let (_, entry) = best.ok_or(IcoError::NoSupportedEntry)?;
    decode_bmp_ico_entry(data, entry)
}

fn is_supported_bpp(bpp: u16) -> bool {
    bpp == 32 || bpp == 24
}

fn score_entry(entry: DirEntry, preferred_size: usize) -> usize {
    let size_penalty = if entry.width > preferred_size {
        entry.width - preferred_size
    } else {
        preferred_size - entry.width
    };

    let bpp_bonus = match entry.bpp {
        32 => 1000,
        24 => 500,
        _ => 0,
    };

    10_000usize
        .saturating_sub(size_penalty * 10)
        .saturating_add(bpp_bonus)
        .saturating_add(entry.width.min(256))
}

fn parse_dir_entries(data: &[u8]) -> Result<Vec<DirEntry>, IcoError> {
    if data.len() < 6 {
        return Err(IcoError::TooShort);
    }

    let reserved = le_u16(data, 0)?;
    let kind = le_u16(data, 2)?;
    let count = le_u16(data, 4)? as usize;

    if reserved != 0 || kind != 1 || count == 0 {
        return Err(IcoError::BadHeader);
    }

    let mut entries = Vec::with_capacity(count);
    let mut off = 6usize;

    for _ in 0..count {
        if off + 16 > data.len() {
            return Err(IcoError::BadEntry);
        }

        let w = if data[off] == 0 { 256 } else { data[off] as usize };
        let _color_count = data[off + 2];
        let _reserved = data[off + 3];
        let _planes = le_u16(data, off + 4)?;
        let bpp = le_u16(data, off + 6)?;
        let bytes_in_res = le_u32(data, off + 8)? as usize;
        let image_offset = le_u32(data, off + 12)? as usize;

        entries.push(DirEntry {
            width: w,
            bpp,
            bytes_in_res,
            image_offset,
        });

        off += 16;
    }

    Ok(entries)
}

fn decode_bmp_ico_entry(data: &[u8], entry: DirEntry) -> Result<DecodedIcon, IcoError> {
    let blob = slice_at(data, entry.image_offset, entry.bytes_in_res)?;

    if blob.len() < 40 {
        return Err(IcoError::UnsupportedBmpHeader);
    }

    let dib_size = le_u32(blob, 0)? as usize;
    if dib_size < 40 || blob.len() < dib_size {
        return Err(IcoError::UnsupportedBmpHeader);
    }

    let bmp_w = le_i32(blob, 4)?;
    let bmp_h_total = le_i32(blob, 8)?;
    let _planes = le_u16(blob, 12)?;
    let bpp = le_u16(blob, 14)?;
    let compression = le_u32(blob, 16)?;

    if compression != 0 {
        return Err(IcoError::UnsupportedCompression);
    }
    if bpp != 32 && bpp != 24 {
        return Err(IcoError::UnsupportedBitDepth);
    }
    if bmp_w <= 0 || bmp_h_total == 0 {
        return Err(IcoError::BadDimensions);
    }

    let width = bmp_w as usize;
    let total_h_abs = bmp_h_total.unsigned_abs() as usize;
    if total_h_abs < 2 {
        return Err(IcoError::BadDimensions);
    }
    let height = total_h_abs / 2;
    if height == 0 {
        return Err(IcoError::BadDimensions);
    }

    let xor_stride = row_stride(width, bpp as usize);
    let xor_size = xor_stride.checked_mul(height).ok_or(IcoError::OutOfBounds)?;
    let xor_off = dib_size;
    let xor_end = xor_off.checked_add(xor_size).ok_or(IcoError::OutOfBounds)?;
    if xor_end > blob.len() {
        return Err(IcoError::OutOfBounds);
    }

    let and_stride = ((width + 31) / 32) * 4;
    let and_size = and_stride.checked_mul(height).ok_or(IcoError::OutOfBounds)?;
    let and_off = xor_end;
    let and_end = and_off.checked_add(and_size).ok_or(IcoError::OutOfBounds)?;
    if and_off > blob.len() {
        return Err(IcoError::OutOfBounds);
    }

    let and_mask = if and_end <= blob.len() {
        &blob[and_off..and_end]
    } else {
        &[]
    };

    let xor = &blob[xor_off..xor_end];
    let mut out = vec![0u8; width * height * 4];

    let bottom_up = bmp_h_total > 0;

    match bpp {
        32 => decode_bgra32(xor, and_mask, width, height, xor_stride, and_stride, bottom_up, &mut out)?,
        24 => decode_bgr24_and_mask(xor, and_mask, width, height, xor_stride, and_stride, bottom_up, &mut out)?,
        _ => return Err(IcoError::UnsupportedBitDepth),
    }

    Ok(DecodedIcon {
        width,
        height,
        pixels_rgba: out,
    })
}

fn decode_bgra32(
    xor: &[u8],
    and_mask: &[u8],
    width: usize,
    height: usize,
    xor_stride: usize,
    and_stride: usize,
    bottom_up: bool,
    out: &mut [u8],
) -> Result<(), IcoError> {
    for y in 0..height {
        let src_y = if bottom_up { height - 1 - y } else { y };
        let src_row = src_y.checked_mul(xor_stride).ok_or(IcoError::OutOfBounds)?;
        let and_row = src_y.checked_mul(and_stride).ok_or(IcoError::OutOfBounds)?;

        for x in 0..width {
            let src = src_row + x * 4;
            if src + 4 > xor.len() {
                return Err(IcoError::OutOfBounds);
            }

            let b = xor[src];
            let g = xor[src + 1];
            let r = xor[src + 2];
            let mut a = xor[src + 3];

            if !and_mask.is_empty() {
                let transparent = and_mask_bit(and_mask, and_row, x)?;
                if transparent && a == 0 {
                    a = 0;
                }
            }

            let dst = (y * width + x) * 4;
            out[dst] = r;
            out[dst + 1] = g;
            out[dst + 2] = b;
            out[dst + 3] = a;
        }
    }

    Ok(())
}

fn decode_bgr24_and_mask(
    xor: &[u8],
    and_mask: &[u8],
    width: usize,
    height: usize,
    xor_stride: usize,
    and_stride: usize,
    bottom_up: bool,
    out: &mut [u8],
) -> Result<(), IcoError> {
    for y in 0..height {
        let src_y = if bottom_up { height - 1 - y } else { y };
        let src_row = src_y.checked_mul(xor_stride).ok_or(IcoError::OutOfBounds)?;
        let and_row = src_y.checked_mul(and_stride).ok_or(IcoError::OutOfBounds)?;

        for x in 0..width {
            let src = src_row + x * 3;
            if src + 3 > xor.len() {
                return Err(IcoError::OutOfBounds);
            }

            let b = xor[src];
            let g = xor[src + 1];
            let r = xor[src + 2];
            let a = if !and_mask.is_empty() && and_mask_bit(and_mask, and_row, x)? {
                0
            } else {
                255
            };

            let dst = (y * width + x) * 4;
            out[dst] = r;
            out[dst + 1] = g;
            out[dst + 2] = b;
            out[dst + 3] = a;
        }
    }

    Ok(())
}

fn and_mask_bit(mask: &[u8], row_off: usize, x: usize) -> Result<bool, IcoError> {
    let byte_index = row_off + (x / 8);
    if byte_index >= mask.len() {
        return Err(IcoError::OutOfBounds);
    }
    let bit = 7 - (x % 8);
    Ok(((mask[byte_index] >> bit) & 1) != 0)
}

fn row_stride(width: usize, bpp: usize) -> usize {
    ((width * bpp + 31) / 32) * 4
}

fn slice_at(data: &[u8], off: usize, len: usize) -> Result<&[u8], IcoError> {
    let end = off.checked_add(len).ok_or(IcoError::OutOfBounds)?;
    if end > data.len() {
        return Err(IcoError::OutOfBounds);
    }
    Ok(&data[off..end])
}

fn le_u16(data: &[u8], off: usize) -> Result<u16, IcoError> {
    if off + 2 > data.len() {
        return Err(IcoError::TooShort);
    }
    Ok(u16::from_le_bytes([data[off], data[off + 1]]))
}

fn le_u32(data: &[u8], off: usize) -> Result<u32, IcoError> {
    if off + 4 > data.len() {
        return Err(IcoError::TooShort);
    }
    Ok(u32::from_le_bytes([
        data[off],
        data[off + 1],
        data[off + 2],
        data[off + 3],
    ]))
}

fn le_i32(data: &[u8], off: usize) -> Result<i32, IcoError> {
    if off + 4 > data.len() {
        return Err(IcoError::TooShort);
    }
    Ok(i32::from_le_bytes([
        data[off],
        data[off + 1],
        data[off + 2],
        data[off + 3],
    ]))
}
