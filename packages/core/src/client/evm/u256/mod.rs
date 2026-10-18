//! Exact 256-bit integers, the EVM's word: balances and fees in wei, gas
//! prices, ABI integers, and the native coin's 18 decimals. JSON-RPC writes
//! them as quantities, `0x` and hex digits without leading zeros. Nothing here
//! passes through a float.

pub mod amount;
pub mod b256;
pub mod text;
pub mod uint;

pub use amount::{BadAmount, NATIVE_DECIMALS};
pub use b256::B256;
pub use text::BadQuantity;
pub use uint::U256;
