pub mod syscall {
    pub mod dispatch;
    pub mod nr;
    pub mod types;
    pub mod user_api;
    pub mod userbuf;
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
}

pub mod sched {
    pub mod proc;
    pub mod switch_context;
    pub mod task;
}

pub mod fs {
    pub mod fat16;
    pub mod vfs;
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
