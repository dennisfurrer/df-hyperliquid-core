//! Keccak-256, the Ethereum hash: pad byte `0x01`, not SHA3's `0x06`.
//!
//! The sponge and padding are written here. The permutation is borrowed from
//! the `keccak` crate, which uses the ARMv8.2 SHA3 instructions where a native
//! build has them and its portable code everywhere else, wasm32 included.

/// The rate of Keccak-`f[1600]` at 256-bit capacity: 136 bytes per permute.
const RATE: usize = 136;

#[must_use]
pub fn keccak256(data: &[u8]) -> [u8; 32] {
    Keccak256::new().chain(data).finalize()
}

#[derive(Clone)]
pub struct Keccak256 {
    state: [u64; 25],
    block: [u8; RATE],
    filled: usize,
}

impl Keccak256 {
    #[must_use]
    pub const fn new() -> Self {
        Keccak256 { state: [0u64; 25], block: [0u8; RATE], filled: 0 }
    }

    pub fn update(&mut self, data: &[u8]) {
        let mut rest = data;
        while !rest.is_empty() {
            let take = (RATE - self.filled).min(rest.len());
            self.block[self.filled..self.filled + take].copy_from_slice(&rest[..take]);
            self.filled += take;
            rest = &rest[take..];
            if self.filled == RATE {
                self.absorb();
                self.filled = 0;
            }
        }
    }

    #[must_use]
    pub fn chain(mut self, data: &[u8]) -> Self {
        self.update(data);
        self
    }

    #[must_use]
    pub fn finalize(mut self) -> [u8; 32] {
        self.block[self.filled..].fill(0);
        self.block[self.filled] = 0x01;
        self.block[RATE - 1] |= 0x80;
        self.absorb();

        let mut digest = [0u8; 32];
        for (bytes, lane) in digest.chunks_exact_mut(8).zip(self.state.iter()) {
            bytes.copy_from_slice(&lane.to_le_bytes());
        }
        digest
    }

    fn absorb(&mut self) {
        for (lane, bytes) in self.state.iter_mut().zip(self.block.chunks_exact(8)) {
            let mut word = [0u8; 8];
            word.copy_from_slice(bytes);
            *lane ^= u64::from_le_bytes(word);
        }
        permute(&mut self.state);
    }
}

impl Default for Keccak256 {
    fn default() -> Self {
        Keccak256::new()
    }
}

impl std::fmt::Debug for Keccak256 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Keccak256")
    }
}

fn permute(state: &mut [u64; 25]) {
    keccak::Keccak::new().with_f1600(|f| f(state));
}

#[cfg(test)]
#[path = "tests/keccak.rs"]
mod tests;
