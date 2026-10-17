//! Connecting to Hyperliquid: the [`Client`] that reads, signs and sends over one network's
//! rate-limited `/info` and `/exchange` ([`http`]), the networks, HyperEVM ([`evm`]), and the
//! native and browser wires and websockets.

pub mod evm;
pub mod http;
#[cfg(feature = "native")]
pub mod native;
pub mod network;
#[cfg(feature = "web")]
pub mod web;

use crate::account::SignError;
#[cfg(feature = "sign")]
use crate::account::Signer;
use crate::client::http::Api;
#[cfg(feature = "sign")]
use crate::utils::sign::{Domain, L1Action, UserAction};
use crate::utils::{Address, NonceClock};
use std::sync::{Mutex, PoisonError};

pub use http::{
    Accepted, ApiError, ExchangeResponse, Http, Request, Response, Spent, Status, Transport, TransportError, exchange,
    info, info_request, user_request,
};
pub use network::{
    L1_CHAIN_ID, MAINNET_HYPE, MAINNET_USDC, Network, TESTNET_HYPE, TESTNET_USDC, Token, USDC_INDEX, UnknownNetwork,
};

/// Unix milliseconds. Injected: std has no clock in the browser or Workers,
/// and a test pins the time.
pub type Clock = fn() -> u64;

/// Why an action was not taken. An action whose answer never came
/// ([`TransportError::Unreachable`]) may have landed, so it is never resent.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SendError {
    #[error(transparent)]
    Sign(#[from] SignError),
    #[error(transparent)]
    Api(#[from] ApiError),
}

/// Reads for any module (`markets::perps::meta(&client)`) and, with a
/// [`Signer`], actions. `S` is `()` for a client that only reads.
pub struct Client<H, S = ()> {
    api: Api<H>,
    signer: S,
    vault: Option<Address>,
    clock: Clock,
    nonces: Mutex<NonceClock>,
}

impl<H: Http> Client<H> {
    /// A client that reads and never signs.
    pub fn reader(http: H, network: Network, clock: Clock) -> Self {
        Client::new(http, network, clock, ())
    }
}

impl<H: Http, S> Client<H, S> {
    /// One network, through `http`, signing with `signer`: the account's own
    /// key or wallet, or an agent for the actions an agent may sign.
    pub fn new(http: H, network: Network, clock: Clock, signer: S) -> Self {
        Client {
            api: Api::new(http, network, clock),
            signer,
            vault: None,
            clock,
            nonces: Mutex::new(NonceClock::new()),
        }
    }

    /// Acts for a subaccount or vault: its L1 actions name it as
    /// `vaultAddress`, and are still signed by this client's signer.
    #[must_use]
    pub fn acting_for(mut self, vault: Address) -> Self {
        self.vault = Some(vault);
        self
    }

    #[must_use]
    pub const fn network(&self) -> Network {
        self.api.network()
    }

    #[must_use]
    pub const fn http(&self) -> &H {
        self.api.http()
    }

    #[must_use]
    pub const fn signer(&self) -> &S {
        &self.signer
    }

    /// What went out over the last minute, and whether the gate is closed.
    pub fn spent(&self) -> Spent {
        self.api.spent()
    }

    /// The next nonce: the clock, or one past the last if the clock has not
    /// moved. Each client keeps its own, so give every signer one client.
    pub fn next_nonce(&self) -> u64 {
        self.nonces.lock().unwrap_or_else(PoisonError::into_inner).next((self.clock)())
    }
}

/// Sending needs the `sign` feature: each signature is recovered and must
/// match the signer before anything is posted, since a wallet can sign as a
/// different account than expected.
#[cfg(feature = "sign")]
impl<H: Http, S: Signer> Client<H, S> {
    /// Signs an L1 action at the next nonce and sends it once.
    pub async fn send(&self, action: &impl L1Action) -> Result<Accepted, SendError> {
        self.send_at(action, self.next_nonce()).await
    }

    /// Signs an L1 action at `nonce` and sends it once. The nonce must be new
    /// for this signer; [`Client::send`] takes care of that.
    pub async fn send_at(&self, action: &impl L1Action, nonce: u64) -> Result<Accepted, SendError> {
        let typed = action.typed(nonce, self.vault, self.network());
        let signature = self.signer.sign_typed(&Domain::EXCHANGE, &typed).await?;
        self.check(&typed.digest(&Domain::EXCHANGE), &signature)?;
        Ok(exchange(&self.api, action.body(nonce, &signature, self.vault)).await?)
    }

    /// Signs a user-signed action under this network's chain and sends it
    /// once. Its nonce is the one it was built with ([`Client::next_nonce`]
    /// gives one). Only the account's own key may sign these.
    pub async fn send_user(&self, action: &impl UserAction) -> Result<Accepted, SendError> {
        let network = self.network();
        let domain = Domain::user_signed(network);
        let typed = action.typed(network);
        let signature = self.signer.sign_typed(&domain, &typed).await?;
        self.check(&typed.digest(&domain), &signature)?;
        Ok(exchange(&self.api, action.body(&signature, network)).await?)
    }

    /// The signature must recover to the signer before it is posted.
    fn check(&self, digest: &[u8; 32], signature: &crate::account::Signature) -> Result<(), SignError> {
        crate::account::verify(digest, signature, self.signer.address())
    }
}

impl<H: Http, S> Transport for Client<H, S> {
    fn post(&self, path: &str, body: String) -> impl Future<Output = Result<String, TransportError>> {
        self.api.post(path, body)
    }
}

#[cfg(test)]
#[cfg(feature = "sign")]
#[path = "tests/client.rs"]
mod tests;
