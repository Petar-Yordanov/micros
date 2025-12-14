pub struct Bitset<const N: usize, const WORDS: usize> {
    storage: [u64; WORDS],
}

impl<const N: usize, const WORDS: usize> Bitset<N, WORDS> {
    pub const fn new() -> Self {
        Self {
            storage: [0; WORDS],
        }
    }

    #[allow(dead_code)]
    #[inline]
    pub const fn capacity(&self) -> usize {
        N
    }

    #[inline]
    pub unsafe fn get_unchecked(&self, i: usize) -> bool {
        let w = i / 64;
        let b = i % 64;
        (self.storage[w] >> b) & 1 == 1
    }

    #[inline]
    pub unsafe fn set_unchecked(&mut self, i: usize, v: bool) {
        let w = i / 64;
        let b = i % 64;
        if v {
            self.storage[w] |= 1u64 << b;
        } else {
            self.storage[w] &= !(1u64 << b);
        }
    }

    pub fn fill_tracked(&mut self, tracked: usize, v: bool) {
        let full_words = tracked / 64;
        let rem_bits = tracked % 64;
        if v {
            for w in 0..full_words {
                self.storage[w] = u64::MAX;
            }
            if rem_bits > 0 {
                self.storage[full_words] |= (1u64 << rem_bits) - 1;
            }
        } else {
            for w in 0..full_words {
                self.storage[w] = 0;
            }
            if rem_bits > 0 {
                let mask = (1u64 << rem_bits) - 1;
                self.storage[full_words] &= !mask;
            }
        }
    }

    pub fn set_range(&mut self, start: usize, end: usize, tracked: usize, v: bool) {
        let s = start.min(tracked);
        let e = end.min(tracked);
        if s >= e {
            return;
        }
        let mut i = s;
        while i < e {
            unsafe {
                self.set_unchecked(i, v);
            }
            i += 1;
        }
    }

    pub fn first_free(&self, tracked: usize) -> Option<usize> {
        if tracked == 0 {
            return None;
        }
        let full_words = tracked / 64;
        let rem_bits = tracked % 64;

        for w in 0..full_words {
            let word = self.storage[w];
            if word != u64::MAX {
                let inv = !word;
                let bit = inv.trailing_zeros() as usize;
                return Some(w * 64 + bit);
            }
        }

        if rem_bits > 0 {
            let w = full_words;
            let valid_mask = (1u64 << rem_bits) - 1;
            let masked = self.storage[w] | !valid_mask;
            if masked != u64::MAX {
                let inv = !masked;
                let bit = inv.trailing_zeros() as usize;
                debug_assert!(bit < rem_bits);
                return Some(w * 64 + bit);
            }
        }
        None
    }

    pub fn find_run(&self, tracked: usize, n: usize) -> Option<usize> {
        if n == 0 || n > tracked {
            return None;
        }
        let mut run = 0usize;
        let mut start = 0usize;
        for i in 0..tracked {
            if unsafe { !self.get_unchecked(i) } {
                if run == 0 {
                    start = i;
                }
                run += 1;
                if run == n {
                    return Some(start);
                }
            } else {
                run = 0;
            }
        }
        None
    }
}
