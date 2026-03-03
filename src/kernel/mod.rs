pub mod syscall {
    pub mod dispatch;
    pub mod exec;
    pub mod fb;
    pub mod input;
    pub mod log;
    pub mod proc;
    pub mod util;
    pub mod vfs;

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
        pub mod pci;
        pub mod virtqueue;
    }
}

pub mod input {
    pub mod events;
    pub mod parser;
}

pub mod exec {
    pub mod init;
    pub mod elf;
    pub mod path;
    pub mod exec_impl;

    pub use exec_impl::*;
}
