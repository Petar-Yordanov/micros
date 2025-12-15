#[repr(C)]
pub struct FbInfo {
    pub addr: u64,
    pub width: u32,
    pub height: u32,
    pub pitch: u32,
    pub bpp: u32,
}

//pub type VfsResult<T> = Result<T, vfs::VfsError>;

//#[derive(Debug, Clone, Copy)]
//pub struct HeapStats {
//    pub total_pages: usize,
//    pub used_pages: usize,
//}
