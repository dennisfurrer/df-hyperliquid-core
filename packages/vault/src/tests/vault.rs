use hl_core::Network;
use std::{
    cell::RefCell,
    future::{Future, ready},
    pin::pin,
    task::{Context, Poll, Waker},
};

use hl_core::{L1Action, LocalKey, TransportError, hex};

use super::*;
use crate::transfer::{HLP, HLP_LOCKUP_MS, VAULT_LOCKUP_MS, VaultTransfer, lockup_ms};

const KEY: &str = "0x0123456789012345678901234567890123456789012345678901234567890123";
const NONCE: u64 = 1_790_412_105_001;

/// Live, mainnet, 2026-10-06: HLP asked about one follower, trimmed to one
/// follower, one portfolio row and two children.
const HLP_DETAILS: &str = r#"{"name":"Hyperliquidity Provider (HLP)","vaultAddress":"0xdfc24b077bc1425ad1dea75bcb6f8158e10df303","leader":"0x677d831aef5328190852e24f13c46cac05f984e7","description":"This community-owned vault provides liqu","portfolio":[["day",{"accountValueHistory":[[1791216735605,"181550588.8688080013"]],"pnlHistory":[[1791216735605,"0.0"]],"vlm":"0.0"}]],"apr":0.037569521048262375,"followerState":{"user":"0x03e161499870b0a37549b16a5d33e0582fe1255e","vaultEquity":"604934.1646616657","pnl":"80799.3608496657","allTimePnl":"156824.7213636657","daysFollowing":655,"vaultEntryTime":1734650428028,"lockupUntil":1770680880422},"leaderFraction":0.002002618331064799,"leaderCommission":0.0,"followers":[{"user":"Leader","vaultEquity":"361964.4018696983","pnl":"273234.2946016983","allTimePnl":"273238.1857936983","daysFollowing":1252,"vaultEntryTime":1683243596849,"lockupUntil":1687702696528},{"user":"0x03e161499870b0a37549b16a5d33e0582fe1255e","vaultEquity":"604934.1646616657","pnl":"80799.3608496657","allTimePnl":"156824.7213636657","daysFollowing":655,"vaultEntryTime":1734650428028,"lockupUntil":1770680880422}],"maxDistributable":41085359.552966,"maxWithdrawable":604934.164661,"isClosed":false,"relationship":{"type":"parent","data":{"childAddresses":["0x010461c14e146ac35fe42271bdc1134ee31c703a","0x2e3d94f0562703b25c83308a05046ddaf9a8dd14"]}},"allowDeposits":true,"alwaysCloseOnWithdraw":false}"#;
/// Live, the same follower.
const EQUITIES: &str = r#"[{"vaultAddress":"0xdfc24b077bc1425ad1dea75bcb6f8158e10df303","equity":"604931.6453119999","lockedUntilTimestamp":1770680880422}]"#;
/// Live, HLP's leader.
const LEADING: &str =
    r#"[{"address":"0xdfc24b077bc1425ad1dea75bcb6f8158e10df303","name":"Hyperliquidity Provider (HLP)"}]"#;

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

/// The vector `@nktkas/hyperliquid` 0.33.2 made, as hip4-tui recorded it:
/// 10 USDC into HLP. Mainnet.
#[test]
fn a_vault_deposit_is_the_sdk_s_byte_for_byte() {
    let key = LocalKey::from_hex(KEY).unwrap();
    let action = VaultTransfer::deposit(HLP, dec("10")).unwrap();
    let json = r#"{"type":"vaultTransfer","vaultAddress":"0xdfc24b077bc1425ad1dea75bcb6f8158e10df303","isDeposit":true,"usd":10000000}"#;
    assert_eq!(action.wire(), json);
    assert_eq!(
        hex::encode_prefixed(&action.connection_id(NONCE, None)),
        "0x58a49fb2e400641875c0df3c98fd48f61210198012623c24f1ce8cc748c6e57a"
    );
    let signature = key.sign_digest(&action.digest(NONCE, None, Network::Mainnet)).unwrap();
    let (r, s) = (
        "0xd7b57bb7650ca10752a733ed62df2a5a8931e1c4f7290e0a143bcf96b22ad1fb",
        "0x63045ef3dfa4f14c26ac85dc03a76b4c332bbca837fb52ea4f9cf84bfcabe450",
    );
    assert_eq!((signature.r_hex().as_str(), signature.s_hex().as_str(), signature.v()), (r, s, 28));
    assert_eq!(
        action.body(NONCE, &signature, None),
        format!(
            r#"{{"action":{json},"nonce":{NONCE},"signature":{{"r":"{r}","s":"{s}","v":28}},"vaultAddress":null}}"#
        )
    );
}

