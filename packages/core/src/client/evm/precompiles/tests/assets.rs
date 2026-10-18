//! Asset, token and supply reads against HyperEVM mainnet's answers and viem's reading of them.

use crate::client::evm::fixtures;
use crate::client::evm::precompiles::support::{s, sent, viem};

use super::*;

#[test]
fn assets_tokens_and_supplies_read_as_the_chain_wrote_them() {
    let info = sent(&perp_asset_info(0), "perpAssetInfo");
    let expected = viem("perpAssetInfo");
    assert_eq!(info.coin, "BTC");
    assert_eq!(
        [
            info.margin_table_id.to_string(),
            info.sz_decimals.to_string(),
            info.max_leverage.to_string(),
            info.only_isolated.to_string()
        ],
        [
            s(&expected["marginTableId"]),
            s(&expected["szDecimals"]),
            s(&expected["maxLeverage"]),
            s(&expected["onlyIsolated"])
        ]
    );
    let pair = sent(&spot_info(107), "spotInfo");
    assert_eq!(pair.name, fixtures::text(&viem("spotInfo")["name"]));
    assert_eq!(
        pair.tokens.map(|one| one.to_string()),
        [s(&viem("spotInfo")["tokens"][0]), s(&viem("spotInfo")["tokens"][1])]
    );

    let token = sent(&token_info(150), "tokenInfo");
    let expected = viem("tokenInfo");
    assert_eq!(token.name, "HYPE");
    let spots: Vec<String> = expected["spots"].as_array().unwrap().iter().map(s).collect();
    assert_eq!(token.spots.iter().map(u64::to_string).collect::<Vec<_>>(), spots);
    assert_eq!(token.deployer_trading_fee_share.to_string(), s(&expected["deployerTradingFeeShare"]));
    assert_eq!(token.deployer.to_hex(), s(&expected["deployer"]));
    assert_eq!(token.evm_contract.to_hex(), s(&expected["evmContract"]));
    assert_eq!(
        [token.sz_decimals.to_string(), token.wei_decimals.to_string(), token.evm_extra_wei_decimals.to_string()],
        [s(&expected["szDecimals"]), s(&expected["weiDecimals"]), s(&expected["evmExtraWeiDecimals"])]
    );

    let supply = sent(&token_supply(1), "tokenSupply");
    let expected = viem("tokenSupply");
    assert_eq!(
        [supply.max_supply, supply.total_supply, supply.circulating_supply, supply.future_emissions]
            .map(|one| one.to_string()),
        [
            s(&expected["maxSupply"]),
            s(&expected["totalSupply"]),
            s(&expected["circulatingSupply"]),
            s(&expected["futureEmissions"])
        ]
    );
    let holders = expected["nonCirculatingUserBalances"].as_array().unwrap();
    assert_eq!(supply.non_circulating_user_balances.len(), holders.len());
    for (one, said) in supply.non_circulating_user_balances.iter().zip(holders) {
        assert_eq!((one.user.to_hex(), one.balance.to_string()), (s(&said["user"]), s(&said["balance"])));
    }
}
