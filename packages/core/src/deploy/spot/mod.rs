//! Deploying a HIP-1 spot token and its HIP-2 hyperliquidity: every `spotDeploy` variant, all L1
//! actions, and `finalizeEvmContract`, which completes a token's link to a HyperEVM contract. The
//! reads that go with them are in [`tokens`](crate::deploy::tokens).

pub mod evm;
pub mod share;

pub use evm::{EvmFinalizer, FinalizeEvmContract};
pub use share::FeeShare;

use crate::deploy::annotation::Annotation;
use crate::deploy::error::BadDeploy;
use crate::deploy::name::DexName;
use crate::deploy::tokens::TokenSpec;
use crate::utils::sign::{L1Action, Map, Value};
use crate::utils::{Address, Dec};

/// `{"type":"spotDeploy",<variant>:{..}}`. A token is deployed in five steps
/// sent in this order: [`RegisterToken2`](SpotDeploy::RegisterToken2),
/// [`UserGenesis`](SpotDeploy::UserGenesis) (as many times as needed),
/// [`Genesis`](SpotDeploy::Genesis), [`RegisterSpot`](SpotDeploy::RegisterSpot)
/// and [`RegisterHyperliquidity`](SpotDeploy::RegisterHyperliquidity). The
/// rest are optional. Wei amounts are integers in the token's wei decimals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpotDeploy {
    /// `registerToken2`: claims a token index, paying the token auction up
    /// to `max_gas`. `fullName` is left out when `None`.
    RegisterToken2 {
        /// The name, size decimals and wei decimals to register.
        spec: TokenSpec,
        /// The most to pay for the token auction (`maxGas`), in native token wei.
        max_gas: u64,
        /// The full name (`fullName`); `None` leaves it out.
        full_name: Option<String>,
    },
    /// `userGenesis`: wei for addresses, wei shared among the holders of
    /// existing tokens, and addresses to blacklist (`true`) or take off the
    /// blacklist (`false`). `blacklistUsers` is left out when empty.
    UserGenesis {
        /// The token index.
        token: u32,
        /// Wei given to each address (`userAndWei`).
        user_and_wei: Vec<(Address, u128)>,
        /// Wei shared among the holders of each existing token, by token index (`existingTokenAndWei`).
        existing_token_and_wei: Vec<(u32, u128)>,
        /// Addresses to blacklist (`true`) or take off the blacklist (`false`) (`blacklistUsers`).
        blacklist_users: Vec<(Address, bool)>,
    },
    /// `genesis`: the maximum supply in wei, which must equal what the
    /// user genesis steps gave out. `noHyperliquidity` is sent, as `true`,
    /// only when set.
    Genesis {
        /// The token index.
        token: u32,
        /// The maximum supply in wei (`maxSupply`), sent as a string.
        max_supply: u128,
        /// `true` deploys without hyperliquidity (`noHyperliquidity`).
        no_hyperliquidity: bool,
    },
    /// `registerSpot`: the pair of a base and a quote token. Between two
    /// existing tokens it pays the spot pair auction.
    RegisterSpot {
        /// The base token index, sent first in `tokens`.
        base: u32,
        /// The quote token index, sent second in `tokens`.
        quote: u32,
    },
    /// `registerHyperliquidity`: the pair index (not the token's), the
    /// starting price, each order's size in tokens (not wei), the number of
    /// orders (0 after a genesis with no hyperliquidity) and the number of
    /// levels seeded with USDC. `nSeededLevels` is left out when `None`.
    RegisterHyperliquidity {
        /// The spot pair index.
        spot: u32,
        /// The starting price (`startPx`).
        start_px: Dec,
        /// Each order's size in tokens, not wei (`orderSz`).
        order_sz: Dec,
        /// The number of orders (`nOrders`).
        n_orders: u32,
        /// The number of levels seeded with USDC (`nSeededLevels`); `None` leaves it out.
        n_seeded_levels: Option<u32>,
    },
    /// `setDeployerTradingFeeShare`, at any time after `registerToken2`, and
    /// again only to lower it.
    SetDeployerTradingFeeShare {
        /// The token index.
        token: u32,
        /// The share the deployer keeps.
        share: FeeShare,
    },
    /// `enableQuoteToken`: lets the token quote spot pairs.
    EnableQuoteToken(u32),
    /// `enableAlignedQuoteToken`: makes the token an aligned quote asset.
    EnableAlignedQuoteToken(u32),
    /// `disableQuoteToken`.
    DisableQuoteToken(u32),
    /// `disableAlignedQuoteToken`, sent after `disableQuoteToken`.
    DisableAlignedQuoteToken(u32),
    /// `setTokenAnnotation`, at any time after `registerToken2`, at most
    /// once a day.
    SetTokenAnnotation {
        /// The token index.
        token: u32,
        /// The annotation.
        annotation: Annotation,
    },
    /// `setDeployerLabel`: the label shown for the deployer's tokens, set
    /// once per deployer.
    SetDeployerLabel(DexName),
    /// `enableFreezePrivilege`: lets the deployer freeze holders of the
    /// token. In Hyperliquid's Python SDK; not in the API docs.
    EnableFreezePrivilege(u32),
    /// `freezeUser`: freezes (`true`) or unfreezes (`false`) one holder. In
    /// Hyperliquid's Python SDK; not in the API docs.
    FreezeUser {
        /// The token index.
        token: u32,
        /// The holder to freeze or unfreeze.
        user: Address,
        /// `true` freezes, `false` unfreezes.
        freeze: bool,
    },
    /// `revokeFreezePrivilege`: gives the freeze privilege up. In
    /// Hyperliquid's Python SDK; not in the API docs.
    RevokeFreezePrivilege(u32),
    /// `requestEvmContract`: asks to link the token to an ERC-20 on the
    /// HyperEVM; the contract's deployer then sends
    /// [`FinalizeEvmContract`]. `evm_extra_wei_decimals` is the EVM
    /// contract's decimals less the token's wei decimals, from -2 to 18.
    /// Linking cannot be undone.
    RequestEvmContract {
        /// The token index.
        token: u32,
        /// The ERC-20 contract's address on the HyperEVM.
        address: Address,
        /// The EVM contract's decimals less the token's wei decimals (`evmExtraWeiDecimals`), from -2 to 18.
        evm_extra_wei_decimals: i8,
    },
}

