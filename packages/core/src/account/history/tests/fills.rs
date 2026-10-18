//! A fill and the fee answer as the venue writes them. Live answers, mainnet, 2026-10-08, from one
//! account: fills of each direction and fee token, and its fee schedule. Beside them the venue's
//! own builder fill from its info endpoint docs, since no recorded fill paid one, a recorded socket
//! `userFills` row from a liquidation, and two more `userFees` answers, one trimmed to what a fee
//! figure needs. The reads run in the mocked journeys.

use super::*;
use crate::account::fixtures::FEES_UNIFIED;

const FILLS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/trade/user_fills.json"));
const FEES: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/trade/user_fees.json"));

/// The info endpoint page's perp fill, comments taken out.
const DOCS_FILL: &str = r#"{"closedPnl":"0.0","coin":"AVAX","crossed":false,"dir":"Open Long","hash":"0xa166e3fa63c25663024b03f2e0da011a00307e4017465df020210d3d432e7cb8","oid":90542681,"px":"18.435","side":"B","startPosition":"26.86","sz":"93.53","time":1681222254710,"fee":"0.01","feeToken":"USDC","builderFee":"0.01","tid":118906512037719}"#;

const LIQUIDATION: &str = r#"{"coin":"BTC","px":"80432.0","sz":"0.02603","side":"B","time":1791480388855,"startPosition":"-12.56343","dir":"Close Short","closedPnl":"16.680024","hash":"0xeba0ed56a1ceefc8ed1a04462006440000dc053c3cc20e9a8f6998a960c2c9b3","oid":569015257022,"crossed":false,"fee":"-0.062809","tid":1051492541339376,"cloid":"0xf82e29e34219a1dcc66c6960c49a6f00","liquidation":{"liquidatedUser":"0xf467f1f46cad3f1041c765765be2891245144f6e","markPx":"80429.0","method":"market"},"feeToken":"USDC","twapId":null}"#;
/// The live `userFees` shape, its schedule trimmed to the base tier.
const FEES_TRIMMED: &str = r#"{"dailyUserVlm":[
    {"date":"2026-09-17","userCross":"1498565.3600000001","userAdd":"0.0","exchange":"6184805063.68"},
    {"date":"2026-09-18","userCross":"0.0","userAdd":"250.5","exchange":"10088730521.48"},
    {"date":"2026-09-19","userCross":"12.25","userAdd":"0.0","exchange":"3362590973.46"}],
    "feeSchedule":{"cross":"0.00045","add":"0.00015","spotCross":"0.0007","spotAdd":"0.0004"},
    "userCrossRate":"0.00045","userAddRate":"0.00015","userSpotCrossRate":"0.0007",
    "userSpotAddRate":"0.0004","activeReferralDiscount":"0.0","trial":null,
    "nextTrialAvailableTimestamp":null,"stakingLink":null}"#;

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

#[test]
fn a_fill_reads_every_figure_exactly_keeps_the_rare_ones_optional_and_needs_its_fee_token() {
    let fills: Vec<Fill> = serde_json::from_str(FILLS).unwrap();
    assert_eq!(fills.len(), 5);
    let close = &fills[0];
    assert_eq!((close.coin.as_str(), close.side, close.dir.as_str()), ("GRAM", Side::Sell, "Close Long"));
    assert_eq!((close.px, close.sz, close.closed_pnl), (dec("1.3415"), dec("423"), dec("-34.940646")));
    assert_eq!((close.fee, close.fee_token.as_str(), close.builder_fee), (dec("0.192934"), "USDC", None));
    assert_eq!(
        (close.oid, close.tid, close.crossed, close.twap_id),
        (569_016_338_230, 196_547_699_056_480, true, None)
    );
    let dust = &fills[2];
    assert_eq!((dust.dir.as_str(), dust.sz, dust.tid), ("Spot Dust Conversion", dec("0.00000001"), 0));
    assert_eq!(
        (fills[4].fee_token.as_str(), fills[4].fee),
        ("UZEC", dec("0.000362456")),
        "a spot buy pays in the token"
    );

    let built: Fill = serde_json::from_str(DOCS_FILL).unwrap();
    assert_eq!((built.fee, built.builder_fee), (dec("0.01"), Some(dec("0.01"))), "the fee includes the builder's");
    assert_eq!((built.start_position, built.cloid, built.liquidation), (dec("26.86"), None, None));
    let liquidated: Fill = serde_json::from_str(LIQUIDATION).unwrap();
    assert_eq!((liquidated.builder_fee, liquidated.twap_id), (None, None));
    assert_eq!(liquidated.cloid.as_deref(), Some("0xf82e29e34219a1dcc66c6960c49a6f00"));
    assert_eq!(
        liquidated.liquidation,
        Some(FillLiquidation {
            liquidated_user: Some("0xf467f1f46cad3f1041c765765be2891245144f6e".parse().unwrap()),
            mark_px: dec("80429"),
            method: "market".to_owned(),
        })
    );
    for gone in ["\"feeToken\":\"USDC\",", "\"crossed\":false,"] {
        let text = LIQUIDATION.replace(gone, "");
        assert!(serde_json::from_str::<Fill>(&text).is_err(), "without {gone}");
    }
}

#[test]
fn the_fee_answer_reads_as_fractions_and_sums_every_day_it_sends() {
    let fees: UserFees = serde_json::from_str(FEES).unwrap();
    assert_eq!((fees.user_cross_rate, fees.user_add_rate), (dec("0.00034"), dec("0.000102")));
    assert_eq!((fees.user_spot_cross_rate, fees.user_spot_add_rate), (dec("0.00051"), dec("0.000255")));
    assert_eq!(fees.fee_schedule.cross, dec("0.00045"));
    assert_eq!(fees.fee_schedule.tiers.vip[0].ntl_cutoff, dec("5000000"));
    assert_eq!(fees.fee_schedule.tiers.mm[0].add, dec("-0.00001"), "a maker rebate is negative");
    assert_eq!(fees.fee_schedule.referral_discount, dec("0.04"));
    assert_eq!(fees.active_staking_discount.map(|tier| tier.discount), Some(dec("0.15")));
    assert_eq!(fees.daily_user_vlm[0].exchange, dec("9424822576.8999996185"));
    assert_eq!(fees.active_referral_discount, Dec::ZERO);

    // A live account at a VIP tier: its own rates under the base schedule.
    let tiered: UserFees = serde_json::from_str(FEES_UNIFIED).unwrap();
    assert_eq!(
        (tiered.user_cross_rate, tiered.user_add_rate, tiered.user_spot_cross_rate, tiered.user_spot_add_rate),
        (dec("0.0003"), dec("0.00004"), dec("0.0004"), dec("0.0001"))
    );
    assert_eq!((tiered.fee_schedule.cross, tiered.fee_schedule.add), (dec("0.00045"), dec("0.00015")), "the base");
    assert_eq!(tiered.active_staking_discount.map(|staking| staking.discount), Some(Dec::ZERO));
    assert_eq!(tiered.daily_user_vlm.len(), 15, "fourteen days and today");

    for (said, volume) in [
        (FEES_TRIMMED, "1498828.1100000001"),
        (FEES_UNIFIED, "131872229.1499999978"),
        (r#"{"userCrossRate":"0.00045"}"#, "0"),
    ] {
        let fees: UserFees = serde_json::from_str(said).unwrap();
        assert_eq!(fees.volume(), Some(dec(volume)), "every day's taking and making, exactly: {said}");
    }
}