#[test]
fn usd_goes_out_in_millionths_and_finer_is_refused() {
    let out = VaultTransfer::withdraw(HLP, dec("604934.164661")).unwrap();
    assert_eq!(out.usd(), 604_934_164_661);
    assert!(out.wire().contains(r#""isDeposit":false,"usd":604934164661"#));
    assert!(VaultTransfer::withdraw(HLP, dec("604931.6453119999")).is_err(), "an equity, uncut");
    assert!(VaultTransfer::deposit(HLP, Dec::ZERO).is_err());
    let cut = hl_core::cut_units(dec("604931.6453119999"), 6).unwrap();
    assert_eq!(VaultTransfer::from_micros(HLP, false, cut).usd(), 604_931_645_311);
}

#[test]
fn hlp_locks_a_deposit_four_days_and_any_other_vault_one() {
    assert_eq!(lockup_ms(HLP), HLP_LOCKUP_MS);
    assert_eq!(HLP_LOCKUP_MS, 4 * VAULT_LOCKUP_MS);
    assert_eq!(lockup_ms("0x010461c14e146ac35fe42271bdc1134ee31c703a".parse().unwrap()), 86_400_000);
}

#[test]
fn vault_details_read_with_their_floats_kept_as_written() {
    let hlp: VaultDetails = serde_json::from_str(HLP_DETAILS).unwrap();
    assert_eq!(hlp.vault_address, HLP);
    assert_eq!(hlp.apr.as_ref().map(Figure::as_str), Some("0.037569521048262375"));
    assert_eq!(hlp.apr.as_ref().and_then(Figure::to_dec), Some(dec("0.037569521048262375")));
    assert_eq!(hlp.max_withdrawable.as_ref().and_then(Figure::to_dec), Some(dec("604934.164661")));
    assert_eq!(hlp.leader_commission.as_ref().and_then(Figure::to_dec), Some(Dec::ZERO));
    let mine = hlp.follower_state.as_ref().unwrap();
    assert_eq!((mine.vault_equity, mine.lockup_until), (dec("604934.1646616657"), Some(1_770_680_880_422)));
    // the leader's own row names no address (as answered live on 2026-10-08)
    let users: Vec<Option<Address>> = hlp.followers.iter().map(|f| f.user).collect();
    assert_eq!(users, [None, Some(Address::parse_const("0x03e161499870b0a37549b16a5d33e0582fe1255e").unwrap())]);
    assert!(hlp.takes_deposits());
    assert_eq!(serde_json::from_str::<Option<VaultDetails>>("null").unwrap(), None, "no vault at that address");
}

#[test]
fn equities_and_leading_vaults_read() {
    let equities: Vec<VaultEquity> = serde_json::from_str(EQUITIES).unwrap();
    assert_eq!((equities[0].vault_address, equities[0].equity), (HLP, dec("604931.6453119999")));
    assert!(!equities[0].is_locked(1_791_305_840_205));
    assert!(serde_json::from_str::<Vec<VaultEquity>>("[]").unwrap().is_empty());
    let leading: Vec<LeadingVault> = serde_json::from_str(LEADING).unwrap();
    assert_eq!((leading[0].address, leading[0].name.as_str()), (HLP, "Hyperliquidity Provider (HLP)"));
}

struct Recorded {
    sent: RefCell<Vec<String>>,
}

impl Transport for Recorded {
    fn post(&self, _: &str, body: String) -> impl Future<Output = Result<String, TransportError>> {
        self.sent.borrow_mut().push(body);
        ready(Ok(HLP_DETAILS.to_owned()))
    }
}

fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return value;
        }
    }
}

/// Through `info`, as the app reads it: the floats survive the transport.
#[test]
fn a_details_read_names_the_vault_and_the_follower() {
    let transport = Recorded { sent: RefCell::new(Vec::new()) };
    let follower: Address = "0x03e161499870b0a37549b16a5d33e0582fe1255e".parse().unwrap();
    let hlp = block_on(vault_details(&transport, HLP, Some(follower))).unwrap().unwrap();
    assert_eq!(hlp.apr.as_ref().map(Figure::as_str), Some("0.037569521048262375"));
    assert_eq!(
        transport.sent.borrow()[0],
        r#"{"type":"vaultDetails","vaultAddress":"0xdfc24b077bc1425ad1dea75bcb6f8158e10df303","user":"0x03e161499870b0a37549b16a5d33e0582fe1255e"}"#
    );
    assert_eq!(
        vault_details_request(HLP, None),
        r#"{"type":"vaultDetails","vaultAddress":"0xdfc24b077bc1425ad1dea75bcb6f8158e10df303"}"#
    );
}
