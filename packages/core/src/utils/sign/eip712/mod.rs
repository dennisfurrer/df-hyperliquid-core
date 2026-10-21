//! EIP-712 for the flat structs Hyperliquid signs, and HyperEVM contracts that
//! take the same kind (one level, `uint256[]` the only array): hashing, and the
//! typed data a browser wallet's `eth_signTypedData_v4` is handed. Typed data
//! another program hands over, any shape, reads with [`typed`].

pub mod domain;
pub mod field;
pub mod typed;
pub mod typed_struct;
pub mod wallet;
pub mod word;

pub use domain::{DOMAIN_TYPE, Domain};
pub use field::Field;
pub use typed::{BadTypedData, TypedData};
pub use typed_struct::{TypedStruct, digest};
pub use wallet::digest_of_wallet_json;
pub use word::{decimal_word, uint256, word_decimal};