/// `[key, value]`, the tuple form the venue's lists take.
fn pair(key: impl Into<Value>, value: impl Into<Value>) -> Value {
    Value::Array(vec![key.into(), value.into()])
}

impl SpotDeploy {
    /// `registerHyperliquidity`, refusing a negative price or size.
    pub fn register_hyperliquidity(
        spot: u32,
        start_px: Dec,
        order_sz: Dec,
        n_orders: u32,
        n_seeded_levels: Option<u32>,
    ) -> Result<Self, BadDeploy> {
        for (what, value) in [("startPx", start_px), ("orderSz", order_sz)] {
            if value.is_negative() {
                return Err(BadDeploy::OutOfRange { what, least: "0", most: "any", value: value.to_wire() });
            }
        }
        Ok(SpotDeploy::RegisterHyperliquidity { spot, start_px, order_sz, n_orders, n_seeded_levels })
    }

    /// `requestEvmContract`, refusing extra wei decimals outside -2 to 18.
    pub fn request_evm_contract(token: u32, address: Address, evm_extra_wei_decimals: i8) -> Result<Self, BadDeploy> {
        if !(-2..=18).contains(&evm_extra_wei_decimals) {
            return Err(BadDeploy::EvmExtraWeiDecimals(evm_extra_wei_decimals));
        }
        Ok(SpotDeploy::RequestEvmContract { token, address, evm_extra_wei_decimals })
    }

