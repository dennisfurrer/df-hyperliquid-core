//! The [`Client`] itself: what its rate-limit gate lets out and holds back, how the venue's
//! refusals, an error status and a lost answer reach the caller, and how actions are signed for
//! an account, a vault and a network. HyperEVM goes through an [`Rpc`] instead: its JSON-RPC and
//! CoreWriter in [`hyperevm`], its read precompiles in [`precompiles`].
//!
//! [`Rpc`]: df_hyperliquid_core::client::evm::Rpc

mod hyperevm;
mod precompiles;

use std::cell::Cell;
use std::future::{Future, ready};

use df_hyperliquid_core::account::{LocalKey, SignError, Signature, Signer};
use df_hyperliquid_core::client::{ApiError, TransportError};
use df_hyperliquid_core::markets::{Dex, Precision, book, perps};
use df_hyperliquid_core::trade::{self, Asset, Cancel, CancelAnswer};
use df_hyperliquid_core::transfer::{Amount, UsdClassTransfer};
use df_hyperliquid_core::utils::sign::{Domain, TypedStruct};
use df_hyperliquid_core::utils::{Address, Json};
use df_hyperliquid_core::{Client, Network, SendError};

use crate::support::venue::Venue;
use crate::support::{NOW, clock, key};

/// The HLP vault, which its leader trades for.
const HLP: &str = "0xdfc24b077bc1425ad1dea75bcb6f8158e10df303";

thread_local! {
    static AT: Cell<u64> = const { Cell::new(NOW) };
}

/// A clock that starts at [`NOW`] and moves when the test says. Each test runs on its own
/// thread, so each has its own.
fn moving() -> u64 {
    AT.with(Cell::get)
}

fn later(ms: u64) {
    AT.with(|at| at.set(at.get() + ms));
}

fn cancel(oid: u64) -> Cancel {
    Cancel::one(Asset::perp(0).unwrap(), oid)
}

const fn limited(ms: u64) -> TransportError {
    TransportError::RateLimited { retry_after_ms: Some(ms) }
}

/// Each kind the last minute spent on: its name, how many, and their weight.
fn kinds<S>(client: &Client<Venue, S>) -> Vec<(String, u32, u32)> {
    client.spent().kinds.into_iter().map(|kind| (kind.name, kind.count, kind.weight)).collect()
}

