//! Each `perpDeploy` variant's posted JSON, per-coin lists sorted bytewise whatever order they were
//! given in, and what the constructors refuse. Hashes and signatures against the reference SDKs
//! are in `tests/conformance/deploy.rs`.

use super::*;

const USER: &str = "0x0d1d9635d0640821d15e323ac8adadfa9c111414";

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

fn xyz() -> DexName {
    "xyz".parse().unwrap()
}

fn user() -> Address {
    USER.parse().unwrap()
}

fn asset() -> NewAsset {
    NewAsset { coin: "xyz:TEST".to_owned(), sz_decimals: 2, oracle_px: dec("12.50"), margin_table_id: 3 }
}

#[test]
fn registering_sends_null_for_no_gas_cap_no_schema_and_no_oracle_updater() {
    let bare = PerpDeploy::register_asset(xyz(), None, asset(), true, None).unwrap();
    assert_eq!(
        bare.wire(),
        r#"{"type":"perpDeploy","registerAsset":{"maxGas":null,"assetRequest":{"coin":"xyz:TEST","szDecimals":2,"oraclePx":"12.5","marginTableId":3,"onlyIsolated":true},"dex":"xyz","schema":null}}"#
    );
    let schema = DexSchema { full_name: "Test dex".to_owned(), collateral_token: 0, oracle_updater: None };
    let new_dex = PerpDeploy::register_asset2(xyz(), Some(0), asset(), MarginMode::Normal, Some(schema)).unwrap();
    assert_eq!(
        new_dex.wire(),
        r#"{"type":"perpDeploy","registerAsset2":{"maxGas":0,"assetRequest":{"coin":"xyz:TEST","szDecimals":2,"oraclePx":"12.5","marginTableId":3,"marginMode":"normal"},"dex":"xyz","schema":{"fullName":"Test dex","collateralToken":0,"oracleUpdater":null}}}"#
    );
    let negative = NewAsset { oracle_px: dec("-1"), ..asset() };
    assert!(matches!(
        PerpDeploy::register_asset2(xyz(), None, negative, MarginMode::NoCross, None),
        Err(BadDeploy::OutOfRange { what: "oraclePx", .. })
    ));
}

