//! Money moved through the [`Client`]: USDC and a spot token to another account, USDC between an
//! account's own balances by its key or its agent, a token to HyperEVM and back, and USDC in
//! through the Arbitrum bridge. Every user-signed action names mainnet's chain; every L1 one
//! recovers to the key that should have signed it.

use std::future::Future;

use df_hyperliquid_core::account::{Keeping, LocalKey, abstraction};
use df_hyperliquid_core::client::evm::tx::with_headroom;
use df_hyperliquid_core::client::evm::{BlockTag, Call, CallRequest, Fees, Rpc, U256, tx_hash};
use df_hyperliquid_core::client::{ApiError, Http, Request, Response, TransportError, info};
use df_hyperliquid_core::markets::spot::spot_meta_request;
use df_hyperliquid_core::transfer::{
    Amount, Ctx, Linked, PERP_DEX, SpotOut, SpotToken, bridge, erc20, pre_transfer_check,
};
use df_hyperliquid_core::utils::{Address, Json, hex};
use df_hyperliquid_core::{Client, Dec, Network, SendError};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::support::venue::Venue;
use crate::support::{clock, entry, key, vector};

/// Fresh on mainnet when `pre_transfer_check_mainnet.json` was recorded.
const FRESH: &str = "0x7a3c5e8d1f2b4c6a9e0d8f7b6a5c4d3e2f1a0b9c";
const HLP: &str = "0xdfc24b077bc1425ad1dea75bcb6f8158e10df303";
/// Hardhat's account #1, standing in for an agent: a published test key that holds nothing.
const AGENT_KEY: &str = "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d";

/// `spotMeta`'s `tokens`, as a transfer reads them.
#[derive(Deserialize)]
struct Tokens {
    tokens: Vec<SpotToken>,
}

fn address(text: &str) -> Address {
    text.parse().unwrap()
}

fn amount(typed: &str) -> Amount {
    Amount::parse(typed).unwrap()
}

fn dec(text: &str) -> Dec {
    text.parse().unwrap()
}

