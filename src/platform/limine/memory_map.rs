use limine::{request::MemoryMapRequest, response::MemoryMapResponse};

pub static MEMMAP_REQ: MemoryMapRequest = MemoryMapRequest::new();

#[inline(always)]
pub fn response() -> &'static MemoryMapResponse {
    MEMMAP_REQ.get_response().expect("No Limine memory map")
}
