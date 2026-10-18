//! Each enum's codes as the docs' table gives them, read both ways.

use super::*;

#[test]
fn enum_codes_read_both_ways() {
    for (tif, code) in [(Tif::Alo, 1), (Tif::Gtc, 2), (Tif::Ioc, 3), (Tif::Other(0), 0)] {
        assert_eq!((tif.code(), Tif::from_code(code)), (code, tif));
    }
    for (op, code) in [
        (BorrowLendOp::Supply, 0),
        (BorrowLendOp::Withdraw, 1),
        (BorrowLendOp::Borrow, 2),
        (BorrowLendOp::Repay, 3),
        (BorrowLendOp::Other(4), 4),
    ] {
        assert_eq!((op.code(), BorrowLendOp::from_code(code)), (code, op));
    }
    for (variant, code) in
        [(FinalizeVariant::Create, 1), (FinalizeVariant::FirstStorageSlot, 2), (FinalizeVariant::CustomStorageSlot, 3)]
    {
        assert_eq!((variant.code(), FinalizeVariant::from_code(code)), (code, variant));
    }
    for (mode, code) in
        [(Abstraction::Disabled, 1), (Abstraction::UnifiedAccount, 2), (Abstraction::PortfolioMargin, 3)]
    {
        assert_eq!((mode.code(), Abstraction::from_code(code)), (code, mode));
    }
    for (op, code) in [
        (OutcomeOp::SplitOutcome, 0),
        (OutcomeOp::MergeOutcome, 1),
        (OutcomeOp::MergeQuestion, 2),
        (OutcomeOp::NegateOutcome, 3),
    ] {
        assert_eq!((op.code(), OutcomeOp::from_code(code)), (code, op));
    }
}
