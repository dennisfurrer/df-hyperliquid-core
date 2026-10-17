//! Recorded `/info` answers (`fixtures/`), live on mainnet 2026-10-08 unless named otherwise and
//! kept as the venue wrote them. Long lists are cut to their last few entries and zero spot rows
//! past the first few dropped; no value is edited.

use std::{
    cell::RefCell,
    future::{Future, ready},
    pin::pin,
    task::{Context, Poll, Waker},
};

use crate::client::{Transport, TransportError};
use crate::utils::{Address, Dec};

/// A unified account (`0x114dc2...`): spot, the main dex, `xyz`, and `portfolio`, all read within
/// five seconds of each other.
pub const SPOT_UNIFIED: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/account/spot_unified.json"));
pub const PERP_UNIFIED: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/account/perp_unified.json"));
pub const PERP_UNIFIED_XYZ: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/account/perp_unified_xyz.json"));
/// Every span cut to its last three points but `day`, kept whole.
pub const PORTFOLIO_UNIFIED: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/account/portfolio_unified.json"));
pub const SUBS_UNIFIED: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/account/subs_unified.json"));
pub const FEES_UNIFIED: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/account/fees_unified.json"));
pub const RATE_LIMIT_UNIFIED: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/account/rate_limit_unified.json"));

/// A standard (`default`) account (`0x677d83...`) with two vault deposits, read the same way. Its
/// `userVaultEquities` figures are taken here as plain numbers.
pub const SPOT_STANDARD: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/account/spot_standard.json"));
pub const PERP_STANDARD: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/account/perp_standard.json"));
pub const PERP_STANDARD_XYZ: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/account/perp_standard_xyz.json"));
pub const VAULTS_STANDARD: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/account/vaults_standard.json"));
pub const PORTFOLIO_STANDARD: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/account/portfolio_standard.json"));

/// The two accounts' tokens' USDC pairs, from `spotMeta`, and those pairs'
/// mids from `allMids`, read straight after them.
pub const PRICES: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/account/prices.json"));

/// A builder (`0x6967...a843`) on both networks.
pub const BUILDER_PERP: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/account/builder_perp.json"));
pub const BUILDER_PERP_TESTNET: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/account/builder_perp_testnet.json"));
pub const BUILDER_SPOT: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/account/spot_builder.json"));
pub const BUILDER_AGENTS: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/account/agents_builder.json"));
pub const BUILDER_AGENTS_TESTNET: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/account/agents_builder_testnet.json"));
pub const BUILDER_REFERRAL: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/account/referral_builder.json"));
/// Its 168 referral rows cut to two.
pub const BUILDER_REFERRAL_TESTNET: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/account/referral_builder_testnet.json"));
pub const RATE_LIMIT_BUILDER: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/account/rate_limit_builder.json"));
pub const RATE_LIMIT_BUILDER_TESTNET: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/account/rate_limit_builder_testnet.json"));

/// `userRole` of the unified account's agent, its subaccount, and HLP.
pub const ROLE_AGENT: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/account/role_agent.json"));
pub const ROLE_SUB: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/account/role_sub.json"));
pub const ROLE_VAULT: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/account/role_vault.json"));

/// Portfolio margin, live 2026-10-02: USDC net of a loan, HYPE all lent.
pub const SPOT_PORTFOLIO_MARGIN: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/account/spot_portfolio_margin.json"));

/// The unified account.
pub const UNIFIED: &str = "0x114dc2c90b61ef68b6ccb45d8e9902609cb08f9d";
/// Its subaccount.
pub const UNIFIED_SUB: &str = "0x1a16ca90bc961b6bce46f400be2afb0f9f71b93f";
/// The standard account.
pub const STANDARD: &str = "0x677d831aef5328190852e24f13c46cac05f984e7";
/// The builder.
pub const BUILDER: &str = "0x6967d7acd1ec5f210e9590666498e0cc5d13a843";

pub fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

pub fn address(text: &str) -> Address {
    text.parse().unwrap()
}

pub fn read<T: serde::de::DeserializeOwned>(text: &str) -> T {
    serde_json::from_str(text).unwrap()
}

/// Answers each read by its body, from a list of `(body, answer)`, and keeps
/// what was asked. Nothing leaves the process.
pub struct Recorded {
    pub answers: Vec<(String, &'static str)>,
    pub sent: RefCell<Vec<(String, String)>>,
}

impl Recorded {
    pub fn new(answers: &[(&str, &'static str)]) -> Recorded {
        Recorded {
            answers: answers.iter().map(|(body, answer)| ((*body).to_owned(), *answer)).collect(),
            sent: RefCell::new(Vec::new()),
        }
    }

    pub fn asked(&self) -> Vec<String> {
        self.sent.borrow().iter().map(|(_, body)| body.clone()).collect()
    }
}

impl Transport for Recorded {
    fn post(&self, path: &str, body: String) -> impl Future<Output = Result<String, TransportError>> {
        let answer = self.answers.iter().find(|(asked, _)| *asked == body).map(|(_, answer)| (*answer).to_owned());
        self.sent.borrow_mut().push((path.to_owned(), body.clone()));
        ready(answer.ok_or_else(|| TransportError::Unreachable(format!("no recording for {body}"))))
    }
}

pub fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return value;
        }
    }
}
