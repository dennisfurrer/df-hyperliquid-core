use super::*;
use crate::trade::margin::leverage;
use crate::trade::ticket_support::*;

#[test]
fn leverage_respects_the_market_s_ceiling_and_margin_mode() {
    assert_eq!(
        wire(leverage(&tsla(), MarginMode::Isolated, 10)),
        r#"{"type":"updateLeverage","asset":110001,"isCross":false,"leverage":10}"#
    );
    assert_eq!(leverage(&tsla(), MarginMode::Cross, 10), Err(Refusal::IsolatedOnly));
    assert_eq!(
        leverage(&tsla(), MarginMode::Isolated, 11).unwrap_err().to_string(),
        "leverage is a whole number from 1 to 10, not 11"
    );
    assert_eq!(leverage(&tsla(), MarginMode::Isolated, 0), Err(Refusal::Leverage { asked: 0, most: 10 }));
    assert_eq!(leverage(&hype(), MarginMode::Cross, 2), Err(Refusal::Spot("leverage")));
}
