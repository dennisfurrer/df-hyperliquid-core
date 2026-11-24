//! HIP-3 builder-deployed perp dexes: the dex list, every dex lined up with its meta, each dex's
//! limits and status, and each perp's category and annotation. A dex's meta and contexts are
//! [`crate::markets::perps`]'s.

pub mod annotations;
pub mod categories;
pub mod dex;
pub mod limits;
pub mod listing;

pub use annotations::{
    ConciseAnnotation, PerpAnnotation, perp_annotation, perp_annotation_request, perp_concise_annotations,
    perp_concise_annotations_request,
};
pub use categories::{perp_categories, perp_categories_request};
pub use dex::{PerpDex, PerpDexs, perp_dexs, perp_dexs_request};
pub use limits::{
    DexLimits, DexStatus, perp_dex_limits, perp_dex_limits_request, perp_dex_status, perp_dex_status_request,
};
pub use listing::{Dexes, Misaligned, all_perp_metas, all_perp_metas_request, dexes};

#[cfg(test)]
#[path = "tests/dexes.rs"]
mod dexes_tests;
