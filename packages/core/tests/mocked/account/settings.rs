//! How an account is set up: a subaccount opened, read where it lives and funded; the account moved
//! to unified by its own wallet and held there; a referral code checked and traded under, and what
//! the account may still send. The subaccount the venue names is the unified account's, read as it
//! stands.

use df_hyperliquid_core::account::settings::{INITIAL_REQUESTS, OPEN_ORDERS, open_order_cap};
use df_hyperliquid_core::account::{
    self, Abstraction, AbstractionMode, AccountRef, ActionKind, CreateSubAccount, Keeping, Key, SetReferrer,
    SubAccountSpotTransfer, SubAccountTransfer, SubRefusal, UserSetAbstraction, Why, abstraction, may_sign, rate_limit,
    referral, sub_accounts,
};
use df_hyperliquid_core::markets::Dex;
use df_hyperliquid_core::utils::AssetKind;
use df_hyperliquid_core::{Address, Client, Network};

use super::{BUILDER, address, bodies, dec};
use crate::support::venue::Venue;
use crate::support::{NOW, clock, fixture, key};

/// The unified account's subaccount.
const SUB: &str = "0x1a16ca90bc961b6bce46f400be2afb0f9f71b93f";
/// What `createSubAccount` answers, as `@nktkas/hyperliquid` types it: the new subaccount's address.
const CREATED: &str =
    r#"{"status":"ok","response":{"type":"createSubAccount","data":"0x1a16ca90bc961b6bce46f400be2afb0f9f71b93f"}}"#;
const HYPE: &str = "HYPE:0x0d01dc56dcaaca66ad901c959b4011ec";

