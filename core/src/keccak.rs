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
const ROUND_CONSTANTS: [u64; 24] = [
    0x0000_0000_0000_0001,
    0x0000_0000_0000_8082,
    0x8000_0000_0000_808a,
    0x8000_0000_8000_8000,
    0x0000_0000_0000_808b,
    0x0000_0000_8000_0001,
    0x8000_0000_8000_8081,
    0x8000_0000_0000_8009,
    0x0000_0000_0000_008a,
    0x0000_0000_0000_0088,
    0x0000_0000_8000_8009,
    0x0000_0000_8000_000a,
    0x0000_0000_8000_808b,
    0x8000_0000_0000_008b,
    0x8000_0000_0000_8089,
    0x8000_0000_0000_8003,
    0x8000_0000_0000_8002,
    0x8000_0000_0000_0080,
    0x0000_0000_0000_800a,
    0x8000_0000_8000_000a,
    0x8000_0000_8000_8081,
    0x8000_0000_0000_8080,
    0x0000_0000_8000_0001,
    0x8000_0000_8000_8008,
];

#[cfg(test)]
const RHO: [u32; 25] = [
    0, 1, 62, 28, 27, //
    36, 44, 6, 55, 20, //
    3, 10, 43, 25, 39, //
    41, 45, 15, 21, 8, //
    18, 2, 61, 56, 14,
];

/// Pi permutation, computed rather than transcribed: a typed table is wrong.
#[cfg(test)]
const PI: [usize; 25] = {
    let mut moves = [0usize; 25];
    let mut x = 0;
    while x < 5 {
        let mut y = 0;
        while y < 5 {
            moves[x + 5 * y] = y + 5 * ((2 * x + 3 * y) % 5);
            y += 1;
        }
        x += 1;
    }
    moves
};

/// The permutation written out, to check the borrowed one against.
#[cfg(test)]
fn permute_portable(state: &mut [u64; 25]) {
    for &round_constant in &ROUND_CONSTANTS {
        let mut column = [0u64; 5];
        for (x, parity) in column.iter_mut().enumerate() {
            *parity = state[x] ^ state[x + 5] ^ state[x + 10] ^ state[x + 15] ^ state[x + 20];
        }
        let mut spread = [0u64; 5];
        for (x, value) in spread.iter_mut().enumerate() {
            *value = column[(x + 4) % 5] ^ column[(x + 1) % 5].rotate_left(1);
        }
        for (index, lane) in state.iter_mut().enumerate() {
            *lane ^= spread[index % 5];
        }

        let mut moved = [0u64; 25];
        for (index, &lane) in state.iter().enumerate() {
            moved[PI[index]] = lane.rotate_left(RHO[index]);
        }

        for (y, row) in moved.chunks_exact(5).enumerate() {
            let out = &mut state[y * 5..y * 5 + 5];
            out[0] = row[0] ^ (!row[1] & row[2]);
            out[1] = row[1] ^ (!row[2] & row[3]);
            out[2] = row[2] ^ (!row[3] & row[4]);
            out[3] = row[3] ^ (!row[4] & row[0]);
            out[4] = row[4] ^ (!row[0] & row[1]);
        }

        state[0] ^= round_constant;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hex;

    #[test]
    fn the_published_vectors_hold() {
        assert_eq!(hex::encode(&keccak256(b"")), "c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470");
        assert_eq!(hex::encode(&keccak256(b"abc")), "4e03657aea45a94fc7d47ba826c8d667c0d1e6e33a64a036ec44f58fa12d6c45");
        assert_eq!(
            hex::encode(&keccak256(b"The quick brown fox jumps over the lazy dog")),
            "4d741b6f1eb29cb2a9b9911c82f56fa8d73b04959d3d9d222895df6c0b28aa15"
        );
    }

    #[test]
    fn input_split_into_pieces_hashes_the_same() {
        let whole = b"the quick brown fox jumps over the lazy dog, repeatedly, \
                      for long enough to cross a 136 byte block boundary and \
                      then some more so the tail is short";
        assert!(whole.len() > RATE, "this test needs to span two blocks");
        for split in [0, 1, 7, 8, RATE - 1, RATE, RATE + 1, whole.len()] {
            let pieced = Keccak256::new().chain(&whole[..split]).chain(&whole[split..]).finalize();
            assert_eq!(pieced, keccak256(whole), "split at {split}");
        }
    }

    #[test]
    fn the_borrowed_permutation_is_the_one_this_build_would_have_written() {
        let mut mine = [0u64; 25];
        let mut theirs = [0u64; 25];
        for (at, (a, b)) in mine.iter_mut().zip(theirs.iter_mut()).enumerate() {
            let lane = 0x0123_4567_89ab_cdefu64.wrapping_mul(at as u64 + 1).rotate_left(at as u32);
            *a = lane;
            *b = lane;
        }
        for round in 0..64 {
            permute_portable(&mut mine);
            permute(&mut theirs);
            assert_eq!(mine, theirs, "they parted at round {round}");
        }
        assert_ne!(mine, [0u64; 25], "and the state actually moved");
    }
}
