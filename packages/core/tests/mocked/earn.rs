//! Earning through the [`Client`]: HYPE staked and delegated, a whole stake read back and a
//! delegation taken out of it, HLP joined and left against its details and the account's equity,
//! and lending against the reserves. Staking is user-signed for mainnet's chain; vault and lending
//! moves are L1 actions that recover to the account's key.

use df_hyperliquid_core::client::ApiError;
use df_hyperliquid_core::earn::lending::{self, Operation, all_reserve_states, borrowable, reserve_state, user_state};
use df_hyperliquid_core::earn::staking::{
    self, MAINNET_DELEGATION_LOCK_MS, delegator_summary, staking as whole_stake, validator_summaries,
};
use df_hyperliquid_core::earn::vaults::{
    self, HLP_LOCKUP_MS, MAINNET_HLP, leading_vaults, user_vault_equities, vault_details,
};
use df_hyperliquid_core::utils::{Address, Json};
use df_hyperliquid_core::{Client, Dec, Network, SendError};

use crate::support::venue::Venue;
use crate::support::{NOW, clock, entry, key};

/// HLP's leader.
const LEADER: &str = "0x677d831aef5328190852e24f13c46cac05f984e7";

fn dec(text: &str) -> Dec {
    text.parse().unwrap()
}

fn address(text: &str) -> Address {
    text.parse().unwrap()
}

fn compact(reads: &[Json]) -> Vec<String> {
    reads.iter().map(Json::compact).collect()
}