fn read(kind: &str, user: Address) -> String {
    format!(r#"{{"type":"{kind}","user":"{}"}}"#, user.to_hex())
}

#[tokio::test]
async fn a_master_opens_a_subaccount_reads_it_where_it_lives_and_funds_it() {
    let owner = key().address();
    let venue = Venue::mainnet()
        .info("subAccounts", "account/subs_none_mainnet.json")
        .info("userAbstraction", "account/mode_sub_mainnet.json")
        .info("spotClearinghouseState", "account/spot_sub_mainnet.json")
        .info("clearinghouseState", "account/perp_sub_mainnet.json")
        .exchange_text(CREATED)
        .exchange("default")
        .exchange("default")
        .signed_by(owner);
    let client = Client::new(venue, Network::Mainnet, clock, key());

    assert_eq!(sub_accounts(&client, owner).await.unwrap(), [], "the venue answers null for none");
    let created = client.send(&CreateSubAccount::new("desk two")).await.unwrap();
    let sub: Address = created.data.as_ref().and_then(|data| data.as_str()).unwrap().parse().unwrap();
    let desk = AccountRef::sub(owner, sub);
    let standing = account::standing(&client, desk.read_at(), &[Dex::Main]).await.unwrap();
    client.send(&SubAccountTransfer::deposit(sub, dec("5")).unwrap()).await.unwrap();
    client.send(&SubAccountSpotTransfer::new(sub, true, HYPE, dec("2.0")).unwrap()).await.unwrap();

    let at_sub = address(SUB);
    assert_eq!(sub, at_sub);
    assert_eq!(
        bodies(client.http()),
        [
            read("subAccounts", owner),
            read("userAbstraction", at_sub),
            read("spotClearinghouseState", at_sub),
            read("clearinghouseState", at_sub),
        ]
    );
    assert_eq!(standing.keeping(), Keeping::Unified, "its own mode, read at its own address");
    assert_eq!(standing.spot.as_ref().map(|spot| spot.held().count()), Some(0));
    assert!(!standing.dex("").unwrap().holds_anything());
    let sent: Vec<String> = client.http().actions().iter().map(|sent| sent.action.compact()).collect();
    assert_eq!(
        sent,
        [
            r#"{"type":"createSubAccount","name":"desk two"}"#.to_owned(),
            format!(r#"{{"type":"subAccountTransfer","subAccountUser":"{SUB}","isDeposit":true,"usd":5000000}}"#),
            format!(
                r#"{{"type":"subAccountSpotTransfer","subAccountUser":"{SUB}","isDeposit":true,"token":"{HYPE}","amount":"2"}}"#
            ),
        ]
    );
    assert!(client.http().actions().iter().all(|sent| sent.signer == Some(owner) && sent.vault.is_none()));
    let wrong = may_sign(ActionKind::SubAccountTransfer, Key::Agent).unwrap_err();
    assert_eq!(
        (wrong.kind, wrong.why),
        (ActionKind::SubAccountTransfer, Why::WrongAccount),
        "the master's own key moves money"
    );
    assert_eq!(desk.refuses(ActionKind::Order, Some(AssetKind::Spot)), Some(SubRefusal::Spot));
    assert_eq!(desk.refuses(ActionKind::Order, Some(AssetKind::Perp)), None, "its perps trade as the vault");
    assert_eq!(desk.vault_address(), Some(sub));
}

#[tokio::test]
async fn an_account_moves_to_unified_by_its_own_wallet_and_is_held_there() {
    let owner = key().address();
    let venue = Venue::mainnet().info("userAbstraction", "account/mode_fresh_mainnet.json").exchange("default");
    let client = Client::new(venue, Network::Mainnet, clock, key());

    let mode = abstraction(&client, owner).await.unwrap();
    assert_eq!(mode, Abstraction::Default, "never set");
    assert!(!AbstractionMode::UnifiedAccount.is_refused_from(&mode));
    let unified = UserSetAbstraction::new(owner, AbstractionMode::UnifiedAccount, client.next_nonce());
    client.send_user(&unified).await.unwrap();

    assert_eq!(bodies(client.http()), [read("userAbstraction", owner)]);
    let [set] = client.http().actions().try_into().unwrap();
    assert_eq!(
        set.action.compact(),
        format!(
            r#"{{"type":"userSetAbstraction","signatureChainId":"0xa4b1","hyperliquidChain":"Mainnet","user":"{}","abstraction":"unifiedAccount","nonce":{NOW}}}"#,
            owner.to_hex()
        )
    );
    assert_eq!(set.chain.as_deref(), Some("Mainnet"));
    assert!(AbstractionMode::Disabled.is_refused_from(&Abstraction::UnifiedAccount), "never back to standard");
}

#[tokio::test]
async fn an_account_trades_under_a_code_it_does_not_own_and_knows_what_it_may_send() {
    let owner = key().address();
    let venue = Venue::mainnet()
        .info_where("referral", &[("user", BUILDER)], &fixture("account/referral_builder.json"))
        .info("referral", "account/referral_fresh_mainnet.json")
        .info("userRateLimit", "account/rate_limit_fresh_mainnet.json")
        .exchange("default")
        .signed_by(owner);
    let client = Client::new(venue, Network::Mainnet, clock, key());

    let code_owner = referral(&client, address(BUILDER)).await.unwrap();
    let mine = referral(&client, owner).await.unwrap();
    assert_eq!(code_owner.own_code(), Some("DFU"));
    assert_eq!((mine.own_code(), mine.referred_code()), (None, None));
    assert!(!mine.owns("dfu"), "codes are compared without case");
    client.send(&SetReferrer::new("DFU").unwrap()).await.unwrap();
    let limit = rate_limit(&client, owner).await.unwrap();

    assert_eq!(
        bodies(client.http()),
        [read("referral", address(BUILDER)), read("referral", owner), read("userRateLimit", owner)]
    );
    assert_eq!(mine.referrer_state.stage, "needToTrade");
    assert_eq!(mine.referrer_state.data.and_then(|data| data.required), Some(dec("10000")), "to make a code");
    let [set] = client.http().actions().try_into().unwrap();
    assert_eq!((set.action.compact(), set.signer), (r#"{"type":"setReferrer","code":"DFU"}"#.to_owned(), Some(owner)));
    assert_eq!((limit.cap, limit.remaining(), limit.is_throttled()), (INITIAL_REQUESTS, INITIAL_REQUESTS, false));
    assert_eq!(open_order_cap(limit.volume), OPEN_ORDERS);
}

/// Referral rewards claimed, and a thousand requests reserved for the account and for another: L1
/// actions that recover to the account's key.
#[tokio::test]
async fn referral_rewards_are_claimed_and_requests_reserved_for_the_account_and_another() {
    use df_hyperliquid_core::account::{ClaimRewards, ReserveRequestWeight};

    let venue = Venue::mainnet().signed_by(key().address()).exchange("default").exchange("default").exchange("default");
    let client = Client::new(venue, Network::Mainnet, clock, key());
    let reserve = ReserveRequestWeight::new(1_000).unwrap();
    assert_eq!(reserve.cost(), Some(dec("0.5")), "half a USDC for a thousand");

    client.send(&ClaimRewards).await.unwrap();
    client.send(&reserve).await.unwrap();
    client.send(&reserve.for_user(address(SUB))).await.unwrap();

    let sent: Vec<String> = client.http().actions().iter().map(|sent| sent.action.compact()).collect();
    assert_eq!(
        sent,
        [
            r#"{"type":"claimRewards"}"#.to_owned(),
            r#"{"type":"reserveRequestWeight","weight":1000}"#.to_owned(),
            format!(r#"{{"type":"reserveRequestWeight","weight":1000,"destination":"{SUB}"}}"#),
        ]
    );
}
