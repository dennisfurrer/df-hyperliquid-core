//! The table against `L1Read.sol` as Hyperliquid's docs attach it (2026-10-08).

use super::*;

#[test]
fn the_table_is_l1read_every_address_from_0x800_once() {
    let addresses: Vec<u16> = ALL.iter().map(|one| one.at).collect();
    assert_eq!(addresses, (0x800..=0x814).collect::<Vec<_>>());
    let names: Vec<&str> = ALL.iter().map(|one| one.name).collect();
    assert_eq!(
        names,
        [
            "position",
            "spotBalance",
            "userVaultEquity",
            "withdrawable",
            "delegations",
            "delegatorSummary",
            "markPx",
            "oraclePx",
            "spotPx",
            "l1BlockNumber",
            "perpAssetInfo",
            "spotInfo",
            "tokenInfo",
            "tokenSupply",
            "bbo",
            "accountMarginSummary",
            "coreUserExists",
            "borrowLendUserState",
            "borrowLendReserveState",
            "position2",
            "outcomeStatus",
        ]
    );
    for one in &ALL {
        assert!(crate::client::evm::abi::params(one.args).is_ok(), "{}: args", one.name);
        assert!(crate::client::evm::abi::params(one.returns).is_ok(), "{}: returns", one.name);
        assert_eq!(spec(one.at), Some(one));
    }
    assert_eq!(address(0x80e).to_hex(), "0x000000000000000000000000000000000000080e");
    assert_eq!(spec(0x815), None);
}
