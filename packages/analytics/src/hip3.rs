//! HIP-3: every perp dex a builder deployed, how its deployer set it up, and the builders that
//! route perps. The dexes come from hl-hip3's [`Dexes`] (`perpDexs` and `allPerpMetas` lined
//! up), their activity from `metaAndAssetCtxs` with a `dex`. The builders come from Hyperliquid's
//! own day files of builder fills (hl-hydromancer's [`BuilderDay`]), published a builder at a time
//! and never as a ranking, so [`Routing`] ranks the builders the caller asked about.

use hl_core::{Address, Dec};
use hl_hip3::{DexAsset, DexMeta, DexMetaAndAssetCtxs, Dexes, PerpDex};
use hl_hip4::{Deployer, SubDeployer};
use hl_hydromancer::BuilderDay;

use crate::{
    builders::{Board, Window, labels},
    date::At,
    math::{add, share},
};

/// One HIP-3 dex as its deployer set it up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Setup {
    /// Its place in `perpDexs`, the `dex` in its assets' ids.
    pub place: u32,
    /// The `dex:` of its coins.
    pub name: String,
    pub full_name: String,
    pub deployer: Option<Address>,
    pub oracle_updater: Option<Address>,
    pub fee_recipient: Option<Address>,
    /// Who the deployer lets send which deploy actions, the address trusted with most first.
    pub delegates: Vec<SubDeployer>,
    /// The spot token it margins in, by index; 0 is USDC.
    pub collateral: Option<u32>,
    pub assets: usize,
    pub live: usize,
    pub delisted: usize,
    /// The lowest and highest fee scale its live assets carry: a deployer sets one an asset.
    pub fee_scales: Option<(Dec, Dec)>,
    /// Live assets in growth mode.
    pub growth: usize,
    /// Live assets that refuse cross margin.
    pub isolated: usize,
    /// Coins streaming toward an open interest cap.
    pub streaming_caps: usize,
    /// Coins whose funding is scaled.
    pub funding_multipliers: usize,
    /// Coins with an interest rate of their own in their funding.
    pub funding_rates: usize,
    /// Coins whose hourly funding is clamped.
    pub funding_clamps: usize,
}

/// Every HIP-3 dex, in `perpDexs` order.
#[must_use]
pub fn setups(dexes: &Dexes) -> Vec<Setup> {
    dexes.dexes().map(|(place, dex, meta)| setup(place, dex, meta)).collect()
}

/// One dex at `place`, with its meta.
#[must_use]
pub fn setup(place: u32, dex: &PerpDex, meta: &DexMeta) -> Setup {
    let live: Vec<&DexAsset> = meta.universe.iter().filter(|asset| !asset.is_delisted).collect();
    let fee_scales = live.iter().filter_map(|asset| asset.deployer_fee_scale).fold(None, |range, scale| match range {
        None => Some((scale, scale)),
        Some((low, high)) => Some((scale.min(low), scale.max(high))),
    });
    Setup {
        place,
        name: dex.name.clone(),
        full_name: dex.full_name.clone(),
        deployer: dex.deployer,
        oracle_updater: dex.oracle_updater,
        fee_recipient: dex.fee_recipient,
        delegates: delegates(dex),
        collateral: meta.collateral_token,
        assets: meta.universe.len(),
        live: live.len(),
        delisted: meta.universe.len() - live.len(),
        fee_scales,
        growth: live.iter().filter(|asset| asset.growth_mode.as_deref() == Some("enabled")).count(),
        isolated: live.iter().filter(|asset| !asset.allows_cross()).count(),
        streaming_caps: dex.asset_to_streaming_oi_cap.len(),
        funding_multipliers: dex.asset_to_funding_multiplier.len(),
        funding_rates: dex.asset_to_funding_interest_rate.len(),
        funding_clamps: dex.asset_to_funding_clamp.len(),
    }
}

/// The addresses a dex's deployer delegates to, each with what it may send, the one trusted
/// with most first: the shape a HIP-4 venue's sub-deployers have.
#[must_use]
pub fn delegates(dex: &PerpDex) -> Vec<SubDeployer> {
    Deployer {
        deployer: dex.deployer.unwrap_or(Address::ZERO),
        venue: Some(dex.name.clone()),
        sub_deployers: dex.sub_deployers.clone(),
    }
    .keys()
}

/// How an asset margins, in the venue's words: `strictIsolated` or `noCross` where its deployer
/// set one, `isolated` for an isolated-only asset, `cross` otherwise.
#[must_use]
pub fn margin(asset: &DexAsset) -> &str {
    match asset.margin_mode.as_deref() {
        Some(mode) => mode,
        None if asset.only_isolated => "isolated",
        None => "cross",
    }
}

