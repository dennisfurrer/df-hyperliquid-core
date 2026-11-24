//! `deploy_actions.json`: every `spotDeploy` and `perpDeploy` variant, `finalizeEvmContract`,
//! `validatorL1Stream` and `authorizeAqav2Role`, each built through the typed constructors from the
//! inputs the reference SDK was given, then held to its posted JSON, msgpack, hash and mainnet
//! signature. Three sources: `@nktkas/hyperliquid` 0.33.3, Hyperliquid's Python SDK 0.24.0, and the
//! docs' shapes neither SDK builds, signed by both SDKs' L1 signers.

use df_hyperliquid_core::account::LocalKey;
use df_hyperliquid_core::client::Network;
use df_hyperliquid_core::deploy::perp::{
    DeployerFee, DexSchema, MarginMode, MarginTable, MarginTier, NewAsset, PerpKind, SubDeployer,
};
use df_hyperliquid_core::deploy::spot::EvmFinalizer;
use df_hyperliquid_core::deploy::{
    Annotation, AqaRole, AuthorizeAqav2Role, DexName, FinalizeEvmContract, PerpDeploy, SpotDeploy, TokenSpec,
    ValidatorL1Stream,
};
use df_hyperliquid_core::utils::sign::L1Action;
use df_hyperliquid_core::utils::{Address, Dec, Json, hex};

const VECTORS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/deploy_actions.json"));

const USER: &str = "0x0d1d9635d0640821d15e323ac8adadfa9c111414";
const USER2: &str = "0x2222222222222222222222222222222222222222";
const CONTRACT: &str = "0x9b498c3c8a0b8cd8ba1d9851d40d186f1872b44e";
const TOKEN: u32 = 1234;

fn text<'a>(json: &'a Json, key: &str) -> &'a str {
    json.get(key).and_then(Json::as_str).unwrap_or_else(|| panic!("no {key}"))
}

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

fn address(text: &str) -> Address {
    text.parse().unwrap()
}

fn xyz() -> DexName {
    "xyz".parse().unwrap()
}

fn spec() -> TokenSpec {
    TokenSpec { name: "TEST".to_owned(), sz_decimals: 2, wei_decimals: 8 }
}

fn asset(coin: &str, sz_decimals: u32, oracle_px: &str, margin_table_id: u32) -> NewAsset {
    NewAsset { coin: coin.to_owned(), sz_decimals, oracle_px: dec(oracle_px), margin_table_id }
}

fn test_dex(oracle_updater: Option<&str>) -> Option<DexSchema> {
    Some(DexSchema {
        full_name: "Test dex".to_owned(),
        collateral_token: 0,
        oracle_updater: oracle_updater.map(address),
    })
}

/// Per-coin decimals, typed as a caller types them.
fn coins<'a>(pairs: &[(&'a str, &str)]) -> Vec<(&'a str, Dec)> {
    pairs.iter().map(|(coin, value)| (*coin, dec(value))).collect()
}

fn annotation(category: &str, description: &str, display_name: Option<&str>, keywords: &[&str]) -> Annotation {
    let keywords = keywords.iter().map(|word| (*word).to_owned()).collect();
    Annotation::new(category, description, display_name.map(str::to_owned), keywords).unwrap()
}

