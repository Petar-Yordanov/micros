pub mod x86_64 {
    pub mod cpu {
        pub mod cpuid;
    }

    pub mod descriptors {
        pub mod gdt;
        pub mod idt;
        pub mod tss;
    }

    pub mod interrupts {
        pub mod irq;
        pub mod isr;
        pub mod syscall_entry;
    }

    pub mod legacy {
        pub mod pic8259;
    }

    pub mod time {
        pub mod apic;
        pub mod rtc;
    }

    pub mod serial;
}
