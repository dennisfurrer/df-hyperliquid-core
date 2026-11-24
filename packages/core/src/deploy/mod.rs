//! Deploying on Hyperliquid: spot tokens and their auction, HIP-3 perp dexes, HIP-4 outcome markets
//! and aligned quote assets.
pub mod annotation;
pub mod auction;
pub mod dexes;
pub mod error;
pub mod name;
pub mod outcomes;
pub mod perp;
pub mod spot;
pub mod tokens;

pub use annotation::Annotation;
pub use auction::GasAuction;
pub use dexes::{perp_deploy_auction, perp_deploy_auction_request};
pub use error::BadDeploy;
pub use name::DexName;
pub use perp::PerpDeploy;
pub use spot::{FinalizeEvmContract, SpotDeploy};
pub use tokens::{
    DeployingToken, SpotDeployState, TokenSpec, spot_deploy_state, spot_deploy_state_request, spot_pair_deploy_auction,
    spot_pair_deploy_auction_request,
};
