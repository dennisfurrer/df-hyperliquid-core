//! `perpAnnotation` and `perpConciseAnnotations`: what a HIP-3 perp is, as its deployer describes
//! it for frontends: a category, a description, a display name and words to search on. Perps on
//! the main dex have none, and a read of one answers `null`.

use serde::Deserialize;

use crate::client::{ApiError, Transport, info, info_request};
use crate::utils::Json;

/// One perp's annotation, as `perpAnnotation` answers it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct PerpAnnotation {
    /// The deployer's category, kept as sent: `indices`, `stocks`, `preipo`.
    pub category: String,
    /// What the perp tracks, in the deployer's words.
    pub description: String,
    /// The name a frontend shows in place of the coin.
    #[serde(default)]
    pub display_name: Option<String>,
    /// Words a search should match the perp on.
    #[serde(default)]
    pub keywords: Vec<String>,
}

/// One perp's annotation in `perpConciseAnnotations`: [`PerpAnnotation`] without the description.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct ConciseAnnotation {
    /// The deployer's category, kept as sent.
    pub category: String,
    /// The name a frontend shows in place of the coin.
    #[serde(default)]
    pub display_name: Option<String>,
    /// Words a search should match the perp on.
    #[serde(default)]
    pub keywords: Vec<String>,
}

/// `{"type":"perpAnnotation","coin":..}`.
#[must_use]
pub fn perp_annotation_request(coin: &str) -> String {
    info_request("perpAnnotation", &[("coin", Json::String(coin.to_owned()))])
}

/// The annotation of the perp `coin` (`xyz:XYZ100`); `None` for a perp with none, a main dex perp,
/// or a coin the venue does not list.
pub async fn perp_annotation(transport: &impl Transport, coin: &str) -> Result<Option<PerpAnnotation>, ApiError> {
    info(transport, perp_annotation_request(coin)).await
}

/// `{"type":"perpConciseAnnotations"}`.
#[must_use]
pub fn perp_concise_annotations_request() -> String {
    info_request("perpConciseAnnotations", &[])
}

/// Every annotated perp's annotation by its coin: `("xyz:XYZ100", ..)`.
pub async fn perp_concise_annotations(
    transport: &impl Transport,
) -> Result<Vec<(String, ConciseAnnotation)>, ApiError> {
    info(transport, perp_concise_annotations_request()).await
}