/// Each deployer and the dexes it deployed, the deployer of most first.
#[must_use]
pub fn deployers(dexes: &Dexes) -> Vec<(Address, Vec<String>)> {
    let mut found: Vec<(Address, Vec<String>)> = Vec::new();
    for (_, dex, _) in dexes.dexes() {
        let Some(deployer) = dex.deployer else { continue };
        match found.iter_mut().find(|(who, _)| *who == deployer) {
            Some((_, names)) => names.push(dex.name.clone()),
            None => found.push((deployer, vec![dex.name.clone()])),
        }
    }
    // Stable: deployers of as many dexes keep the order their first dex is listed in.
    found.sort_by(|a, b| b.1.len().cmp(&a.1.len()));
    found
}

/// What a dex's assets are doing now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Activity {
    /// Each asset's open interest, which the venue gives in size, at its mark.
    pub open_interest: Dec,
    /// The day's notional volume.
    pub volume: Dec,
    pub assets: usize,
}

#[must_use]
pub fn activity(ctxs: &DexMetaAndAssetCtxs) -> Activity {
    ctxs.ctxs.iter().fold(Activity::default(), |sum, ctx| Activity {
        open_interest: add(sum.open_interest, ctx.open_interest.checked_mul(ctx.mark_px).unwrap_or(Dec::ZERO)),
        volume: add(sum.volume, ctx.day_ntl_vlm),
        assets: sum.assets + 1,
    })
}

/// The builders that routed perps on one day, from their day files.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Routing {
    /// `YYYYMMDD`.
    pub date: String,
    /// Every builder with a perp fill that day, the most perp volume first.
    pub days: Vec<BuilderDay>,
    /// Builders asked about.
    pub asked: usize,
    /// Builders with no file, or no perp fill in it.
    pub quiet: usize,
    /// Reads that failed.
    pub failed: usize,
    /// The first failure, with the builder it was for.
    pub error: Option<String>,
}

impl Routing {
    /// What each read of a builder's day came to: its day, no file, or why it failed.
    #[must_use]
    pub fn of(date: &str, read: Vec<(Address, Result<Option<BuilderDay>, String>)>) -> Routing {
        let mut routing = Routing { date: date.to_owned(), asked: read.len(), ..Routing::default() };
        for (builder, answer) in read {
            match answer {
                Ok(Some(day)) if day.perps().fills > 0 => routing.days.push(day),
                Ok(_) => routing.quiet += 1,
                Err(error) => {
                    routing.failed += 1;
                    routing.error.get_or_insert_with(|| format!("{builder}: {error}"));
                }
            }
        }
        routing.days.sort_by(|a, b| b.perps().volume.cmp(&a.perps().volume).then_with(|| a.builder.cmp(&b.builder)));
        routing
    }

    /// Perp volume across every ranked builder: a share's whole.
    #[must_use]
    pub fn total(&self) -> Dec {
        self.days.iter().fold(Dec::ZERO, |sum, day| add(sum, day.perps().volume))
    }

    /// What every ranked builder's perp fills paid it.
    #[must_use]
    pub fn fees(&self) -> Dec {
        self.days.iter().fold(Dec::ZERO, |sum, day| add(sum, day.perps().fees))
    }

    /// A builder's share of [`Routing::total`], in hundredths of a percent, cut down.
    #[must_use]
    pub fn share(&self, day: &BuilderDay) -> u32 {
        share(day.perps().volume, self.total())
    }
}

/// The builders worth asking about: the caller's own, the curated list, and the outcome board's
/// builders ranked `top` or better over all time, each once, in that order.
#[must_use]
pub fn candidates(own: &[Address], board: Option<&Board>, top: u32) -> Vec<Address> {
    let ranked = board
        .into_iter()
        .flat_map(|board| board.top(Window::All, top))
        .filter(|row| row.rank.is_some_and(|rank| rank <= top))
        .filter_map(|row| row.builder);
    let mut found: Vec<Address> = Vec::new();
    for builder in own.iter().copied().chain(labels::known()).chain(ranked) {
        if !found.contains(&builder) {
            found.push(builder);
        }
    }
    found
}

/// The UTC day before `now`'s, `YYYYMMDD`: the newest day Hyperliquid has finished writing
/// builder fills for.
#[must_use]
pub fn yesterday(now: At) -> String {
    now.plus_days(-1).compact()
}

#[cfg(test)]
#[path = "tests/dexes.rs"]
mod dexes_tests;

#[cfg(test)]
#[path = "tests/routing.rs"]
mod routing_tests;
