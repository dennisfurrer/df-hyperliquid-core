//! A scale: resting limits evenly spaced from one price to another, the size split evenly, the last
//! level the far price itself; `Gtc` or `Alo` only, 2 to 50 levels.

use super::*;
use crate::trade::orders::scale;
use crate::trade::ticket_support::*;
use crate::trade::*;

#[test]
fn a_scale_splits_the_size_across_evenly_spaced_levels() {
    let leg = |price: &str| {
        format!(r#"{{"a":0,"b":true,"p":"{price}","s":"0.001","r":false,"t":{{"limit":{{"tif":"Gtc"}}}}}}"#)
    };
    let ladder = Scale::new(Side::Buy, dec("0.003"), dec("83000"), dec("82000"), 3, Tif::Gtc);
    assert_eq!(
        wire(scale(&btc(), &ladder, None)),
        format!(r#"{{"type":"order","orders":[{},{},{}],"grouping":"na"}}"#, leg("83000"), leg("82500"), leg("82000"))
    );
    let reducing = Scale { reduce_only: true, ..ladder };
    assert!(scale(&btc(), &reducing, None).unwrap().orders().iter().all(Order::is_reduce_only));
    for (market, intent, refused) in [
        (btc(), Scale { tif: Tif::Ioc, ..ladder }, Refusal::ScaleTif(Tif::Ioc)),
        (btc(), Scale { levels: 1, ..ladder }, Refusal::Levels(1)),
        (btc(), Scale { levels: 51, ..ladder }, Refusal::Levels(51)),
        (
            btc(),
            Scale { size: dec("0.0003"), ..ladder },
            Refusal::UnderMinimum { size: dec("0.0001"), price: dec("83000"), minimum: dec("10") },
        ),
        (hype(), reducing, Refusal::Spot("reduce only")),
    ] {
        assert_eq!(scale(&market, &intent, None), Err(refused), "{intent:?}");
    }
}
