use super::*;
use crate::trade::orders::scale;
use crate::trade::ticket_support::*;
use crate::trade::*;

#[test]
fn a_scale_splits_the_size_across_evenly_spaced_levels() {
    let leg = |price: &str| {
        format!(r#"{{"a":0,"b":true,"p":"{price}","s":"0.001","r":false,"t":{{"limit":{{"tif":"Gtc"}}}}}}"#)
    };
    let mut intent = Scale::new(Side::Buy, dec("0.003"), dec("83000"), dec("82000"), 3, Tif::Gtc);
    assert_eq!(
        wire(scale(&btc(), &intent, None)),
        format!(r#"{{"type":"order","orders":[{},{},{}],"grouping":"na"}}"#, leg("83000"), leg("82500"), leg("82000"))
    );
    intent.tif = Tif::Ioc;
    assert_eq!(scale(&btc(), &intent, None), Err(Refusal::ScaleTif(Tif::Ioc)));
    intent.tif = Tif::Gtc;
    for levels in [1, 51] {
        intent.levels = levels;
        assert_eq!(scale(&btc(), &intent, None), Err(Refusal::Levels(levels)));
    }
}
