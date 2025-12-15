use limine::{request::DateAtBootRequest, response::DateAtBootResponse};

pub static DATE_AT_BOOT_REQ: DateAtBootRequest = DateAtBootRequest::new();

#[inline(always)]
#[allow(unused)]
pub fn date_at_boot() -> &'static DateAtBootResponse {
    DATE_AT_BOOT_REQ
        .get_response()
        .expect("No Limine date_at_boot response")
}
