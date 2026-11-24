//! A backstop liquidator transfer: whole thousands of the quote token, in millionths, on a named
//! HIP-3 dex.

use super::*;

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

#[test]
fn a_transfer_is_whole_thousands_of_the_quote_token_in_millionths() {
    let deposit = Hip3LiquidatorTransfer::deposit("xyz", dec("1000")).unwrap();
    assert_eq!((deposit.dex(), deposit.ntl(), deposit.is_deposit()), ("xyz", 1_000_000_000, true));
    assert_eq!(deposit.wire(), r#"{"type":"hip3LiquidatorTransfer","dex":"xyz","ntl":1000000000,"isDeposit":true}"#);
    let withdrawal = Hip3LiquidatorTransfer::withdraw("xyz", dec("5000.000")).unwrap();
    assert_eq!((withdrawal.ntl(), withdrawal.is_deposit()), (5_000_000_000, false));
    assert_eq!(Hip3LiquidatorTransfer::KEY, Signers::Either);
}

#[test]
fn an_amount_that_is_not_whole_thousands_or_a_missing_dex_is_refused() {
    for amount in ["999", "1500", "1000.5", "0", "-1000"] {
        assert_eq!(
            Hip3LiquidatorTransfer::deposit("xyz", dec(amount)),
            Err(BadLiquidatorTransfer::NotAStep(dec(amount))),
            "{amount}"
        );
    }
    assert_eq!(Hip3LiquidatorTransfer::withdraw("", dec("1000")), Err(BadLiquidatorTransfer::NoDex));
}