fn user_read(kind: &str, user: Address) -> String {
    format!(r#"{{"type":"{kind}","user":"{}"}}"#, user.to_hex())
}

fn stake(name: &str) -> String {
    entry("stake/mainnet.json", name)
}

#[tokio::test]
async fn hype_is_staked_then_delegated_to_an_active_validator_and_locked_a_day() {
    // The summary once the 100 HYPE deposit has landed, in the recorded shape.
    let landed = r#"{"delegated":"0.0","undelegated":"100.0","totalPendingWithdrawal":"0.0","nPendingWithdrawals":0}"#;
    let venue = Venue::mainnet()
        .info("validatorSummaries", "stake/validator_summaries_mainnet.json")
        .info_text("delegatorSummary", landed)
        .exchange("default")
        .exchange("default");
    let client = Client::new(venue, Network::Mainnet, clock, key());
    let me = key().address();

    let validators = validator_summaries(&client).await.unwrap();
    let chosen = validators.iter().find(|one| one.is_active && !one.is_jailed).unwrap();
    assert_eq!((chosen.name.as_str(), chosen.stake_hype()), ("ValiDAO", Some(dec("6499721.03992442"))));
    let ctx = staking::Ctx { network: Network::Mainnet, nonce: client.next_nonce(), now_ms: NOW };
    let deposit = ctx.stake(dec("100")).unwrap();
    client.send_user(&deposit.action).await.unwrap();
    let summary = delegator_summary(&client, me).await.unwrap();
    assert!(deposit.effect.landed(Dec::ZERO, summary.undelegated));
    let ctx = staking::Ctx { nonce: client.next_nonce(), ..ctx };
    assert!(ctx.delegate(chosen.validator, dec("100.00000001"), &summary).is_err(), "more than is staked");
    let delegate = ctx.delegate(chosen.validator, dec("100"), &summary).unwrap();
    client.send_user(&delegate.action).await.unwrap();

    assert_eq!(
        compact(&client.http().reads()),
        [r#"{"type":"validatorSummaries"}"#.to_owned(), user_read("delegatorSummary", me)]
    );
    let actions = client.http().actions();
    let [into_staking, to_validator] = actions.as_slice() else { panic!("{actions:?}") };
    assert_eq!(
        into_staking.action.compact(),
        format!(
            r#"{{"type":"cDeposit","signatureChainId":"0xa4b1","hyperliquidChain":"Mainnet","wei":10000000000,"nonce":{}}}"#,
            deposit.nonce
        )
    );
    assert_eq!(
        to_validator.action.compact(),
        format!(
            r#"{{"type":"tokenDelegate","signatureChainId":"0xa4b1","hyperliquidChain":"Mainnet","validator":"0x000000000056f99d36b6f2e0c51fd41496bbacb8","wei":10000000000,"isUndelegate":false,"nonce":{}}}"#,
            delegate.nonce
        )
    );
    assert!(actions.iter().all(|sent| sent.chain.as_deref() == Some("Mainnet")));
    assert_eq!(delegate.free_at, Some(NOW + MAINNET_DELEGATION_LOCK_MS));
}

/// The four reads of a HIP-3 deployer's stake, recorded on mainnet, read at [`NOW`]: thirty
/// daily rewards fall in the month, and nothing waits in the queue.
#[tokio::test]
async fn a_whole_stake_reads_from_four_answers_and_an_unlocked_delegation_is_taken_back() {
    let venue = Venue::mainnet()
        .info_text("delegatorSummary", &stake("delegatorSummary"))
        .info_text("delegations", &stake("delegations"))
        .info_text("delegatorRewards", &stake("delegatorRewards"))
        .info_text("delegatorHistory", &stake("delegatorHistory"))
        .exchange("refused_whole");
    let client = Client::new(venue, Network::Mainnet, clock, key());
    let me = key().address();

    let held = whole_stake(&client, Network::Mainnet, me, NOW).await.unwrap();
    assert_eq!(held.total(), Some(dec("501216.46877001")));
    assert_eq!(held.rewards, dec("909.24093562"));
    assert!(held.pending.is_empty());
    let infinite = address("0xa23b4556090260828ff3f939d2dbdd4f318b5f1f");
    let delegation = held.delegation(infinite).unwrap();
    assert!(!delegation.is_locked(NOW));
    let ctx = staking::Ctx { network: Network::Mainnet, nonce: client.next_nonce(), now_ms: NOW };
    let back = ctx.undelegate(infinite, delegation.amount, &held.delegations).unwrap();
    let refused = client.send_user(&back.action).await;

    assert_eq!(
        compact(&client.http().reads()),
        ["delegatorSummary", "delegations", "delegatorRewards", "delegatorHistory"].map(|kind| user_read(kind, me))
    );
    let actions = client.http().actions();
    let [sent] = actions.as_slice() else { panic!("{actions:?}") };
    assert_eq!(
        sent.action.compact(),
        format!(
            r#"{{"type":"tokenDelegate","signatureChainId":"0xa4b1","hyperliquidChain":"Mainnet","validator":"{infinite}","wei":27023501381309,"isUndelegate":true,"nonce":{}}}"#,
            back.nonce
        )
    );
    assert_eq!(refused, Err(SendError::Api(ApiError::Refused("User or API Wallet 0xf6b2 does not exist.".to_owned()))));
}

/// The vault HLP's leader leads, its details as mainnet recorded them asked about no one and then
/// about the account, and the leader's equities standing in for the account's: in HLP and one more
/// vault, both long unlocked.
#[tokio::test]
async fn hlp_is_found_by_its_leader_joined_against_its_details_and_left_with_the_whole_equity() {
    let venue = Venue::mainnet()
        .info_text("vaultDetails", &entry("vault/details.json", "mainnetHlp"))
        .info("userVaultEquities", "vault/user_vault_equities_mainnet.json")
        .info("leadingVaults", "vault/leading_vaults_mainnet.json")
        .exchange("default")
        .exchange("default")
        .signed_by(key().address());
    let client = Client::new(venue, Network::Mainnet, clock, key());
    let me = key().address();

    let led = leading_vaults(&client, address(LEADER)).await.unwrap();
    assert_eq!(led.iter().map(|vault| vault.address).collect::<Vec<_>>(), [MAINNET_HLP]);
    let listed = vault_details(&client, MAINNET_HLP, None).await.unwrap().unwrap();
    assert_eq!(listed.apr.and_then(|apr| apr.to_dec()), Some(dec("0.04165495390705244")));
    let hlp = vault_details(&client, MAINNET_HLP, Some(me)).await.unwrap().unwrap();
    assert!(hlp.takes_deposits());
    assert_eq!(hlp.withdrawable_now(NOW), Dec::ZERO, "not a follower yet");
    let ctx = vaults::Ctx { network: Network::Mainnet, nonce: client.next_nonce(), now_ms: NOW };
    let deposit = ctx.deposit(&hlp, dec("10")).unwrap();
    assert_eq!(deposit.free_at, Some(NOW + HLP_LOCKUP_MS));
    client.send(&deposit.action).await.unwrap();
    let equities = user_vault_equities(&client, me).await.unwrap();
    let share = equities.iter().find(|equity| equity.vault_address == MAINNET_HLP).unwrap();
    assert!(ctx.withdraw(share, dec("362286.490168")).is_err(), "more than the equity");
    let out = ctx.withdraw_all(share).unwrap();
    client.send(&out.action).await.unwrap();

    assert_eq!(
        compact(&client.http().reads()),
        [
            user_read("leadingVaults", address(LEADER)),
            format!(r#"{{"type":"vaultDetails","vaultAddress":"{MAINNET_HLP}"}}"#),
            format!(r#"{{"type":"vaultDetails","vaultAddress":"{MAINNET_HLP}","user":"{}"}}"#, me.to_hex()),
            user_read("userVaultEquities", me),
        ]
    );
    let actions = client.http().actions();
    let wires: Vec<String> = actions.iter().map(|sent| sent.action.compact()).collect();
    assert_eq!(
        wires,
        [
            format!(r#"{{"type":"vaultTransfer","vaultAddress":"{MAINNET_HLP}","isDeposit":true,"usd":10000000}}"#),
            format!(
                r#"{{"type":"vaultTransfer","vaultAddress":"{MAINNET_HLP}","isDeposit":false,"usd":362286490167}}"#
            ),
        ]
    );
    assert!(actions.iter().all(|sent| sent.signer == Some(me) && sent.vault.is_none()));
}

/// Mainnet's reserves and an HLP strategy's lending, both recorded: it supplies USDC and owes
/// nothing.
#[tokio::test]
async fn lending_supplies_hype_borrows_usdc_against_it_and_takes_its_usdc_back() {
    let venue = Venue::mainnet()
        .info_text("allBorrowLendReserveStates", &entry("lend/reserves.json", "mainnet"))
        .info("borrowLendUserState", "lend/user_state_mainnet.json")
        .info_text("borrowLendReserveState", &entry("lend/reserve.json", "mainnet"))
        .exchange("default")
        .exchange("default")
        .exchange("default")
        .signed_by(key().address());
    let client = Client::new(venue, Network::Mainnet, clock, key());
    let me = key().address();

    let reserves = all_reserve_states(&client).await.unwrap();
    let state = user_state(&client, me).await.unwrap();
    let ctx = lending::Ctx { network: Network::Mainnet, nonce: client.next_nonce() };
    client.send(&ctx.supply(150, dec("10"), &reserves).unwrap().action).await.unwrap();
    let most = borrowable(&[(150, dec("10"))], &reserves, &state);
    assert_eq!(most, dec("542.8085"), "ten HYPE at 83.509 and a 0.65 loan to value");
    client.send(&ctx.borrow(0, dec("100"), &reserves).unwrap().action).await.unwrap();
    let usdc = reserve_state(&client, 0).await.unwrap();
    assert_eq!(usdc.supply_rate_now(), usdc.supply_yearly_rate);
    let back = ctx.withdraw(0, None, &state).unwrap();
    assert_eq!(back.action.operation(), Operation::Withdraw);
    client.send(&back.action).await.unwrap();
    assert_eq!(ctx.repay(0, None, &state), Err(lending::Refused::Nothing(0)), "nothing owed");

    assert_eq!(
        compact(&client.http().reads()),
        [
            r#"{"type":"allBorrowLendReserveStates"}"#.to_owned(),
            user_read("borrowLendUserState", me),
            r#"{"type":"borrowLendReserveState","token":0}"#.to_owned(),
        ]
    );
    let actions = client.http().actions();
    let wires: Vec<String> = actions.iter().map(|sent| sent.action.compact()).collect();
    assert_eq!(
        wires,
        [
            r#"{"type":"borrowLend","operation":"supply","token":150,"amount":"10"}"#,
            r#"{"type":"borrowLend","operation":"borrow","token":0,"amount":"100"}"#,
            r#"{"type":"borrowLend","operation":"withdraw","token":0,"amount":null}"#,
        ]
    );
    assert!(actions.iter().all(|sent| sent.signer == Some(me)));
}

/// HYPE staked by an L1 action an agent could sign, and the `xyz` dex's backstop liquidator funded
/// and drawn on: both recover to the account's key.
#[tokio::test]
async fn hype_is_staked_by_an_l1_action_and_a_hip3_liquidator_is_funded_and_drawn_on() {
    use df_hyperliquid_core::account::Signers;
    use df_hyperliquid_core::earn::staking::CDeposit2;
    use df_hyperliquid_core::earn::vaults::Hip3LiquidatorTransfer;

    let venue = Venue::mainnet().signed_by(key().address()).exchange("default").exchange("default").exchange("default");
    let client = Client::new(venue, Network::Mainnet, clock, key());
    assert_eq!((CDeposit2::KEY, Hip3LiquidatorTransfer::KEY), (Signers::Either, Signers::Either));

    client.send(&CDeposit2::new(dec("2.5")).unwrap()).await.unwrap();
    client.send(&Hip3LiquidatorTransfer::deposit("xyz", dec("2000")).unwrap()).await.unwrap();
    client.send(&Hip3LiquidatorTransfer::withdraw("xyz", dec("1000")).unwrap()).await.unwrap();

    let sent: Vec<String> = client.http().actions().iter().map(|sent| sent.action.compact()).collect();
    assert_eq!(
        sent,
        [
            r#"{"type":"cDeposit2","wei":250000000}"#,
            r#"{"type":"hip3LiquidatorTransfer","dex":"xyz","ntl":2000000000,"isDeposit":true}"#,
            r#"{"type":"hip3LiquidatorTransfer","dex":"xyz","ntl":1000000000,"isDeposit":false}"#,
        ]
    );
}
