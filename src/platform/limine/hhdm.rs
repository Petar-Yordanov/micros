use limine::request::HhdmRequest;
use limine::response::HhdmResponse;
use x86_64::VirtAddr;

pub static HHDM_REQ: HhdmRequest = HhdmRequest::new();

#[inline(always)]
#[allow(unused)]
pub fn hhdm_base() -> VirtAddr {
    let resp: &HhdmResponse = HHDM_REQ.get_response().expect("no HHDM response");
    VirtAddr::new(resp.offset())
}

#[inline(always)]
#[allow(unused)]
pub fn phys_to_hhdm(pa: u64) -> VirtAddr {
    hhdm_base() + pa
}

#[inline(always)]
#[allow(unused)]
pub fn hhdm_to_phys(va: VirtAddr) -> u64 {
    va.as_u64() - hhdm_base().as_u64()
}
