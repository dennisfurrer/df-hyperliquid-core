//! One account as last read: its mode, each perp dex it was read on, and spot, read at its own
//! address (a subaccount's for one). What it can draw on is [`crate::account::holdings::usable`],
//! what it is worth [`crate::account::holdings::worth`].

use crate::account::holdings::keeping::Keeping;
use crate::account::holdings::perps::{ClearinghouseState, clearinghouse_state};
use crate::account::holdings::spot::{SpotState, spot_state};
use crate::account::settings::mode::{Abstraction, abstraction};
use crate::client::{ApiError, Transport};
use crate::markets::Dex;
use crate::utils::{Address, Dec};

/// One account as a [`Standing`]: its mode, spot, and each of `dexes`, one read after another at
/// its own address (a subaccount's for one: [`crate::account::AccountRef::read_at`]). Weighs 22
/// and 2 a dex.
pub async fn standing(transport: &impl Transport, user: Address, dexes: &[Dex<'_>]) -> Result<Standing, ApiError> {
    let mode = abstraction(transport, user).await?;
    let spot = spot_state(transport, user).await?;
    let mut read = Vec::with_capacity(dexes.len());
    for dex in dexes {
        let state = clearinghouse_state(transport, user, *dex).await?;
        read.push(DexState { dex: dex.name().unwrap_or_default().to_owned(), state });
    }
    Ok(Standing { mode: Some(mode), dexes: read, spot: Some(spot), heard: None })
}

/// One perp dex's clearinghouse, by the dex's name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DexState {
    /// `""` for the main dex.
    pub dex: String,
    pub state: ClearinghouseState,
}

/// One account as last read: its mode, each dex it was read on, and spot.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Standing {
    /// `None` until a read or the socket says.
    pub mode: Option<Abstraction>,
    pub dexes: Vec<DexState>,
    pub spot: Option<SpotState>,
    /// When it was last told anything, in milliseconds.
    pub heard: Option<u64>,
}

impl Standing {
    #[must_use]
    pub fn dex(&self, dex: &str) -> Option<&ClearinghouseState> {
        self.dexes.iter().find(|state| state.dex == dex).map(|state| &state.state)
    }

    #[must_use]
    pub fn keeping(&self) -> Keeping {
        Keeping::of(self.mode.as_ref(), self.spot.as_ref())
    }

    /// Whether perp margin is drawn from spot, so a dex's value is already
    /// inside the spot balances.
    #[must_use]
    pub fn pools(&self) -> bool {
        self.keeping().pools()
    }

    /// A spot token less what orders hold of it, never below nothing.
    #[must_use]
    pub fn free(&self, token: u32) -> Dec {
        self.spot.as_ref().map_or(Dec::ZERO, |spot| spot.free(token))
    }
}
