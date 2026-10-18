//! Staking, vault and lending reads from mainnet, each held against its recording: validators, a
//! HIP-3 deployer's stake, HLP and its leader, the reserves and an HLP strategy's lending.

use df_hyperliquid_core::Dec;
use df_hyperliquid_core::earn::lending::{all_reserve_states, reserve_state, user_state};
use df_hyperliquid_core::earn::staking::{
    delegations, delegator_history, delegator_rewards, delegator_summary, validator_summaries,
};
use df_hyperliquid_core::earn::vaults::{MAINNET_HLP, leading_vaults, user_vault_equities, vault_details};
use df_hyperliquid_core::utils::Address;

use crate::tap::{reader, same_shape};

/// The HIP-3 deployer whose stake `stake/mainnet.json` recorded.
const STAKER: &str = "0x88806a71d74ad0a510b350545c9ae490912f0888";
const LEADER: &str = "0x677d831aef5328190852e24f13c46cac05f984e7";
/// An HLP strategy that supplies USDC.
const SUPPLIER: &str = "0x469f690213c467c39a23efacfd2816896009d7d8";

fn address(text: &str) -> Address {
    text.parse().unwrap()
}

#[tokio::test]
#[ignore = "mainnet"]
async fn validators_list_their_stake_and_weekly_stats() {
    let client = reader();
    let validators = validator_summaries(&client).await.unwrap();
    let active = validators.iter().find(|one| one.is_active && !one.is_jailed).unwrap();
    assert!(active.stake_hype().is_some_and(|stake| stake > Dec::ZERO) && active.stats("week").is_some());
    same_shape(&client, "stake/validator_summaries_mainnet.json");
}

#[tokio::test]
#[ignore = "mainnet"]
async fn a_deployer_s_stake_reads_its_summary_delegations_rewards_and_history() {
    let client = reader();
    let staker = address(STAKER);
    let summary = delegator_summary(&client, staker).await.unwrap();
    same_shape(&client, "stake/mainnet.json#delegatorSummary");
    let delegated = delegations(&client, staker).await.unwrap();
    same_shape(&client, "stake/mainnet.json#delegations");
    let sum = delegated.iter().try_fold(Dec::ZERO, |sum, one| sum.checked_add(one.amount));
    assert_eq!(sum, Some(summary.delegated), "the delegations add up to the summary");
    assert!(!delegator_rewards(&client, staker).await.unwrap().is_empty());
    same_shape(&client, "stake/mainnet.json#delegatorRewards");
    assert!(!delegator_history(&client, staker).await.unwrap().is_empty());
    same_shape(&client, "stake/mainnet.json#delegatorHistory");
}

#[tokio::test]
#[ignore = "mainnet"]
async fn hlp_reads_its_details_and_its_leader_its_equities_and_the_vault_it_leads() {
    let client = reader();
    let leader = address(LEADER);
    let hlp = vault_details(&client, MAINNET_HLP, None).await.unwrap().unwrap();
    assert_eq!((hlp.leader, hlp.takes_deposits()), (leader, true));
    same_shape(&client, "vault/details.json#mainnetHlp");
    assert!(vault_details(&client, MAINNET_HLP, Some(leader)).await.unwrap().is_some());
    same_shape(&client, "vault/details.json#mainnetHlp");
    let equities = user_vault_equities(&client, leader).await.unwrap();
    assert!(equities.iter().any(|equity| equity.vault_address == MAINNET_HLP));
    same_shape(&client, "vault/user_vault_equities_mainnet.json");
    let led = leading_vaults(&client, leader).await.unwrap();
    assert!(led.iter().any(|vault| vault.address == MAINNET_HLP));
    same_shape(&client, "vault/leading_vaults_mainnet.json");
}

#[tokio::test]
#[ignore = "mainnet"]
async fn the_reserves_follow_their_rate_rules_and_a_supplier_s_position_reads() {
    let client = reader();
    let reserves = all_reserve_states(&client).await.unwrap();
    same_shape(&client, "lend/reserves.json#mainnet");
    for (token, reserve) in &reserves {
        assert_eq!(reserve.supply_rate_now(), reserve.supply_yearly_rate, "{token}");
    }
    let usdc = reserve_state(&client, 0).await.unwrap();
    same_shape(&client, "lend/reserve.json#mainnet");
    assert_eq!(usdc.oracle_px, Dec::ONE);
    let supplier = user_state(&client, address(SUPPLIER)).await.unwrap();
    same_shape(&client, "lend/user_state_mainnet.json");
    assert!(supplier.supplied(0) > Dec::ZERO);
}
