pub mod syscall {
    pub mod dispatch;
    pub mod exec;
    pub mod fb;
    pub mod input;
    pub mod log;
    pub mod proc;
    pub mod util;
    pub mod vfs;
    pub mod time;
    pub mod chan;
    pub mod shm;
    pub mod power;
    pub use dispatch::dispatch;
}

pub mod utils {
    pub mod align;
    pub mod bitset;
}

pub mod mm {
    pub mod phys {
        pub mod frame;
    }
    pub mod virt {
        pub mod vmarena;
    }
    pub mod map {
        pub mod mapper;
        pub mod mmio;
    }
    pub mod heap {
        pub mod freelist;
        pub mod global_alloc;
    }
    pub mod aspace {
        pub mod address_space;
        pub mod user_copy;
    }
    pub mod user {
        pub mod mapfb;
    }
}

pub mod sched {
    pub mod proc;
    pub mod switch_context;
    pub mod task;
    pub mod kstack;
}

pub mod fs {
    pub mod fat32;
    pub mod ext2;
    pub mod vfs {
        pub mod error;
        pub mod mount;
        pub mod ops;
        pub mod path;
        pub mod selftest;
        pub use crate::kernel::fs::vfs::selftest::vfs_selftest;
    }
}

pub mod drivers {
    pub mod pci {
        pub mod cfg_io;
    }

    pub mod virtio {
        pub mod blk;
        pub mod input;

        pub mod pci {
            pub mod caps;
            pub mod init;
            pub mod transport;

            pub use caps::VirtioPciCommonCfg;
            pub(crate) use caps::VirtioPciRegs;
            pub use init::init;
            pub use transport::{
                devcfg_read_le32, devcfg_read_le64, negotiate_features, setup_queue, STATUS_DRIVER_OK,
            };
        }

        pub mod virtqueue {
            pub mod defs;
            pub mod mem;
            pub mod queue;

            pub use defs::{VIRTQ_DESC_F_NEXT, VIRTQ_DESC_F_WRITE};
            pub use queue::VirtQueue;
        }
    }
}

pub mod exec {
    pub mod init;
    pub mod elf;
    pub mod exec_impl;

    pub use exec_impl::*;
}

pub mod bootlog {
    mod state;
    mod console;
    mod render;
    mod tags;

    pub use state::{
        boot_progress_step,
        bootlog_fb_disable,
        bootlog_fb_enable,
        bootlog_push_line,
        bootlog_set_progress_total,
        try_init,
    };
}
pub mod boot;
pub mod selftest;
