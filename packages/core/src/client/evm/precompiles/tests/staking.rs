//! Staking reads against HyperEVM mainnet's answers, viem's reading of them, and the info API.

use crate::client::evm::precompiles::support::{DELEGATOR, a, s, sent, viem};
use crate::utils::Dec;

use super::*;

#[test]
fn staking_reads_as_the_chain_wrote_it() {
    let delegations = sent(&delegations(a(DELEGATOR)), "delegations");
    let expected = viem("delegations");
    assert_eq!(delegations.len(), expected.as_array().unwrap().len());
    for (one, said) in delegations.iter().zip(expected.as_array().unwrap()) {
        assert_eq!(one.validator.to_hex(), s(&said["validator"]));
        assert_eq!(one.amount.to_string(), s(&said["amount"]));
        assert_eq!(one.locked_until_timestamp.to_string(), s(&said["lockedUntilTimestamp"]));
    }
    // The info API said 6446.56647816 HYPE: amounts are HYPE at eight decimals.
    assert_eq!(Dec::new(i128::from(delegations[0].amount), 8), Dec::parse("6446.56647816").ok());
    let summary = sent(&delegator_summary(a(DELEGATOR)), "delegatorSummary");
    let expected = viem("delegatorSummary");
    assert_eq!(
        [summary.delegated, summary.undelegated, summary.total_pending_withdrawal, summary.n_pending_withdrawals]
            .map(|one| one.to_string()),
        [
            s(&expected["delegated"]),
            s(&expected["undelegated"]),
            s(&expected["totalPendingWithdrawal"]),
            s(&expected["nPendingWithdrawals"])
        ]
    );
}
