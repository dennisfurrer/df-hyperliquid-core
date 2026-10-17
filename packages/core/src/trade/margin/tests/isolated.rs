use super::*;
use crate::trade::margin::margin;
use crate::trade::ticket_support::*;
use crate::trade::*;
use crate::utils::Dec;

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
    assert_eq!(margin(&tsla(), dec("2"), MarginMode::Cross, dec("12.5")), Err(Refusal::CrossPosition));
    assert_eq!(margin(&tsla(), Dec::ZERO, MarginMode::Isolated, dec("12.5")), Err(Refusal::NoPosition));
}
