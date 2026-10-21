//! Isolated margin moves USDC in millionths on the position's own side, and only on an isolated
//! position.

use super::*;
use crate::trade::margin::margin;
use crate::trade::ticket_support::*;
use crate::trade::*;
use crate::utils::{BadUnits, Dec};

#[test]
fn isolated_margin_moves_millionths_on_the_position_s_side() {
    let short = dec("-2");
    assert_eq!(
        wire(margin(&tsla(), short, MarginMode::Isolated, dec("12.5"))),
        r#"{"type":"updateIsolatedMargin","asset":110001,"isBuy":false,"ntli":12500000}"#
    );
    assert_eq!(
        wire(margin(&tsla(), short, MarginMode::Isolated, dec("-12.5"))),
        r#"{"type":"updateIsolatedMargin","asset":110001,"isBuy":false,"ntli":-12500000}"#
    );
    for (market, position, mode, refused) in [
        (tsla(), dec("2"), MarginMode::Cross, Refusal::CrossPosition),
        (tsla(), Dec::ZERO, MarginMode::Isolated, Refusal::NoPosition),
        (hype(), dec("2"), MarginMode::Isolated, Refusal::Spot("margin")),
    ] {
        assert_eq!(margin(&market, position, mode, dec("12.5")), Err(refused));
    }
    let asset = tsla().asset();
    assert_eq!(add_margin(asset, Side::Sell, dec("1")).map(|add| (add.asset(), add.ntli())), Ok((asset, 1_000_000)));
    assert_eq!(remove_margin(asset, Side::Sell, dec("1")).map(|out| out.ntli()), Ok(-1_000_000));
    for moved in [add_margin(asset, Side::Sell, dec("-1")), remove_margin(asset, Side::Sell, Dec::ZERO)] {
        assert!(matches!(moved, Err(TradeError::Margin(BadUnits::NotPositive(_)))), "{moved:?}");
    }
}
