use crate::kernel::syscall::{chan, exec, fb, input, log, proc, shm, time, vfs};
use micros_abi::sysnr as nr;
use crate::kernel::syscall::power;

pub fn dispatch(nr_no: u64, a0: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64) -> i64 {
    match nr_no {
        nr::SYS_FB_INFO => fb::sys_fb_info(a0),
        nr::SYS_FB_MAP => fb::sys_fb_map(),
        nr::SYS_LOG => log::sys_log(a0, a1),
        nr::SYS_INPUT_NEXT_EVENT => input::sys_input_next_event(a0),

        nr::SYS_EXIT => proc::sys_exit(a0),
        nr::SYS_YIELD => proc::sys_yield(),
        nr::SYS_GETPID => proc::sys_getpid(),

        nr::SYS_PROC_SPAWN => proc::sys_proc_spawn(a0),
        nr::SYS_PROC_WAIT => proc::sys_proc_wait(a0, a1),
        nr::SYS_PROC_KILL => proc::sys_proc_kill(a0, a1),
        nr::SYS_PROC_LIST => proc::sys_proc_list(a0, a1, a2),
        nr::SYS_SLEEP_MS => proc::sys_sleep_ms(a0),
        nr::SYS_PROC_INFO => proc::sys_proc_info(a0, a1),

        nr::SYS_TIME_WALL => time::sys_time_wall(a0),
        nr::SYS_TIME_UPTIME => time::sys_time_uptime(a0),

        nr::SYS_VFS_READ => vfs::sys_vfs_read(a0),
        nr::SYS_VFS_WRITE => vfs::sys_vfs_write(a0),
        nr::SYS_VFS_LIST => vfs::sys_vfs_list(a0),
        nr::SYS_VFS_MKDIR => vfs::sys_vfs_mkdir(a0, a1),
        nr::SYS_VFS_MOUNT => vfs::sys_vfs_mount(a0),

        nr::SYS_EXEC => exec::sys_exec(a0, a1),

        nr::SYS_CHAN_CREATE => chan::sys_chan_create(a0),
        nr::SYS_CHAN_SEND => chan::sys_chan_send(a0),
        nr::SYS_CHAN_RECV => chan::sys_chan_recv(a0),

        nr::SYS_SHM_CREATE => shm::sys_shm_create(a0),
        nr::SYS_SHM_MAP => shm::sys_shm_map(a0),

        nr::SYS_POWER => power::sys_power(a0),

        _ => {
            crate::ksprintln!(
                "[syscall] unknown nr={} (a0={:#x} a1={:#x} a2={:#x} a3={:#x} a4={:#x} a5={:#x})",
                nr_no,
                a0,
                a1,
                a2,
                a3,
                a4,
                a5
            );
            -38 // -ENOSYS (TODO: Substitute with micros_abi errno values)
        }
    }
}