/// The case's action, built from the inputs its SDK was given. Per-coin lists are given out of order
/// where the SDK sorted them, so the sorting is this crate's.
fn built(name: &str) -> Box<dyn L1Action> {
    let spot = |action: SpotDeploy| -> Box<dyn L1Action> { Box::new(action) };
    let perp = |action: PerpDeploy| -> Box<dyn L1Action> { Box::new(action) };
    match name {
        "registerToken2" => spot(SpotDeploy::RegisterToken2 {
            spec: spec(),
            max_gas: 1_000_000_000_000,
            full_name: Some("Test token".to_owned()),
        }),
        "registerToken2NoFullName" => spot(SpotDeploy::RegisterToken2 { spec: spec(), max_gas: 500, full_name: None }),
        "userGenesis" => spot(SpotDeploy::UserGenesis {
            token: TOKEN,
            user_and_wei: vec![(address(USER), 100_000_000_000_000), (address(USER2), 5)],
            existing_token_and_wei: vec![(1, 100_000_000_000_000)],
            blacklist_users: vec![],
        }),
        "userGenesisBlacklist" => spot(SpotDeploy::UserGenesis {
            token: TOKEN,
            user_and_wei: vec![],
            existing_token_and_wei: vec![],
            blacklist_users: vec![(address(USER), true), (address(USER2), false)],
        }),
        "genesis" | "genesisNoHyperliquidity" => spot(SpotDeploy::Genesis {
            token: TOKEN,
            max_supply: 300_000_000_000_000,
            no_hyperliquidity: name == "genesisNoHyperliquidity",
        }),
        "registerSpot" => spot(SpotDeploy::RegisterSpot { base: TOKEN, quote: 0 }),
        "registerHyperliquidity" => {
            spot(SpotDeploy::register_hyperliquidity(210, dec("2.50"), dec("4"), 100, None).unwrap())
        }
        "registerHyperliquiditySeeded" => {
            spot(SpotDeploy::register_hyperliquidity(210, dec("0.0125"), dec("1000.5"), 20, Some(10)).unwrap())
        }
        "setDeployerTradingFeeShare" => {
            spot(SpotDeploy::SetDeployerTradingFeeShare { token: TOKEN, share: "99.4%".parse().unwrap() })
        }
        "enableQuoteToken" => spot(SpotDeploy::EnableQuoteToken(TOKEN)),
        "disableQuoteToken" => spot(SpotDeploy::DisableQuoteToken(TOKEN)),
        "enableAlignedQuoteToken" => spot(SpotDeploy::EnableAlignedQuoteToken(TOKEN)),
        "disableAlignedQuoteToken" => spot(SpotDeploy::DisableAlignedQuoteToken(TOKEN)),
        "enableFreezePrivilege" => spot(SpotDeploy::EnableFreezePrivilege(TOKEN)),
        "revokeFreezePrivilege" => spot(SpotDeploy::RevokeFreezePrivilege(TOKEN)),
        "freezeUser" => spot(SpotDeploy::FreezeUser { token: TOKEN, user: address(USER), freeze: true }),
        "unfreezeUser" => spot(SpotDeploy::FreezeUser { token: TOKEN, user: address(USER2), freeze: false }),
        "setTokenAnnotation" => spot(SpotDeploy::SetTokenAnnotation {
            token: TOKEN,
            annotation: annotation("meme", "A cat.", Some("PURR"), &["cat", "meme"]),
        }),
        "setTokenAnnotationNoDisplayName" => {
            spot(SpotDeploy::SetTokenAnnotation { token: TOKEN, annotation: annotation("", "", None, &[]) })
        }
        "setDeployerLabel" => spot(SpotDeploy::SetDeployerLabel("dfu".parse().unwrap())),
        "requestEvmContract" => spot(SpotDeploy::request_evm_contract(TOKEN, address(CONTRACT), 13).unwrap()),
        "requestEvmContractFewerDecimals" => {
            spot(SpotDeploy::request_evm_contract(TOKEN, address(CONTRACT), -2).unwrap())
        }
        "finalizeEvmContractCreate" => {
            Box::new(FinalizeEvmContract { token: TOKEN, input: EvmFinalizer::Create { nonce: 4 } })
        }
        "finalizeEvmContractFirstStorageSlot" => {
            Box::new(FinalizeEvmContract { token: TOKEN, input: EvmFinalizer::FirstStorageSlot })
        }
        "finalizeEvmContractCustomStorageSlot" => {
            Box::new(FinalizeEvmContract { token: TOKEN, input: EvmFinalizer::CustomStorageSlot })
        }

        "registerAsset2" => perp(
            PerpDeploy::register_asset2(
                xyz(),
                Some(1_000_000_000_000),
                asset("xyz:TEST", 2, "12.5", 3),
                MarginMode::StrictIsolated,
                test_dex(Some(USER)),
            )
            .unwrap(),
        ),
        "registerAsset2Bare" => perp(
            PerpDeploy::register_asset2(xyz(), None, asset("xyz:TEST1", 0, "0.0010", 10), MarginMode::NoCross, None)
                .unwrap(),
        ),
        "registerAsset" => perp(
            PerpDeploy::register_asset(xyz(), Some(0), asset("xyz:TEST", 2, "12.5", 3), false, test_dex(None)).unwrap(),
        ),
        "registerAssetBare" => {
            perp(PerpDeploy::register_asset(xyz(), None, asset("xyz:TEST", 2, "12.5", 3), true, None).unwrap())
        }
        "registerAssetNewDex" => perp(
            PerpDeploy::register_asset(
                xyz(),
                Some(1_000_000_000_000),
                asset("xyz:TEST", 2, "12.5", 3),
                false,
                test_dex(Some("0x0D1D9635D0640821D15E323AC8ADADFA9C111414")),
            )
            .unwrap(),
        ),
        "setOracle" => perp(
            PerpDeploy::set_oracle(
                xyz(),
                coins(&[("xyz:abc", "0.5"), ("xyz:BTC", "100000")]),
                vec![coins(&[("xyz:abc", "0.51"), ("xyz:BTC", "100010")]), coins(&[("xyz:BTC", "99990")])],
                coins(&[("xyz:abc", "0.5"), ("xyz:BTC", "100001")]),
            )
            .unwrap(),
        ),
        "setOracleNoMarks" => perp(
            PerpDeploy::set_oracle(xyz(), coins(&[("xyz:BTC", "100000")]), vec![], coins(&[("xyz:BTC", "100000")]))
                .unwrap(),
        ),
        "setFundingMultipliers" => {
            perp(PerpDeploy::set_funding_multipliers(coins(&[("xyz:ETH", "10"), ("xyz:BTC", "0.5")])).unwrap())
        }
        "setFundingInterestRates" => {
            perp(PerpDeploy::set_funding_interest_rates(coins(&[("xyz:ETH", "0.0001"), ("xyz:BTC", "-0.01")])).unwrap())
        }
        "setFundingClamps" => {
            perp(PerpDeploy::set_funding_clamps(coins(&[("xyz:ETH", "0.01"), ("xyz:BTC", "0.0003")])).unwrap())
        }
        "haltTrading" => perp(PerpDeploy::HaltTrading { coin: "xyz:BTC".to_owned(), is_halted: true }),
        "insertMarginTable" => {
            let tiers = vec![
                MarginTier { lower_bound: 0, max_leverage: 40 },
                MarginTier { lower_bound: 150_000_000, max_leverage: 20 },
            ];
            perp(PerpDeploy::InsertMarginTable {
                dex: xyz(),
                margin_table: MarginTable::new("tiered 40x", tiers).unwrap(),
            })
        }
        "setMarginTableIds" => perp(PerpDeploy::set_margin_table_ids([("xyz:ETH", 2), ("xyz:BTC", 1)]).unwrap()),
        "setFeeRecipient" => perp(PerpDeploy::SetFeeRecipient { dex: xyz(), fee_recipient: address(USER) }),
        "setOpenInterestCaps" => perp(
            PerpDeploy::set_open_interest_caps([("xyz:ETH", Some(10_000_000_000)), ("xyz:BTC", Some(5_000_000))])
                .unwrap(),
        ),
        "setOpenInterestCapsRemoved" => {
            perp(PerpDeploy::set_open_interest_caps([("xyz:ETH", Some(5_000_000)), ("xyz:BTC", None)]).unwrap())
        }
        "setSubDeployers" => perp(
            PerpDeploy::set_sub_deployers(
                xyz(),
                vec![
                    SubDeployer { variant: PerpKind::SetOracle, user: address(USER), allowed: true },
                    SubDeployer { variant: PerpKind::HaltTrading, user: address(USER2), allowed: false },
                ],
            )
            .unwrap(),
        ),
        "setMarginModes" => perp(
            PerpDeploy::set_margin_modes([("xyz:ETH", MarginMode::NoCross), ("xyz:BTC", MarginMode::StrictIsolated)])
                .unwrap(),
        ),
        "setMarginModesNormal" => perp(PerpDeploy::set_margin_modes([("xyz:BTC", MarginMode::Normal)]).unwrap()),
        "setDeployerFees" => perp(
            PerpDeploy::set_deployer_fees([
                ("xyz:ETH", DeployerFee { scale: dec("9.99"), growth_mode: true }),
                ("xyz:BTC", DeployerFee { scale: dec("3.0"), growth_mode: false }),
            ])
            .unwrap(),
        ),
        "setPerpAnnotation" => perp(PerpDeploy::SetPerpAnnotation {
            coin: "xyz:BTC".to_owned(),
            annotation: annotation("crypto", "Bitcoin on xyz.", Some("BTC"), &["bitcoin", "btc"]),
        }),
        "setPerpAnnotationNoDisplayName" => perp(PerpDeploy::SetPerpAnnotation {
            coin: "xyz:ETH".to_owned(),
            annotation: annotation("crypto", "", None, &[]),
        }),
        "disableDex" => perp(PerpDeploy::DisableDex(xyz())),

        "validatorL1Stream" => Box::new(ValidatorL1Stream::new(dec("0.040")).unwrap()),
        "authorizeAqav2RoleTechnical" => Box::new(AuthorizeAqav2Role { token: 0, role: AqaRole::Technical }),
        "authorizeAqav2RoleTreasury" => Box::new(AuthorizeAqav2Role { token: TOKEN, role: AqaRole::Treasury }),
        other => panic!("no such case {other}"),
    }
}