    /// The key the variant travels under.
    #[must_use]
    pub const fn wire_name(&self) -> &'static str {
        match self {
            SpotDeploy::RegisterToken2 { .. } => "registerToken2",
            SpotDeploy::UserGenesis { .. } => "userGenesis",
            SpotDeploy::Genesis { .. } => "genesis",
            SpotDeploy::RegisterSpot { .. } => "registerSpot",
            SpotDeploy::RegisterHyperliquidity { .. } => "registerHyperliquidity",
            SpotDeploy::SetDeployerTradingFeeShare { .. } => "setDeployerTradingFeeShare",
            SpotDeploy::EnableQuoteToken(_) => "enableQuoteToken",
            SpotDeploy::EnableAlignedQuoteToken(_) => "enableAlignedQuoteToken",
            SpotDeploy::DisableQuoteToken(_) => "disableQuoteToken",
            SpotDeploy::DisableAlignedQuoteToken(_) => "disableAlignedQuoteToken",
            SpotDeploy::SetTokenAnnotation { .. } => "setTokenAnnotation",
            SpotDeploy::SetDeployerLabel(_) => "setDeployerLabel",
            SpotDeploy::EnableFreezePrivilege(_) => "enableFreezePrivilege",
            SpotDeploy::FreezeUser { .. } => "freezeUser",
            SpotDeploy::RevokeFreezePrivilege(_) => "revokeFreezePrivilege",
            SpotDeploy::RequestEvmContract { .. } => "requestEvmContract",
        }
    }

    /// The body under [`wire_name`](Self::wire_name), keys in the order the
    /// docs and the venue's clients write them.
    #[must_use]
    pub fn body(&self) -> Map {
        let token = |token: u32| Map::new().put("token", token);
        match self {
            SpotDeploy::RegisterToken2 { spec, max_gas, full_name } => {
                let spec = Map::new()
                    .put("name", spec.name.clone())
                    .put("szDecimals", spec.sz_decimals)
                    .put("weiDecimals", spec.wei_decimals);
                Map::new().put("spec", spec).put("maxGas", *max_gas).put_some("fullName", full_name.clone())
            }
            SpotDeploy::UserGenesis { token, user_and_wei, existing_token_and_wei, blacklist_users } => {
                let users = user_and_wei.iter().map(|(user, wei)| pair(user.to_hex(), wei.to_string()));
                let existing = existing_token_and_wei.iter().map(|(token, wei)| pair(*token, wei.to_string()));
                let blacklist = (!blacklist_users.is_empty())
                    .then(|| Value::array(blacklist_users.iter().map(|(user, listed)| pair(user.to_hex(), *listed))));
                Map::new()
                    .put("token", *token)
                    .put("userAndWei", Value::array(users))
                    .put("existingTokenAndWei", Value::array(existing))
                    .put_some("blacklistUsers", blacklist)
            }
            SpotDeploy::Genesis { token, max_supply, no_hyperliquidity } => Map::new()
                .put("token", *token)
                .put("maxSupply", max_supply.to_string())
                .put_some("noHyperliquidity", no_hyperliquidity.then_some(true)),
            SpotDeploy::RegisterSpot { base, quote } => Map::new().put("tokens", Value::array([*base, *quote])),
            SpotDeploy::RegisterHyperliquidity { spot, start_px, order_sz, n_orders, n_seeded_levels } => Map::new()
                .put("spot", *spot)
                .put("startPx", start_px.to_wire())
                .put("orderSz", order_sz.to_wire())
                .put("nOrders", *n_orders)
                .put_some("nSeededLevels", *n_seeded_levels),
            SpotDeploy::SetDeployerTradingFeeShare { token, share } => {
                Map::new().put("token", *token).put("share", share.to_wire())
            }
            SpotDeploy::EnableQuoteToken(id)
            | SpotDeploy::EnableAlignedQuoteToken(id)
            | SpotDeploy::DisableQuoteToken(id)
            | SpotDeploy::DisableAlignedQuoteToken(id)
            | SpotDeploy::EnableFreezePrivilege(id)
            | SpotDeploy::RevokeFreezePrivilege(id) => token(*id),
            SpotDeploy::SetTokenAnnotation { token, annotation } => {
                Map::new().put("token", *token).put("annotation", annotation.wire_after(Map::new()))
            }
            SpotDeploy::SetDeployerLabel(label) => Map::new().put("label", label.as_str().to_owned()),
            SpotDeploy::FreezeUser { token, user, freeze } => {
                Map::new().put("token", *token).put("user", user.to_hex()).put("freeze", *freeze)
            }
            SpotDeploy::RequestEvmContract { token, address, evm_extra_wei_decimals } => Map::new()
                .put("token", *token)
                .put("address", address.to_hex())
                .put("evmExtraWeiDecimals", i64::from(*evm_extra_wei_decimals)),
        }
    }
}

impl L1Action for SpotDeploy {
    fn action(&self) -> Map {
        Map::new().put("type", "spotDeploy").put(self.wire_name(), self.body())
    }
}

#[cfg(test)]
#[path = "tests/spot.rs"]
mod tests;