#[tokio::test]
async fn reads_near_the_limit_are_held_back_without_going_out_and_actions_never_are() {
    let venue =
        Venue::mainnet().info("meta", "perps/meta_mainnet.json").exchange("cancel_taken").signed_by(key().address());
    let client = Client::new(venue, Network::Mainnet, moving, key());

    // 48 reads of 20 a second apart fill the 960 reads may spend of the minute's 1200.
    for _ in 0..48 {
        perps::meta(&client, Dex::Main).await.unwrap();
        later(1_000);
    }
    let held = perps::meta(&client, Dex::Main).await;
    assert_eq!(held.map(|_| ()), Err(ApiError::Transport(limited(12_000))), "until the first read rolls off");
    assert_eq!(client.http().reads().len(), 48, "the held read never went out");

    let accepted = client.send(&cancel(77)).await.unwrap();
    assert_eq!(trade::answer::cancels(&accepted, 1), Ok(vec![CancelAnswer::Cancelled]));
    let spent = client.spent();
    assert_eq!((spent.limit, spent.minute, spent.cooldown_ms), (1_200, 961, 0));
    assert_eq!(kinds(&client), [("meta".to_owned(), 48, 960), ("exchange".to_owned(), 1, 1)]);

    later(12_000);
    let still = perps::meta(&client, Dex::Main).await.map(|_| ());
    assert_eq!(still, Err(ApiError::Transport(limited(1_000))), "the action spent from the same minute");
    later(1_000);
    perps::meta(&client, Dex::Main).await.unwrap();
    let reads = client.http().reads();
    assert_eq!(reads.len(), 49);
    assert!(reads.iter().all(|read| read.compact() == r#"{"type":"meta"}"#));
}

#[tokio::test]
async fn a_429_closes_the_gate_for_reads_and_actions_until_it_ends() {
    let venue = Venue::mainnet()
        .info("meta", "perps/meta_mainnet.json")
        .info("l2Book", "trade/l2_book.json")
        .status("/info", 429, "null")
        .retry_after("/info", 10)
        .exchange("cancel_taken")
        .signed_by(key().address());
    let client = Client::new(venue, Network::Mainnet, moving, key());

    let refused = perps::meta(&client, Dex::Main).await.map(|_| ());
    assert_eq!(refused, Err(ApiError::Transport(limited(2_000))), "two seconds for the first strike");
    later(500);
    let book = book::l2_book(&client, "BTC", Precision::FULL).await.map(|_| ());
    assert_eq!(book, Err(ApiError::Transport(limited(1_500))), "another read");
    let action = client.send(&cancel(77)).await.map(|_| ());
    assert_eq!(action, Err(SendError::Api(ApiError::Transport(limited(1_500)))), "an action too");
    assert_eq!((client.http().reads().len(), client.http().actions().len()), (1, 0), "only the first went out");
    let spent = client.spent();
    assert_eq!((spent.refused, spent.cooldown_ms), (1, 1_500));

    later(1_500);
    client.send(&cancel(77)).await.unwrap();
    let longer = perps::meta(&client, Dex::Main).await.map(|_| ());
    assert_eq!(longer, Err(ApiError::Transport(limited(10_000))), "a longer Retry-After wins");
    assert_eq!(client.spent().cooldown_ms, 10_000);
    assert_eq!(client.http().read_kinds(), ["meta", "meta"]);
}

#[tokio::test]
async fn an_error_status_is_the_venue_s_answer_counted_and_never_retried() {
    let venue = Venue::mainnet()
        .info("meta", "perps/meta_mainnet.json")
        .status("/info", 500, "busy")
        .status("/exchange", 502, "bad gateway")
        .signed_by(key().address());
    let client = Client::new(venue, Network::Mainnet, clock, key());

    let read = perps::meta(&client, Dex::Main).await.map(|_| ());
    let busy = TransportError::Status { status: 500, body: "busy".to_owned() };
    assert_eq!(read, Err(ApiError::Transport(busy)));
    let action = client.send(&cancel(77)).await.map(|_| ());
    let gateway = TransportError::Status { status: 502, body: "bad gateway".to_owned() };
    assert_eq!(action, Err(SendError::Api(ApiError::Transport(gateway))));
    assert_eq!((client.http().reads().len(), client.http().actions().len()), (1, 1), "each went out once");
    let spent = client.spent();
    assert_eq!((spent.minute, spent.cooldown_ms), (21, 0), "counted, and only a 429 closes the gate");
    perps::meta(&client, Dex::Main).await.unwrap();
}

#[tokio::test]
async fn an_action_whose_answer_never_came_is_not_sent_again() {
    let venue = Venue::mainnet().unreachable("/exchange").signed_by(key().address());
    let client = Client::new(venue, Network::Mainnet, clock, key());

    let lost = client.send(&cancel(77)).await.unwrap_err();
    assert!(matches!(lost, SendError::Api(ApiError::Transport(TransportError::Unreachable(_)))), "{lost:?}");
    let actions = client.http().actions();
    let [taken] = actions.as_slice() else { panic!("{actions:?}") };
    assert_eq!(taken.action.compact(), r#"{"type":"cancel","cancels":[{"a":0,"o":77}]}"#, "it may have landed");
    let spent = client.spent();
    assert_eq!((spent.minute, spent.refused, spent.cooldown_ms), (0, 0, 0), "neither counted nor struck");
}

#[tokio::test]
async fn the_venue_s_answer_reaches_the_caller_taken_refused_in_part_or_refused_whole() {
    let venue = Venue::mainnet()
        .exchange("cancel_taken")
        .exchange("cancel_refused")
        .exchange("refused_whole")
        .signed_by(key().address());
    let client = Client::new(venue, Network::Mainnet, clock, key());

    let taken = client.send(&cancel(1)).await.unwrap();
    assert_eq!((taken.kind.as_str(), taken.errors().count()), ("cancel", 0));
    let in_part = client.send(&cancel(2)).await.unwrap();
    assert_eq!(in_part.errors().collect::<Vec<_>>(), ["Order was never placed, already canceled, or filled."]);
    assert!(matches!(trade::answer::cancels(&in_part, 1).as_deref(), Ok([CancelAnswer::Refused(_)])));
    let whole = client.send(&cancel(3)).await.unwrap_err();
    assert_eq!(whole, SendError::Api(ApiError::Refused("User or API Wallet 0xf6b2 does not exist.".to_owned())));
    assert_eq!(whole.to_string(), "User or API Wallet 0xf6b2 does not exist.", "in the venue's own words");
    assert_eq!(client.http().actions().len(), 3, "each sent once");
}

#[tokio::test]
async fn a_vault_s_actions_name_it_are_signed_by_its_trader_and_take_rising_nonces() {
    let vault: Address = HLP.parse().unwrap();
    let venue = Venue::mainnet()
        .exchange("cancel_taken")
        .exchange("cancel_taken")
        .exchange("cancel_taken")
        .signed_by(key().address());
    let client = Client::new(venue, Network::Mainnet, clock, key()).acting_for(vault);

    for oid in [1, 2] {
        client.send(&cancel(oid)).await.unwrap();
    }
    client.send_at(&cancel(3), client.next_nonce()).await.unwrap();

    let actions = client.http().actions();
    let nonces: Vec<u64> = actions.iter().map(|action| action.nonce).collect();
    assert_eq!(nonces, [NOW, NOW + 1, NOW + 2], "the clock stands still; each nonce is one past the last");
    for action in &actions {
        assert_eq!((action.vault, action.signer), (Some(vault), Some(key().address())), "{}", action.nonce);
    }
    assert_eq!(actions[2].action.compact(), r#"{"type":"cancel","cancels":[{"a":0,"o":3}]}"#);

    let own = Client::new(Venue::mainnet().exchange("cancel_taken"), Network::Mainnet, clock, key());
    own.send(&cancel(4)).await.unwrap();
    assert_eq!(own.http().actions()[0].vault, None, "for the account itself, an explicit null");
}

#[tokio::test]
async fn a_user_signed_transfer_is_signed_under_the_client_s_network() {
    for network in Network::ALL {
        let client = Client::new(Venue::on(network).exchange("default"), network, clock, key());
        let transfer = UsdClassTransfer::new(Amount::parse("12.5").unwrap(), true, client.next_nonce());
        let accepted = client.send_user(&transfer).await.unwrap();
        assert_eq!((accepted.kind.as_str(), accepted.statuses.len(), accepted.data), ("default", 0, None));

        let actions = client.http().actions();
        let [sent] = actions.as_slice() else { panic!("{actions:?}") };
        assert_eq!(sent.chain.as_deref(), Some(network.hyperliquid_chain()), "{network}");
        assert_eq!(
            sent.action.get("signatureChainId").and_then(Json::as_str),
            Some(network.signature_chain_id_hex()),
            "{network}"
        );
        assert_eq!(sent.action.get("amount").and_then(Json::as_str), Some("12.5"));
        assert_eq!(sent.nonce, NOW);
    }
}

/// Claims one address and signs with another key, like a wallet that switched accounts.
struct Impostor {
    key: LocalKey,
    claims: Address,
}

impl Signer for Impostor {
    fn address(&self) -> Address {
        self.claims
    }

    fn sign_typed(
        &self,
        domain: &Domain,
        data: &TypedStruct<'_>,
    ) -> impl Future<Output = Result<Signature, SignError>> {
        ready(self.key.sign_digest(&data.digest(domain)))
    }
}

#[tokio::test]
async fn a_signature_from_anyone_but_the_signer_is_never_posted() {
    let impostor = Impostor { key: LocalKey::from_bytes(&[7; 32]).unwrap(), claims: key().address() };
    let client = Client::new(Venue::mainnet(), Network::Mainnet, clock, impostor);

    let action = client.send(&cancel(77)).await.unwrap_err();
    assert!(matches!(action, SendError::Sign(SignError::WrongSigner { .. })), "{action:?}");
    let transfer = UsdClassTransfer::new(Amount::parse("1").unwrap(), true, client.next_nonce());
    let user_signed = client.send_user(&transfer).await.unwrap_err();
    assert!(matches!(user_signed, SendError::Sign(SignError::WrongSigner { .. })), "{user_signed:?}");
    assert!(client.http().sent().is_empty(), "nothing went out");
}
