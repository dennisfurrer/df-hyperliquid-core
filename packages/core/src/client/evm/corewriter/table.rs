//! The docs' action table, and the contract's constants: its address, the
//! topic every action is logged under, the selector, and the encoding version.

use crate::client::evm::u256::B256;
use crate::utils::Address;

/// The CoreWriter system contract.
pub const CORE_WRITER: Address = Address::from_bytes([0x33; 20]);

/// `keccak256("RawAction(address,bytes)")`, the topic every action is logged under.
pub const RAW_ACTION_TOPIC: B256 = B256([
    0x8c, 0x7f, 0x58, 0x5f, 0xb2, 0x95, 0xf7, 0xeb, 0x1e, 0x6a, 0xeb, 0x8f, 0xba, 0x61, 0xb2, 0x3a, 0x4f, 0xe6, 0x0b,
    0xed, 0xa4, 0x05, 0xf0, 0x04, 0x50, 0x73, 0xb1, 0x85, 0xc7, 0x44, 0x12, 0xe3,
]);

/// `sendRawAction(bytes)`.
pub const SEND_RAW_ACTION: [u8; 4] = [0x17, 0x93, 0x8e, 0x13];

/// The only encoding version the docs give.
pub const ENCODING_VERSION: u8 = 1;

/// The one id the docs' table skips.
pub const GAP: u32 = 14;

/// `limitPx` and `sz`: 1e8 times what a person reads, whatever the market.
pub const ORDER_DECIMALS: u32 = 8;
/// The dex id `sendAsset` gives spot.
pub const SPOT_DEX: u32 = u32::MAX;

/// One row of the docs' action table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spec {
    /// The action id, 1 to 17.
    pub id: u32,
    /// The action's name as the docs' table writes it, such as `Limit order`.
    pub name: &'static str,
    /// The field names, in order, as the docs give them.
    pub fields: &'static [&'static str],
    /// The Solidity types, as the docs give them.
    pub types: &'static str,
}

/// The docs' table, every id it has.
pub const SPECS: [Spec; 16] = [
    Spec {
        id: 1,
        name: "Limit order",
        fields: &["asset", "isBuy", "limitPx", "sz", "reduceOnly", "encodedTif", "cloid"],
        types: "uint32, bool, uint64, uint64, bool, uint8, uint128",
    },
    Spec { id: 2, name: "Vault transfer", fields: &["vault", "isDeposit", "usd"], types: "address, bool, uint64" },
    Spec {
        id: 3,
        name: "Token delegate",
        fields: &["validator", "wei", "isUndelegate"],
        types: "address, uint64, bool",
    },
    Spec { id: 4, name: "Staking deposit", fields: &["wei"], types: "uint64" },
    Spec { id: 5, name: "Staking withdraw", fields: &["wei"], types: "uint64" },
    Spec { id: 6, name: "Spot send", fields: &["destination", "token", "wei"], types: "address, uint64, uint64" },
    Spec { id: 7, name: "USD class transfer", fields: &["ntl", "toPerp"], types: "uint64, bool" },
    Spec {
        id: 8,
        name: "Finalize EVM contract",
        fields: &["token", "encodedFinalizeEvmContractVariant", "createNonce"],
        types: "uint64, uint8, uint64",
    },
    Spec { id: 9, name: "Add API wallet", fields: &["wallet", "name"], types: "address, string" },
    Spec { id: 10, name: "Cancel order by oid", fields: &["asset", "oid"], types: "uint32, uint64" },
    Spec { id: 11, name: "Cancel order by cloid", fields: &["asset", "cloid"], types: "uint32, uint128" },
    Spec { id: 12, name: "Approve builder fee", fields: &["maxFeeRate", "builder"], types: "uint64, address" },
    Spec {
        id: 13,
        name: "Send asset",
        fields: &["destination", "subAccount", "source_dex", "destination_dex", "token", "wei"],
        types: "address, address, uint32, uint32, uint64, uint64",
    },
    Spec {
        id: 15,
        name: "Borrow lend operation",
        fields: &["encodedOperation", "token", "wei"],
        types: "uint8, uint64, uint64",
    },
    Spec { id: 16, name: "Set abstraction", fields: &["user", "abstraction"], types: "address, uint8" },
    Spec {
        id: 17,
        name: "Outcome operation",
        fields: &["encodedOperation", "question", "outcome", "wei"],
        types: "uint8, uint32, uint32, uint64",
    },
];

/// The table's row for `id`.
#[must_use]
pub fn spec(id: u32) -> Option<&'static Spec> {
    SPECS.iter().find(|spec| spec.id == id)
}

#[cfg(test)]
#[path = "tests/table.rs"]
mod tests;
