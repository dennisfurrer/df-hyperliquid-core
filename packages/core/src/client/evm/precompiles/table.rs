//! The read precompiles as `L1Read.sol` declares them, in address order, and
//! the address each sits at.

use crate::utils::Address;

/// One precompile as `L1Read.sol` declares it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct Spec {
    /// The low two bytes of its address.
    pub at: u16,
    /// The `L1Read.sol` function that wraps it.
    pub name: &'static str,
    /// The arguments as `L1Read.sol` declares them, such as `address user, uint16 perp`.
    pub args: &'static str,
    /// The return type as `L1Read.sol` declares it, such as `(uint64 withdrawable)`.
    pub returns: &'static str,
}

const POSITION: &str = "(int64 szi, uint64 entryNtl, int64 isolatedRawUsd, uint32 leverage, bool isIsolated)";

/// Every read precompile `L1Read.sol` names, in address order.
pub const ALL: [Spec; 21] = [
    Spec { at: 0x800, name: "position", args: "address user, uint16 perp", returns: POSITION },
    Spec {
        at: 0x801,
        name: "spotBalance",
        args: "address user, uint64 token",
        returns: "(uint64 total, uint64 hold, uint64 entryNtl)",
    },
    Spec {
        at: 0x802,
        name: "userVaultEquity",
        args: "address user, address vault",
        returns: "(uint64 equity, uint64 lockedUntilTimestamp)",
    },
    Spec { at: 0x803, name: "withdrawable", args: "address user", returns: "(uint64 withdrawable)" },
    Spec {
        at: 0x804,
        name: "delegations",
        args: "address user",
        returns: "(address validator, uint64 amount, uint64 lockedUntilTimestamp)[]",
    },
    Spec {
        at: 0x805,
        name: "delegatorSummary",
        args: "address user",
        returns: "(uint64 delegated, uint64 undelegated, uint64 totalPendingWithdrawal, uint64 nPendingWithdrawals)",
    },
    Spec { at: 0x806, name: "markPx", args: "uint32 index", returns: "uint64" },
    Spec { at: 0x807, name: "oraclePx", args: "uint32 index", returns: "uint64" },
    Spec { at: 0x808, name: "spotPx", args: "uint32 index", returns: "uint64" },
    Spec { at: 0x809, name: "l1BlockNumber", args: "", returns: "uint64" },
    Spec {
        at: 0x80a,
        name: "perpAssetInfo",
        args: "uint32 perp",
        returns: "(string coin, uint32 marginTableId, uint8 szDecimals, uint8 maxLeverage, bool onlyIsolated)",
    },
    Spec { at: 0x80b, name: "spotInfo", args: "uint32 spot", returns: "(string name, uint64[2] tokens)" },
    Spec {
        at: 0x80c,
        name: "tokenInfo",
        args: "uint32 token",
        returns: "(string name, uint64[] spots, uint64 deployerTradingFeeShare, address deployer, address evmContract, uint8 szDecimals, uint8 weiDecimals, int8 evmExtraWeiDecimals)",
    },
    Spec {
        at: 0x80d,
        name: "tokenSupply",
        args: "uint32 token",
        returns: "(uint64 maxSupply, uint64 totalSupply, uint64 circulatingSupply, uint64 futureEmissions, (address user, uint64 balance)[] nonCirculatingUserBalances)",
    },
    Spec { at: 0x80e, name: "bbo", args: "uint32 asset", returns: "(uint64 bid, uint64 ask)" },
    Spec {
        at: 0x80f,
        name: "accountMarginSummary",
        args: "uint32 perp_dex_index, address user",
        returns: "(int64 accountValue, uint64 marginUsed, uint64 ntlPos, int64 rawUsd)",
    },
    Spec { at: 0x810, name: "coreUserExists", args: "address user", returns: "(bool exists)" },
    Spec {
        at: 0x811,
        name: "borrowLendUserState",
        args: "address user, uint64 token",
        returns: "((uint64 basis, uint64 value) borrow, (uint64 basis, uint64 value) supply)",
    },
    Spec {
        at: 0x812,
        name: "borrowLendReserveState",
        args: "uint64 token",
        returns: "(uint64 borrowYearlyRateBps, uint64 supplyYearlyRateBps, uint64 balance, uint64 utilizationBps, uint64 oraclePx, uint64 ltvBps, uint64 totalSupplied, uint64 totalBorrowed)",
    },
    Spec { at: 0x813, name: "position2", args: "address user, uint32 perp", returns: POSITION },
    Spec {
        at: 0x814,
        name: "outcomeStatus",
        args: "uint32 outcome",
        returns: "(uint8 status, uint64 settledValue, uint32 question)",
    },
];

/// The precompile at `0x...<at>`, if `L1Read.sol` names one there.
#[must_use]
pub fn spec(at: u16) -> Option<&'static Spec> {
    ALL.iter().find(|one| one.at == at)
}

/// `0x` and nineteen zero bytes, then the two of `at`.
#[must_use]
pub fn address(at: u16) -> Address {
    let mut bytes = [0u8; 20];
    bytes[18..].copy_from_slice(&at.to_be_bytes());
    Address::from_bytes(bytes)
}

#[cfg(test)]
#[path = "tests/table.rs"]
mod tests;
