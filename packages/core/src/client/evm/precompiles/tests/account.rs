//! Account reads against what viem encoded and HyperEVM mainnet answered, each answer
//! against viem's `decodeAbiParameters` of the same live bytes, and one against the info API.

use crate::client::evm::precompiles::support::{FOLLOWER, FUND, HLP, SHORT, a, s, sent, viem};
use crate::utils::Dec;

use super::*;

#[test]
fn positions_balances_and_equity_read_as_the_chain_wrote_them() {
    let position = sent(&position(a(SHORT), 0), "position");
    let expected = viem("position");
    assert_eq!(position.szi.to_string(), s(&expected["szi"]));
    assert_eq!(position.entry_ntl.to_string(), s(&expected["entryNtl"]));
    assert_eq!(position.isolated_raw_usd.to_string(), s(&expected["isolatedRawUsd"]));
    assert_eq!(position.leverage.to_string(), s(&expected["leverage"]));
    assert_eq!(position.is_isolated.to_string(), s(&expected["isIsolated"]));
    // The info API said -0.73625 BTC for this vault: szi is in BTC's five size decimals.
    assert_eq!(Dec::new(i128::from(position.szi), 5), Dec::parse("-0.73625").ok());
    assert_eq!(sent(&position2(a(SHORT), 0), "position2"), position, "the widened sibling reads the same");

    let balance = sent(&spot_balance(a(FUND), 150), "spotBalance");
    let expected = viem("spotBalance");
    assert_eq!(
        [balance.total, balance.hold, balance.entry_ntl].map(|one| one.to_string()),
        [s(&expected["total"]), s(&expected["hold"]), s(&expected["entryNtl"])]
    );

    let equity = sent(&user_vault_equity(a(FOLLOWER), a(HLP)), "userVaultEquity");
    let expected = viem("userVaultEquity");
    assert_eq!(equity.equity.to_string(), s(&expected["equity"]));
    assert_eq!(equity.locked_until_timestamp.to_string(), s(&expected["lockedUntilTimestamp"]));

    assert_eq!(sent(&withdrawable(a(HLP)), "withdrawable").to_string(), s(&viem("withdrawable")["withdrawable"]));

    let margin = sent(&account_margin_summary(0, a(SHORT)), "accountMarginSummary");
    let expected = viem("accountMarginSummary");
    assert_eq!(
        [
            margin.account_value.to_string(),
            margin.margin_used.to_string(),
            margin.ntl_pos.to_string(),
            margin.raw_usd.to_string()
        ],
        [s(&expected["accountValue"]), s(&expected["marginUsed"]), s(&expected["ntlPos"]), s(&expected["rawUsd"])]
    );
    assert!(sent(&core_user_exists(a(HLP)), "coreUserExists"));
}
