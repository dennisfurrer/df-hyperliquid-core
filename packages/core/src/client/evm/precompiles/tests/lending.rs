//! Lending reads against HyperEVM mainnet's answers and viem's reading of them; with no live
//! account to answer it, one user state viem encoded.

use crate::client::evm::fixtures;
use crate::client::evm::precompiles::support::{FOLLOWER, a, live, s, sent, viem};

use super::*;

#[test]
fn lending_reads_as_the_chain_wrote_it() {
    let reserve = sent(&borrow_lend_reserve_state(0), "borrowLendReserveState");
    let expected = viem("borrowLendReserveState");
    assert_eq!(
        [
            reserve.borrow_yearly_rate_bps,
            reserve.supply_yearly_rate_bps,
            reserve.balance,
            reserve.utilization_bps,
            reserve.oracle_px,
            reserve.ltv_bps,
            reserve.total_supplied,
            reserve.total_borrowed,
        ]
        .map(|one| one.to_string()),
        [
            "borrowYearlyRateBps",
            "supplyYearlyRateBps",
            "balance",
            "utilizationBps",
            "oraclePx",
            "ltvBps",
            "totalSupplied",
            "totalBorrowed"
        ]
        .map(|key| s(&expected[key]))
    );
    // No live account answered one; viem made this.
    let file = fixtures::viem();
    let state =
        borrow_lend_user_state(a(FOLLOWER), 0).decode(&fixtures::bytes(&file["borrowLend"]["encoded"])).unwrap();
    assert_eq!(
        state,
        BorrowLendUserState {
            borrow: BasisAndValue { basis: 1_000_000, value: 1_000_350 },
            supply: BasisAndValue { basis: 0, value: 0 },
        }
    );
    let (_, data, _) = live("borrowLendUserState");
    assert_eq!(crate::utils::hex::encode_prefixed(&borrow_lend_user_state(a(FOLLOWER), 0).data), data);
}
