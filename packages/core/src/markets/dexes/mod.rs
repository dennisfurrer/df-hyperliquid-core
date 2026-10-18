//! HIP-3 builder-deployed perp dexes: the dex list, every dex lined up with its meta, and each
//! dex's limits and status. A dex's meta and contexts are [`crate::markets::perps`]'s.

pub mod dex;
pub mod limits;
pub mod listing;

pub use dex::{PerpDex, PerpDexs, perp_dexs, perp_dexs_request};
pub use limits::{
    DexLimits, DexStatus, perp_dex_limits, perp_dex_limits_request, perp_dex_status, perp_dex_status_request,
};
pub use listing::{Dexes, Misaligned, all_perp_metas, all_perp_metas_request, dexes};

#[cfg(test)]
#[path = "tests/dexes.rs"]
mod dexes_tests;
