//! CPython's `random.Random(seed)`: MT19937 plus the draw the sampling
//! script makes, `sample`.
//!
//! `precision_sample.py` pins a seed and the sample it draws feeds judging
//! sheets, so the Rust tool must draw the same rows as the Python one.
//! Porting the generator keeps that equality without a Python process in
//! the loop; the tests below pin it against `random.Random`.

const N: usize = 624;
const M: usize = 397;
const MATRIX_A: u32 = 0x9908_b0df;
const UPPER: u32 = 0x8000_0000;
const LOWER: u32 = 0x7fff_ffff;

pub struct Random {
    mt: [u32; N],
    mti: usize,
}

impl Random {
    /// `random.Random(seed)` for a non-negative int seed under 2^32:
    /// CPython splits the absolute value into 32-bit words and seeds
    /// `init_by_array` with them.
    pub fn new(seed: u32) -> Self {
        let mut r = Self { mt: [0; N], mti: N };
        r.init_genrand(19_650_218);
        let key = [seed];
        let (mut i, mut j) = (1usize, 0usize);
        for _ in 0..N.max(key.len()) {
            let prev = r.mt[i - 1];
            r.mt[i] = (r.mt[i] ^ ((prev ^ (prev >> 30)).wrapping_mul(1_664_525)))
                .wrapping_add(key[j])
                .wrapping_add(u32::try_from(j).unwrap_or(0));
            i += 1;
            j += 1;
            if i >= N {
                r.mt[0] = r.mt[N - 1];
                i = 1;
            }
            if j >= key.len() {
                j = 0;
            }
        }
        for _ in 0..N - 1 {
            let prev = r.mt[i - 1];
            r.mt[i] = (r.mt[i] ^ ((prev ^ (prev >> 30)).wrapping_mul(1_566_083_941)))
                .wrapping_sub(u32::try_from(i).unwrap_or(0));
            i += 1;
            if i >= N {
                r.mt[0] = r.mt[N - 1];
                i = 1;
            }
        }
        r.mt[0] = UPPER;
        r
    }

    fn init_genrand(&mut self, seed: u32) {
        self.mt[0] = seed;
        for i in 1..N {
            let prev = self.mt[i - 1];
            self.mt[i] = 1_812_433_253u32
                .wrapping_mul(prev ^ (prev >> 30))
                .wrapping_add(u32::try_from(i).unwrap_or(0));
        }
        self.mti = N;
    }

    fn next_u32(&mut self) -> u32 {
        if self.mti >= N {
            for k in 0..N {
                let y = (self.mt[k] & UPPER) | (self.mt[(k + 1) % N] & LOWER);
                self.mt[k] =
                    self.mt[(k + M) % N] ^ (y >> 1) ^ if y & 1 == 0 { 0 } else { MATRIX_A };
            }
            self.mti = 0;
        }
        let mut y = self.mt[self.mti];
        self.mti += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c_5680;
        y ^= (y << 15) & 0xefc6_0000;
        y ^ (y >> 18)
    }

    /// `getrandbits(k)` for `k` at most 32, the only width `_randbelow` asks
    /// for on a population this tool can hold.
    fn getrandbits(&mut self, k: u32) -> u32 {
        if k == 0 {
            return 0;
        }
        self.next_u32() >> (32 - k)
    }

    /// `_randbelow_with_getrandbits`: rejection sampling on the bit width.
    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            return 0;
        }
        let k = usize::BITS - n.leading_zeros();
        loop {
            let r = self.getrandbits(k) as usize;
            if r < n {
                return r;
            }
        }
    }

    /// `random.sample(population, k)`: the indices it selects, in selection
    /// order. Both branches of CPython's set/pool split are kept, since the
    /// branch decides how many draws are made.
    pub fn sample(&mut self, n: usize, k: usize) -> Vec<usize> {
        let mut setsize = 21f64;
        if k > 5 {
            let k3 = (k * 3) as f64;
            setsize += 4f64.powf((k3.ln() / 4f64.ln()).ceil());
        }
        let mut result = Vec::with_capacity(k);
        if (n as f64) <= setsize {
            let mut pool: Vec<usize> = (0..n).collect();
            for i in 0..k {
                let j = self.below(n - i);
                result.push(pool[j]);
                pool[j] = pool[n - i - 1];
            }
        } else {
            let mut selected = std::collections::HashSet::new();
            for _ in 0..k {
                let mut j = self.below(n);
                while !selected.insert(j) {
                    j = self.below(n);
                }
                result.push(j);
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // every expected value is `random.Random(seed)` under CPython 3.14
    const SEED: u32 = 20_260_851;

    #[test]
    fn the_stream_matches_getrandbits_32() {
        let mut r = Random::new(SEED);
        let got: Vec<u32> = (0..5).map(|_| r.getrandbits(32)).collect();
        assert_eq!(
            got,
            [2301442336, 3280205894, 3701039754, 2439668077, 3783878093]
        );
    }

    #[test]
    fn sample_matches_the_pool_branch() {
        assert_eq!(
            Random::new(SEED).sample(60, 20),
            [
                34, 48, 55, 36, 5, 11, 58, 46, 3, 59, 51, 14, 6, 49, 7, 28, 8, 39, 29, 35
            ]
        );
        // n = 85 is the last population the pool branch takes at k = 20
        assert_eq!(
            Random::new(SEED).sample(85, 20),
            [
                68, 72, 10, 23, 7, 84, 6, 28, 12, 78, 15, 57, 17, 58, 11, 41, 16, 60, 54, 80
            ]
        );
    }

    #[test]
    fn sample_matches_the_selected_set_branch() {
        assert_eq!(
            Random::new(SEED).sample(86, 20),
            [
                68, 72, 10, 23, 7, 6, 28, 12, 15, 57, 17, 79, 58, 71, 11, 41, 16, 60, 54, 78
            ]
        );
        assert_eq!(
            Random::new(SEED).sample(500, 20),
            [
                274, 391, 441, 290, 451, 43, 477, 93, 387, 374, 29, 273, 485, 27, 112, 480, 50,
                468, 449, 26
            ]
        );
    }
}
