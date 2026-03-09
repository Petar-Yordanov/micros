extern crate alloc;

use crate::frame;
use crate::kernel::mm::map::mapper as page;
use crate::kernel::mm::virt::vmarena;
use x86_64::{PhysAddr, VirtAddr};

pub struct QueueMem {
    pub va: VirtAddr,
    pub pa: PhysAddr,
    pub size: usize,
}

impl QueueMem {
    #[allow(unused)]
    pub fn alloc_pages(n_pages: usize) -> Self {
        let va_base = vmarena::alloc_n(n_pages).expect("vq vmarena");
        let first_pa = page::translate(va_base).expect("vq map");
        Self {
            va: va_base,
            pa: first_pa,
            size: n_pages * 4096,
        }
    }

    pub fn alloc_pages_contig(n_pages: usize) -> Self {
        assert!(n_pages >= 1);

        let va_base = vmarena::alloc_n(n_pages).expect("vq vmarena");

        for i in 0..n_pages {
            let va = va_base + (i as u64) * 4096;
            if let Ok(pf) = page::unmap(va) {
                frame::free(pf);
            }
        }

        'retry: loop {
            let mut frames = alloc::vec::Vec::with_capacity(n_pages);

            for _ in 0..n_pages {
                let pf = frame::alloc().expect("vq frame alloc");
                frames.push(pf);
            }

            let base = frames[0].start_address().as_u64();
            for i in 1..n_pages {
                let expect = base + (i as u64) * 4096;
                if frames[i].start_address().as_u64() != expect {
                    for pf in frames {
                        frame::free(pf);
                    }
                    continue 'retry;
                }
            }

            for (i, pf) in frames.iter().enumerate() {
                let va = va_base + (i as u64) * 4096;
                page::map_fixed(va, *pf, crate::kernel::mm::map::mapper::Prot::RW)
                    .expect("vq map_fixed");
            }

            return Self {
                va: va_base,
                pa: PhysAddr::new(base),
                size: n_pages * 4096,
            };
        }
    }
}
