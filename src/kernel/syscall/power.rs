use micros_abi::errno;

use x86_64::instructions::{
    hlt, interrupts,
    port::{Port, PortWriteOnly},
};

pub const POWER_ACTION_OFF: u64 = 0;
pub const POWER_ACTION_REBOOT: u64 = 1;

fn qemu_poweroff() {
    unsafe {
        let mut pm1a_cnt: Port<u16> = Port::new(0x604);
        pm1a_cnt.write(0x2000);
    }
}

fn qemu_debug_exit(code: u8) {
    unsafe {
        let mut port: PortWriteOnly<u8> = PortWriteOnly::new(0x501);
        port.write(code);
    }
}

fn halt_forever() -> ! {
    interrupts::disable();
    loop {
        hlt();
    }
}

pub(super) fn sys_power(action: u64) -> i64 {
    match action {
        POWER_ACTION_OFF => {
            crate::ksprintln!("[power] poweroff requested");

            qemu_poweroff();

            crate::ksprintln!("[power] qemu poweroff did not terminate VM; trying isa-debug-exit");
            qemu_debug_exit(0);

            crate::ksprintln!("[power] shutdown path returned; halting CPU");
            halt_forever();
        }

        POWER_ACTION_REBOOT => {
            crate::ksprintln!("[power] reboot requested but not implemented");
            -errno::ENOSYS
        }

        _ => -errno::EINVAL,
    }
}
