//! `perpDexs`: the main dex, then every HIP-3 dex with its deployer, its oracle and fee settings
//! and who may deploy for it. A dex's meta is [`crate::markets::perps::meta()`].

use crate::client::{ApiError, Transport, info, info_request};
use crate::utils::{Address, Dec};
use serde::Deserialize;

/// `{"type":"perpDexs"}`.
#[must_use]
pub fn perp_dexs_request() -> String {
    info_request("perpDexs", &[])
}

/// Every perp dex, the main dex first as `None`.
pub async fn perp_dexs(transport: &impl Transport) -> Result<PerpDexs, ApiError> {
    info(transport, perp_dexs_request()).await
}

/// `perpDexs`: `null` for the main dex at 0, then each HIP-3 dex. A dex's place here is the `dex`
/// in its asset ids.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(transparent)]
pub struct PerpDexs(pub Vec<Option<PerpDex>>);

impl PerpDexs {
    /// Each HIP-3 dex with its place.
    pub fn dexes(&self) -> impl Iterator<Item = (u32, &PerpDex)> {
        self.0.iter().enumerate().filter_map(|(at, dex)| Some((u32::try_from(at).ok()?, dex.as_ref()?)))
    }

    /// A dex's place by its name: `xyz` is 1.
    #[must_use]
    pub fn index_of(&self, name: &str) -> Option<u32> {
        self.dexes().find(|(_, dex)| dex.name == name).map(|(at, _)| at)
    }

    /// A HIP-3 dex by its name; `None` for a name not listed.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&PerpDex> {
        self.dexes().find(|(_, dex)| dex.name == name).map(|(_, dex)| dex)
    }

    /// A dex's name by its place; `None` for the main dex at 0.
    #[must_use]
    pub fn name_of(&self, place: u32) -> Option<&str> {
        self.0.get(usize::try_from(place).ok()?)?.as_ref().map(|dex| dex.name.as_str())
    }
}

/// One HIP-3 dex as `perpDexs` lists it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerpDex {
    /// 2 to 4 lower case letters, the `dex:` of its coins.
    pub name: String,
    /// The dex's full name (`fullName`).
    pub full_name: String,
    /// The address that deployed the dex.
    #[serde(default)]
    pub deployer: Option<Address>,
    /// The address that updates the dex's oracle (`oracleUpdater`).
    #[serde(default)]
    pub oracle_updater: Option<Address>,
    /// The address the deployer's fee share goes to (`feeRecipient`).
    #[serde(default)]
    pub fee_recipient: Option<Address>,
    /// Each deploy action the deployer lets others send, and who.
    #[serde(default)]
    pub sub_deployers: Vec<(String, Vec<Address>)>,
    /// The open interest cap each coin streams toward, in the quote.
    #[serde(default)]
    pub asset_to_streaming_oi_cap: Vec<(String, Dec)>,
    /// What each listed coin's funding is scaled by, 0 to 10; unlisted is 1.
    #[serde(default)]
    pub asset_to_funding_multiplier: Vec<(String, Dec)>,
    /// An interest rate of its own in a coin's funding, -0.01 to 0.01.
    #[serde(default)]
    pub asset_to_funding_interest_rate: Vec<(String, Dec)>,
    /// The most a coin's hourly funding may be either way, 0 to 0.01 (`setFundingClamps`).
    #[serde(default)]
    pub asset_to_funding_clamp: Vec<(String, Dec)>,
}

impl PerpDex {
    /// The open interest cap `coin` streams toward, in the quote, where one is listed.
    #[must_use]
    pub fn streaming_oi_cap(&self, coin: &str) -> Option<Dec> {
        lookup(&self.asset_to_streaming_oi_cap, coin)
    }

    /// A coin's funding multiplier where the deployer set one.
    #[must_use]
    pub fn funding_multiplier(&self, coin: &str) -> Option<Dec> {
        lookup(&self.asset_to_funding_multiplier, coin)
    }

    /// A coin's funding interest rate where the deployer set one.
    #[must_use]
    pub fn funding_interest_rate(&self, coin: &str) -> Option<Dec> {
        lookup(&self.asset_to_funding_interest_rate, coin)
    }

    /// A coin's funding clamp where the deployer set one.
    #[must_use]
    pub fn funding_clamp(&self, coin: &str) -> Option<Dec> {
        lookup(&self.asset_to_funding_clamp, coin)
    }

    /// Who besides the deployer may send `action` (`registerAsset`, `setOracle`, ...) for this dex.
    #[must_use]
    pub fn sub_deployers_for(&self, action: &str) -> &[Address] {
        self.sub_deployers.iter().find(|(listed, _)| listed == action).map_or(&[], |(_, users)| users.as_slice())
    }
}

fn lookup(pairs: &[(String, Dec)], coin: &str) -> Option<Dec> {
    pairs.iter().find(|(listed, _)| listed == coin).map(|(_, value)| *value)
}
