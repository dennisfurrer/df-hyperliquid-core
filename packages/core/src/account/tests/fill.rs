//! A recorded `userFills` row and the info endpoint docs' builder fill, beside a recorded socket
//! `userFills` row from a liquidation (mainnet, 2026-10-08): one shape either way.

use super::*;

const READ: &str = r#"{"closedPnl":"0.0","coin":"AVAX","crossed":false,"dir":"Open Long","hash":"0xa166e3fa63c25663024b03f2e0da011a00307e4017465df020210d3d432e7cb8","oid":90542681,"px":"18.435","side":"B","startPosition":"26.86","sz":"93.53","time":1681222254710,"fee":"0.01","feeToken":"USDC","builderFee":"0.01","tid":118906512037719}"#;
const LIQUIDATION: &str = r#"{"coin":"BTC","px":"80432.0","sz":"0.02603","side":"B","time":1791480388855,"startPosition":"-12.56343","dir":"Close Short","closedPnl":"16.680024","hash":"0xeba0ed56a1ceefc8ed1a04462006440000dc053c3cc20e9a8f6998a960c2c9b3","oid":569015257022,"crossed":false,"fee":"-0.062809","tid":1051492541339376,"cloid":"0xf82e29e34219a1dcc66c6960c49a6f00","liquidation":{"liquidatedUser":"0xf467f1f46cad3f1041c765765be2891245144f6e","markPx":"80429.0","method":"market"},"feeToken":"USDC","twapId":null}"#;

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

#[test]
fn a_fill_keeps_what_only_some_fills_carry_as_options() {
    let read: Fill = serde_json::from_str(READ).unwrap();
    assert_eq!((read.builder_fee, read.cloid.as_deref(), read.liquidation.as_ref()), (Some(dec("0.01")), None, None));
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
}

#[test]
fn a_fill_without_its_fee_token_or_whether_it_crossed_is_refused() {
    for gone in ["\"feeToken\":\"USDC\",", "\"crossed\":false,"] {
        let text = LIQUIDATION.replace(gone, "");
        assert!(serde_json::from_str::<Fill>(&text).is_err(), "without {gone}");
    }
}
