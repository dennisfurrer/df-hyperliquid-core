//! What a HIP-3 dex may hold (`perpDexLimits`) and what it holds (`perpDexStatus`). Both always
//! name the dex, the main dex as `""`, which has no limits: `null`.

use crate::client::{ApiError, Transport, info, info_request};
use crate::markets::dex::Dex;
use crate::utils::{Dec, Json};
use serde::Deserialize;

/// `{"dex":<name>}`, `""` for the main dex.
fn named(kind: &str, dex: Dex<'_>) -> String {
    info_request(kind, &[("dex", Json::String(dex.name().unwrap_or_default().to_owned()))])
}

/// `{"type":"perpDexLimits","dex":<name>}`.
#[must_use]
pub fn perp_dex_limits_request(dex: Dex<'_>) -> String {
    named("perpDexLimits", dex)
}

/// `None` for the main dex and for a dex the venue does not know.
pub async fn perp_dex_limits(transport: &impl Transport, dex: Dex<'_>) -> Result<Option<DexLimits>, ApiError> {
    info(transport, perp_dex_limits_request(dex)).await
}

/// `{"type":"perpDexStatus","dex":<name>}`.
#[must_use]
pub fn perp_dex_status_request(dex: Dex<'_>) -> String {
    named("perpDexStatus", dex)
}

/// `None` where the venue answers `null`.
pub async fn perp_dex_status(transport: &impl Transport, dex: Dex<'_>) -> Result<Option<DexStatus>, ApiError> {
    info(transport, perp_dex_status_request(dex)).await
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DexLimits {
    /// Open interest across the dex, in its collateral.
    pub total_oi_cap: Dec,
    /// Open interest of one perp, in size.
    pub oi_sz_cap_per_perp: Dec,
    /// The most one transfer may move, in its collateral.
    pub max_transfer_ntl: Dec,
    /// Each coin's own open interest cap, in its collateral.
    #[serde(default)]
    pub coin_to_oi_cap: Vec<(String, Dec)>,
}

impl DexLimits {
    /// A coin's own open interest cap.
    #[must_use]
    pub fn coin_cap(&self, coin: &str) -> Option<Dec> {
        self.coin_to_oi_cap.iter().find(|(listed, _)| listed == coin).map(|(_, cap)| *cap)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DexStatus {
    /// What has come into the dex less what has left, in its collateral.
    pub total_net_deposit: Dec,
}