/// Mainnet's tokens on file, answered as `spotMeta`.
fn spot_meta() -> String {
    format!(r#"{{"tokens":{},"universe":[]}}"#, entry("transfer/spot_tokens.json", "mainnet"))
}

fn compact(reads: &[Json]) -> Vec<String> {
    reads.iter().map(Json::compact).collect()
}

/// HyperEVM mainnet's own answer to the first recorded call of `method`.
fn node(method: &str) -> String {
    let file: Value = serde_json::from_str(&vector("hyperevm_mainnet.json")).unwrap();
    let call = file["calls"].as_array().unwrap().iter().find(|call| call["method"] == method).unwrap();
    call["answer"].as_str().unwrap().to_owned()
}

fn result(value: &str) -> String {
    format!(r#"{{"jsonrpc":"2.0","id":1,"result":"{value}"}}"#)
}

#[tokio::test]
async fn usdc_to_a_fresh_account_checks_it_first_and_signs_usd_send_for_mainnet() {
    let venue = Venue::mainnet()
        .info_text("preTransferCheck", &entry("transfer/pre_transfer_check_mainnet.json", "fresh"))
        .exchange("default");
    let client = Client::new(venue, Network::Mainnet, clock, key());
    let me = key().address();

    let check = pre_transfer_check(&client, address(FRESH), me).await.unwrap();
    assert!(!check.is_sanctioned && !check.user_exists);
    assert_eq!(check.activation_fee(), Some(Dec::ONE));
    let ctx = Ctx { network: Network::Mainnet, account: me, keeping: Keeping::Apart, nonce: client.next_nonce() };
    let plan = ctx.usd_send(address(FRESH), amount("7.000")).unwrap();
    let accepted = client.send_user(&plan.action).await.unwrap();

    assert_eq!(
        compact(&client.http().reads()),
        [format!(r#"{{"type":"preTransferCheck","user":"{FRESH}","source":"{}"}}"#, me.to_hex())]
    );
    let actions = client.http().actions();
    let [sent] = actions.as_slice() else { panic!("{actions:?}") };
    assert_eq!(
        sent.action.compact(),
        format!(
            r#"{{"type":"usdSend","signatureChainId":"0xa4b1","hyperliquidChain":"Mainnet","destination":"{FRESH}","amount":"7","time":{}}}"#,
            ctx.nonce
        )
    );
    assert_eq!((sent.nonce, sent.chain.as_deref()), (ctx.nonce, Some("Mainnet")));
    assert_eq!(accepted.kind, "default");
    assert!(plan.effect.landed(dec("100"), dec("92")), "seven, and the fresh account's fee on top");
}

/// A unified account refuses `spotSend`, so its token leaves by `sendAsset` from spot to spot.
#[tokio::test]
async fn a_spot_token_leaves_a_unified_account_by_send_asset_and_a_refusal_comes_back_in_the_venue_s_words() {
    let venue = Venue::mainnet()
        .info_text("userAbstraction", r#""unifiedAccount""#)
        .info_text("spotMeta", &spot_meta())
        .info_text("preTransferCheck", &entry("transfer/pre_transfer_check_mainnet.json", "known"))
        .exchange("default")
        .exchange("refused_whole");
    let client = Client::new(venue, Network::Mainnet, clock, key());
    let me = key().address();

    let mode = abstraction(&client, me).await.unwrap();
    let keeping = Keeping::of(Some(&mode), None);
    assert_eq!(keeping, Keeping::Unified);
    let meta: Tokens = info(&client, spot_meta_request()).await.unwrap();
    let hype = meta.tokens.iter().find(|token| token.is_hype(Network::Mainnet)).unwrap();
    let ubtc = meta.tokens.iter().find(|token| token.name == "UBTC").unwrap();
    assert!(pre_transfer_check(&client, address(HLP), me).await.unwrap().activation_fee().is_none());
    let ctx = Ctx { network: Network::Mainnet, account: me, keeping, nonce: client.next_nonce() };
    let plan = ctx.send_token(address(HLP), hype, amount("0.50")).unwrap();
    assert!(matches!(plan.action, SpotOut::SendAsset(_)), "{:?}", plan.action);
    let accepted = client.send_user(&plan.action).await.unwrap();
    let again = Ctx { nonce: client.next_nonce(), ..ctx };
    let refused = client.send_user(&again.send_token(address(HLP), ubtc, amount("0.0001")).unwrap().action).await;

    assert_eq!(client.http().read_kinds(), ["userAbstraction", "spotMeta", "preTransferCheck"]);
    assert_eq!(client.http().reads()[0].compact(), format!(r#"{{"type":"userAbstraction","user":"{}"}}"#, me.to_hex()));
    let actions = client.http().actions();
    let [hype_out, ubtc_out] = actions.as_slice() else { panic!("{actions:?}") };
    assert_eq!(
        hype_out.action.compact(),
        format!(
            r#"{{"type":"sendAsset","signatureChainId":"0xa4b1","hyperliquidChain":"Mainnet","destination":"{HLP}","sourceDex":"spot","destinationDex":"spot","token":"HYPE:0x0d01dc56dcaaca66ad901c959b4011ec","amount":"0.5","fromSubAccount":"","nonce":{}}}"#,
            ctx.nonce
        )
    );
    assert_eq!(ubtc_out.action.get("token").and_then(Json::as_str), Some("UBTC:0x8f254b963e8468305d409b33aa137c67"));
    assert!(actions.iter().all(|sent| sent.chain.as_deref() == Some("Mainnet")));
    assert_eq!(accepted.kind, "default");
    assert_eq!(refused, Err(SendError::Api(ApiError::Refused("User or API Wallet 0xf6b2 does not exist.".to_owned()))));
}

/// The account's own key moves USDC from spot to perps; its agent moves it between two perp dexes
/// as an L1 action that carries its nonce and recovers to the agent.
#[tokio::test]
async fn usdc_moves_between_an_account_s_own_balances_by_its_key_or_its_agent() {
    let me = key().address();
    let usdc = SpotToken::from(Network::Mainnet.usdc());
    let own = Client::new(Venue::mainnet().exchange("default"), Network::Mainnet, clock, key());
    let ctx = Ctx { network: Network::Mainnet, account: me, keeping: Keeping::Apart, nonce: own.next_nonce() };
    let class = ctx.class_transfer(amount("12.50"), true).unwrap();
    own.send_user(&class.action).await.unwrap();

    let agent = LocalKey::from_hex(AGENT_KEY).unwrap();
    let venue = Venue::mainnet().exchange("default").signed_by(agent.address());
    let by_agent = Client::new(venue, Network::Mainnet, clock, agent);
    let ctx = Ctx { nonce: by_agent.next_nonce(), ..ctx };
    let between = ctx.agent_between(PERP_DEX, "xyz", &usdc, amount("25")).unwrap();
    by_agent.send_at(&between.action, between.nonce).await.unwrap();
    let pooled = Ctx { keeping: Keeping::Unified, ..ctx };
    assert!(pooled.class_transfer(amount("1"), true).is_err(), "one balance, nothing to move between");

    let actions = own.http().actions();
    let [class_sent] = actions.as_slice() else { panic!("{actions:?}") };
    assert_eq!(
        class_sent.action.compact(),
        format!(
            r#"{{"type":"usdClassTransfer","signatureChainId":"0xa4b1","hyperliquidChain":"Mainnet","amount":"12.5","toPerp":true,"nonce":{}}}"#,
            class.nonce
        )
    );
    let actions = by_agent.http().actions();
    let [between_sent] = actions.as_slice() else { panic!("{actions:?}") };
    assert_eq!(
        between_sent.action.compact(),
        format!(
            r#"{{"type":"agentSendAsset","destination":"{}","sourceDex":"","destinationDex":"xyz","token":"USDC:0x6d1e7cde53ba9467b783cb7c530ce054","amount":"25","fromSubAccount":"","nonce":{}}}"#,
            me.to_hex(),
            between.nonce
        )
    );
    assert_eq!((between_sent.nonce, between_sent.vault, between_sent.chain.as_deref()), (between.nonce, None, None));
    assert!(between.effect.landed(dec("100"), dec("75")), "the main dex's withdrawable, by exactly the amount");
}

/// KNTQ leaves a standard account by `spotSend` to its system address, and comes back by an
/// ERC-20 `transfer` to the same address on HyperEVM, signed as a type 2 transaction for chain 999.
#[tokio::test]
async fn a_token_crosses_to_hyperevm_by_its_system_address_and_comes_back_by_a_hyperevm_transfer() {
    let venue = Venue::mainnet().info_text("userAbstraction", r#""default""#).info_text("spotMeta", &spot_meta());
    let client = Client::new(venue.exchange("default"), Network::Mainnet, clock, key());
    let me = key().address();
    let keeping = Keeping::of(Some(&abstraction(&client, me).await.unwrap()), None);
    let meta: Tokens = info(&client, spot_meta_request()).await.unwrap();
    let kntq = meta.tokens.iter().find(|token| token.name == "KNTQ").unwrap();
    let ctx = Ctx { network: Network::Mainnet, account: me, keeping, nonce: client.next_nonce() };
    let out = ctx.to_evm(kntq, amount("2.5")).unwrap();
    client.send_user(&out.action).await.unwrap();

    let contract = kntq.evm_contract.unwrap();
    let link = Linked::erc20(u64::from(kntq.index), contract.address, kntq.wei_decimals, contract.extra_wei_decimals);
    let home = link.to_core(dec("2.5")).unwrap();
    let units = U256::from_u128(2_500_000_000_000_000_000);
    assert_eq!(
        (home.to, home.value, home.data.clone()),
        (Some(contract.address), U256::ZERO, erc20::transfer(link.system(), units))
    );
    let (nonce, gas) = (0x15_ffdf, with_headroom(0x7b2f));
    let fees = Fees::over_base(U256::from_u64(0x12d4_4a93), U256::ZERO);
    let expected = home.clone().transaction(999, nonce, gas, fees).sign(&key()).unwrap();
    let hyperevm = Rpc::hyperevm(
        Venue::mainnet()
            .rpc("eth_getTransactionCount", &node("eth_getTransactionCount"))
            .rpc("eth_estimateGas", &node("eth_estimateGas"))
            .rpc("eth_gasPrice", &node("eth_gasPrice"))
            .rpc("eth_maxPriorityFeePerGas", &node("eth_maxPriorityFeePerGas"))
            .rpc("eth_sendRawTransaction", &result(&tx_hash(&expected).to_hex())),
        Network::Mainnet,
    );
    let raw = send(&hyperevm, home, Network::Mainnet.evm_chain_id()).await;

    let actions = client.http().actions();
    let [sent] = actions.as_slice() else { panic!("{actions:?}") };
    assert_eq!(
        sent.action.compact(),
        format!(
            r#"{{"type":"spotSend","signatureChainId":"0xa4b1","hyperliquidChain":"Mainnet","destination":"0x200000000000000000000000000000000000007c","token":"KNTQ:0xbd31bd605c0a1b82c72aae3587f9061f","amount":"2.5","time":{}}}"#,
            ctx.nonce
        )
    );
    let calls = hyperevm.http().calls();
    let transfer = hex::encode_prefixed(&erc20::transfer(link.system(), units));
    assert_eq!(calls[1]["params"], json!([{"from": me.to_hex(), "to": contract.address.to_hex(), "data": transfer}]));
    assert_eq!(calls.last().unwrap()["params"], json!([hex::encode_prefixed(&raw)]));
    assert_eq!(raw, expected, "chain 999, the recorded nonce, the estimate with headroom, twice the base fee");
}

/// A deposit reads the owner's USDC and ETH on Arbitrum, then sends USDC `transfer` to the bridge
/// in millionths, as a type 2 transaction for chain 42161 to native USDC.
#[tokio::test]
async fn a_usdc_deposit_reads_arbitrum_then_sends_a_usdc_transfer_to_the_bridge() {
    let owner = key().address();
    let usdc = bridge::usdc(Network::Mainnet);
    let calldata = concat!(
        "0xa9059cbb",
        "0000000000000000000000002df1c51e09aecf9cacb7bc98cb1742757f163df7",
        "0000000000000000000000000000000000000000000000000000000001c9c380",
    );
    let deposit = Call { to: Some(usdc), value: U256::ZERO, data: hex::decode(calldata).unwrap() };
    let fees = Fees::over_base(U256::from_u64(10_000_000), U256::ZERO);
    let expected = deposit.transaction(42_161, 7, with_headroom(0xd1c4), fees).sign(&key()).unwrap();
    let node = Venue::mainnet()
        .rpc_call(&usdc.to_hex(), &result("0x000000000000000000000000000000000000000000000000000000000c84588f"))
        .rpc("eth_getBalance", &result("0x71afd498d0000"))
        .rpc("eth_getTransactionCount", &result("0x7"))
        .rpc("eth_estimateGas", &result("0xd1c4"))
        .rpc("eth_gasPrice", &result("0x989680"))
        .rpc("eth_maxPriorityFeePerGas", &result("0x0"))
        .rpc("eth_sendRawTransaction", &result(&tx_hash(&expected).to_hex()));
    let arbitrum = Rpc::new(Arbitrum(node), bridge::rpc_url(Network::Mainnet));

    let held = arbitrum.run(bridge::usdc_balance_request(Network::Mainnet, owner)).await.unwrap();
    let gas_money = arbitrum.run(bridge::eth_balance_request(owner)).await.unwrap();
    assert_eq!(bridge::usdc_amount(&held), Ok(dec("210.000015")));
    assert_eq!(bridge::eth_amount(gas_money), Ok(dec("0.002")));
    assert!(bridge::deposit_calldata(Network::Mainnet, dec("4.999999")).is_err(), "below the minimum is lost");
    let data = bridge::deposit_calldata(Network::Mainnet, dec("30")).unwrap();
    assert_eq!(data, calldata);
    let raw =
        send(&arbitrum, Call { to: Some(usdc), value: U256::ZERO, data: hex::decode(&data).unwrap() }, 42_161).await;

    let calls = arbitrum.http().0.calls();
    assert_eq!(
        calls[0]["params"],
        json!([{"to": usdc.to_hex(), "data": format!("0x70a08231000000000000000000000000{}", &owner.to_hex()[2..])}, "latest"])
    );
    assert_eq!(
        (calls[1]["method"].as_str(), &calls[1]["params"]),
        (Some("eth_getBalance"), &json!([owner.to_hex(), "latest"]))
    );
    assert_eq!(calls[3]["params"], json!([{"from": owner.to_hex(), "to": usdc.to_hex(), "data": calldata}]));
    assert_eq!(calls.last().unwrap()["params"], json!([hex::encode_prefixed(&raw)]));
    assert_eq!(raw, expected);
}

/// Asks the node what the transaction needs, signs it with [`key`], sends it once, and checks the
/// hash the node answers. The raw bytes sent.
async fn send<H: Http>(rpc: &Rpc<H>, call: Call, chain_id: u64) -> Vec<u8> {
    let from = key().address();
    let nonce = rpc.transaction_count(from, BlockTag::Latest).await.unwrap();
    let gas = rpc.estimate_gas(&CallRequest::of(Some(from), &call)).await.unwrap();
    let fees = Fees::over_base(rpc.gas_price().await.unwrap(), rpc.max_priority_fee().await.unwrap());
    let raw = call.transaction(chain_id, nonce, with_headroom(gas), fees).sign(&key()).unwrap();
    assert_eq!(rpc.send_raw_transaction(&raw).await.unwrap(), tx_hash(&raw));
    raw
}

/// Arbitrum's node: the venue's JSON-RPC answers, asked at the bridge's own RPC.
struct Arbitrum(Venue);

impl Http for Arbitrum {
    fn send(&self, mut request: Request) -> impl Future<Output = Result<Response, TransportError>> {
        assert_eq!(request.url, bridge::rpc_url(Network::Mainnet), "a deposit's reads go to Arbitrum");
        request.url = Network::Mainnet.evm_rpc_url().to_owned();
        self.0.send(request)
    }
}