#[test]
fn per_coin_lists_go_out_sorted_bytewise_and_a_coin_given_twice_is_refused() {
    let oracle = PerpDeploy::set_oracle(
        xyz(),
        [("xyz:b", dec("2")), ("xyz:B", dec("1.0"))],
        vec![vec![("xyz:b", dec("2.1")), ("xyz:B", dec("1.1"))]],
        [("xyz:b", dec("2")), ("xyz:B", dec("1"))],
    )
    .unwrap();
    assert_eq!(
        oracle.wire(),
        r#"{"type":"perpDeploy","setOracle":{"dex":"xyz","oraclePxs":[["xyz:B","1"],["xyz:b","2"]],"markPxs":[[["xyz:B","1.1"],["xyz:b","2.1"]]],"externalPerpPxs":[["xyz:B","1"],["xyz:b","2"]]}}"#
    );
    assert_eq!(
        PerpDeploy::set_oracle(xyz(), [("xyz:A", dec("1")), ("xyz:A", dec("2"))], vec![], []),
        Err(BadDeploy::Twice("xyz:A".to_owned()))
    );
    assert_eq!(PerpDeploy::set_oracle::<&str>(xyz(), [], vec![vec![], vec![], vec![]], []), Err(BadDeploy::MarkPxs(3)));
    assert!(PerpDeploy::set_oracle(xyz(), [("xyz:A", dec("-1"))], vec![], []).is_err());

    let caps = PerpDeploy::set_open_interest_caps([("xyz:B", None), ("xyz:A", Some(5_000_000))]).unwrap();
    assert_eq!(caps.wire(), r#"{"type":"perpDeploy","setOpenInterestCaps":[["xyz:A",5000000],["xyz:B",null]]}"#);
    let modes = PerpDeploy::set_margin_modes([("xyz:B", MarginMode::Normal), ("xyz:A", MarginMode::StrictIsolated)]);
    assert_eq!(
        modes.unwrap().wire(),
        r#"{"type":"perpDeploy","setMarginModes":[["xyz:A","strictIsolated"],["xyz:B","normal"]]}"#
    );
}

#[test]
fn funding_settings_keep_to_their_ranges() {
    let multipliers = PerpDeploy::set_funding_multipliers([("xyz:B", dec("10")), ("xyz:A", dec("0.50"))]).unwrap();
    assert_eq!(multipliers.wire(), r#"{"type":"perpDeploy","setFundingMultipliers":[["xyz:A","0.5"],["xyz:B","10"]]}"#);
    assert!(PerpDeploy::set_funding_multipliers([("xyz:A", dec("10.01"))]).is_err());
    let rates = PerpDeploy::set_funding_interest_rates([("xyz:A", dec("-0.01")), ("xyz:B", dec("0.0001"))]).unwrap();
    assert_eq!(
        rates.wire(),
        r#"{"type":"perpDeploy","setFundingInterestRates":[["xyz:A","-0.01"],["xyz:B","0.0001"]]}"#
    );
    assert_eq!(
        PerpDeploy::set_funding_interest_rates([("xyz:A", dec("-0.011"))]),
        Err(BadDeploy::OutOfRange {
            what: "a funding interest rate",
            least: "-0.01",
            most: "0.01",
            value: "-0.011".to_owned()
        })
    );
    let clamps = PerpDeploy::set_funding_clamps([("xyz:A", dec("0.0003"))]).unwrap();
    assert_eq!(clamps.wire(), r#"{"type":"perpDeploy","setFundingClamps":[["xyz:A","0.0003"]]}"#);
    assert!(PerpDeploy::set_funding_clamps([("xyz:A", dec("-0.0001"))]).is_err());
    assert!(PerpDeploy::set_funding_clamps([("xyz:A", dec("0.0101"))]).is_err());
}

#[test]
fn deployer_fees_take_up_to_three_or_just_under_ten_in_growth_mode() {
    let fee = |scale: &str, growth_mode| DeployerFee { scale: dec(scale), growth_mode };
    let fees = PerpDeploy::set_deployer_fees([("xyz:B", fee("9.99", true)), ("xyz:A", fee("3", false))]).unwrap();
    assert_eq!(
        fees.wire(),
        r#"{"type":"perpDeploy","setDeployerFees":[["xyz:A",{"scale":"3","growthMode":false}],["xyz:B",{"scale":"9.99","growthMode":true}]]}"#
    );
    for (scale, growth_mode) in [("3.01", false), ("10", true), ("-0.1", true), ("-0.1", false)] {
        assert!(PerpDeploy::set_deployer_fees([("xyz:A", fee(scale, growth_mode))]).is_err(), "{scale} {growth_mode}");
    }
}

#[test]
fn margin_tables_take_one_to_three_ordered_tiers_and_ids_are_never_zero() {
    let tier = |lower_bound, max_leverage| MarginTier { lower_bound, max_leverage };
    let table = MarginTable::new("tiered 50x", vec![tier(0, 50), tier(1_000_000, 20)]).unwrap();
    assert_eq!(
        PerpDeploy::InsertMarginTable { dex: xyz(), margin_table: table }.wire(),
        r#"{"type":"perpDeploy","insertMarginTable":{"dex":"xyz","marginTable":{"description":"tiered 50x","marginTiers":[{"lowerBound":0,"maxLeverage":50},{"lowerBound":1000000,"maxLeverage":20}]}}}"#
    );
    for refused in [
        vec![],
        vec![tier(0, 51)],
        vec![tier(0, 0)],
        vec![tier(0, 10), tier(0, 5)],
        vec![tier(0, 10), tier(5, 10)],
        vec![tier(0, 40), tier(1, 30), tier(2, 20), tier(3, 10)],
    ] {
        assert_eq!(MarginTable::new("", refused.clone()), Err(BadDeploy::MarginTiers), "{refused:?}");
    }
    let ids = PerpDeploy::set_margin_table_ids([("xyz:B", 2), ("xyz:A", 1)]).unwrap();
    assert_eq!(ids.wire(), r#"{"type":"perpDeploy","setMarginTableIds":[["xyz:A",1],["xyz:B",2]]}"#);
    assert_eq!(PerpDeploy::set_margin_table_ids([("xyz:A", 0)]), Err(BadDeploy::MarginTableZero("xyz:A".to_owned())));
}

#[test]
fn the_remaining_variants_carry_their_fields_in_the_docs_order() {
    assert_eq!(
        PerpDeploy::HaltTrading { coin: "xyz:A".to_owned(), is_halted: true }.wire(),
        r#"{"type":"perpDeploy","haltTrading":{"coin":"xyz:A","isHalted":true}}"#
    );
    assert_eq!(
        PerpDeploy::SetFeeRecipient { dex: xyz(), fee_recipient: user() }.wire(),
        format!(r#"{{"type":"perpDeploy","setFeeRecipient":{{"dex":"xyz","feeRecipient":"{USER}"}}}}"#)
    );
    let grant = SubDeployer { variant: PerpKind::SetOracle, user: user(), allowed: true };
    assert_eq!(
        PerpDeploy::set_sub_deployers(xyz(), vec![grant]).unwrap().wire(),
        format!(
            r#"{{"type":"perpDeploy","setSubDeployers":{{"dex":"xyz","subDeployers":[{{"variant":"setOracle","user":"{USER}","allowed":true}}]}}}}"#
        )
    );
    assert_eq!(PerpDeploy::set_sub_deployers(xyz(), vec![]), Err(BadDeploy::NoGrants));
    let annotation = Annotation::new("index", "An index.", Some("IDX".to_owned()), vec![]).unwrap();
    assert_eq!(
        PerpDeploy::SetPerpAnnotation { coin: "xyz:A".to_owned(), annotation }.wire(),
        r#"{"type":"perpDeploy","setPerpAnnotation":{"coin":"xyz:A","category":"index","description":"An index.","displayName":"IDX","keywords":[]}}"#
    );
    assert_eq!(PerpDeploy::DisableDex(xyz()).wire(), r#"{"type":"perpDeploy","disableDex":"xyz"}"#);
}

#[test]
fn every_kind_names_its_own_variant_once() {
    let names: std::collections::BTreeSet<&str> = PerpKind::ALL.iter().map(|kind| kind.wire()).collect();
    assert_eq!(names.len(), PerpKind::ALL.len());
    assert_eq!(PerpDeploy::DisableDex(xyz()).kind(), PerpKind::DisableDex);
}
