use micros_abi::sysnr;
use micros_abi::types::AbiInputEvent;
use crate::syscall::syscall1;

#[inline(always)]
pub fn next_event(out: &mut AbiInputEvent) -> i64 {
    syscall1(sysnr::SYS_INPUT_NEXT_EVENT, out as *mut _ as u64)
}