/// The Python SDK writes `r` and `s` without leading zeros.
fn word(text: &str) -> String {
    format!("0x{:0>64}", text.trim_start_matches("0x"))
}

/// Every case of `section` against its typed build: the posted JSON, the msgpack (as recorded where
/// the file has it, else this crate's reading of the recorded JSON), the hash and the signature.
fn check_section(section: &str, count: usize) {
    let file = Json::parse(VECTORS).unwrap();
    let key = LocalKey::from_hex(text(&file, "key")).unwrap();
    let nonce = file.get("nonce").and_then(Json::as_u64).unwrap();
    let cases = file.get(section).and_then(|one| one.get("cases")).and_then(Json::as_array).unwrap();
    assert_eq!(cases.len(), count, "{section}");
    for case in cases {
        let name = text(case, "name");
        let typed = built(name);
        let recorded = case.get("action").unwrap();
        assert_eq!(typed.wire(), recorded.compact(), "{section} {name}: the action");
        assert_eq!(typed.msgpack(), recorded.to_action().unwrap().msgpack(), "{section} {name}: msgpack of the JSON");
        if let Some(Json::String(packed)) = case.get("msgpack") {
            assert_eq!(hex::encode_prefixed(&typed.msgpack()), *packed, "{section} {name}: msgpack");
        }
        let hash = hex::encode_prefixed(&typed.connection_id(nonce, None));
        assert_eq!(hash, text(case, "actionHash"), "{section} {name}: hash");
        let signature = key.sign_digest(&typed.digest(nonce, None, Network::Mainnet)).unwrap();
        let expected = case.get("signature").unwrap();
        assert_eq!(signature.r_hex(), word(text(expected, "r")), "{section} {name}: r");
        assert_eq!(signature.s_hex(), word(text(expected, "s")), "{section} {name}: s");
        assert_eq!(u64::from(signature.v()), expected.get("v").and_then(Json::as_u64).unwrap(), "{section} {name}: v");
    }
}

#[test]
fn every_nktkas_deploy_link_and_aligned_quote_action_comes_out_of_the_builders() {
    check_section("nktkas", 37);
}

#[test]
fn the_python_sdk_s_deploy_actions_come_out_of_the_builders() {
    check_section("python", 19);
}

#[test]
fn the_docs_shapes_neither_sdk_builds_sign_as_both_sdks_sign_them() {
    check_section("docs", 9);
}
