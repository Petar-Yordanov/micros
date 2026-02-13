#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Errno(pub i32);

pub fn cvt(ret: i64) -> Result<i64, Errno> {
    if ret < 0 {
        Err(Errno((-ret) as i32))
    } else {
        Ok(ret)
    }
}
