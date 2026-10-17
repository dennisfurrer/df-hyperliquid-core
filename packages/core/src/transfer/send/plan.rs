//! From a move's typed values to exactly what is signed: the action, its nonce, who may sign it,
//! and the balance that should change once it lands. An agent's signature on a user-signed action
//! recovers to the agent, which holds nothing, so [`Signers::Main`] refuses it before signing.

use crate::account::{ActionKind, Keeping, Signers};
use crate::client::{Network, USDC_INDEX};
use crate::transfer::evm::is_system_address;
use crate::transfer::send::actions::{
    AgentSendAsset, PERP_DEX, SPOT_DEX, SendAsset, SpotOut, UsdClassTransfer, UsdSend, Withdraw3,
};
use crate::transfer::send::amount::Amount;
use crate::transfer::send::token::SpotToken;
use crate::utils::{Address, Dec, USDC_DECIMALS};

/// The transfer actions.
const KINDS: [ActionKind; 6] = [
    ActionKind::UsdClassTransfer,
    ActionKind::UsdSend,
    ActionKind::SpotSend,
    ActionKind::SendAsset,
    ActionKind::Withdraw3,
    ActionKind::AgentSendAsset,
];

/// Who may sign a transfer action, by its wire `type`.
#[must_use]
pub fn key(kind: &str) -> Option<Signers> {
    ActionKind::from_wire(kind).filter(|kind| KINDS.contains(kind)).map(ActionKind::signers)
}

/// A figure a read back checks.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Balance {
    /// What a perp dex lets move out, `clearinghouseState`'s `withdrawable`;
    /// `""` is the main dex.
    Perp(String),
    /// A spot token's `total` in `spotClearinghouseState`, by index. A pooled
    /// account's USDC is `Spot(0)`.
    Spot(u32),
}

/// How the figure moves once the action lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// Up by exactly this much.
    Up(Dec),
    /// Down by exactly this much: a withdrawal's fee comes out of the amount.
    Down(Dec),
    /// Down by this much and whatever the venue takes on top: a fresh
    /// recipient's activation fee, or HYPE for a spot send.
    DownAtLeast(Dec),
}

/// The balance an action changes, and what reading it back should show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Effect {
    pub balance: Balance,
    pub change: Change,
}

impl Effect {
    /// Whether `after`, read back, shows the action landed against `before`,
    /// read when it was planned.
    #[must_use]
    pub fn landed(&self, before: Dec, after: Dec) -> bool {
        match self.change {
            Change::Up(amount) => before.checked_add(amount) == Some(after),
            Change::Down(amount) => before.checked_sub(amount) == Some(after),
            Change::DownAtLeast(amount) => before.checked_sub(amount).is_some_and(|most| after <= most),
        }
    }
}

/// Exactly what is signed, and what to check once it lands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan<A> {
    pub network: Network,
    pub action: A,
    /// What it is signed with; a user-signed action carries it too.
    pub nonce: u64,
    pub key: Signers,
    pub effect: Effect,
}

/// Why a move is not signed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Refused {
    #[error("a pooled account holds USDC in one balance, so there is nothing to move between {0}")]
    OneBalance(&'static str),
    #[error("the two balances are the same one")]
    SameDex,
    #[error("that is this account; a send goes somewhere else")]
    ToSelf,
    #[error("{0} is a system address: a send there crosses to HyperEVM, which `to_evm` plans")]
    SystemAddress(Address),
    #[error("{0} has no token on HyperEVM to cross to")]
    NotLinked(String),
    /// No vector covers USDC crossing to HyperEVM, so it is refused.
    #[error("USDC does not cross to HyperEVM here")]
    Usdc,
    #[error("{amount} has more than {places} decimal places")]
    TooFine { amount: Dec, places: u32 },
}

/// Everything outside the amounts that a plan is made from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ctx {
    pub network: Network,
    /// The account that moves the money.
    pub account: Address,
    pub keeping: Keeping,
    pub nonce: u64,
}

impl Ctx {
    /// Where USDC leaving the perp side is read back: spot USDC for a pooled
    /// account, the main dex's `withdrawable` otherwise.
    #[must_use]
    pub fn usdc(&self) -> Balance {
        match self.keeping.pools() {
            true => Balance::Spot(USDC_INDEX),
            false => Balance::Perp(PERP_DEX.to_owned()),
        }
    }

    const fn plan<A>(&self, action: A, key: Signers, effect: Effect) -> Plan<A> {
        Plan { network: self.network, action, nonce: self.nonce, key, effect }
    }

    /// `usdClassTransfer`: USDC between the account's own perp and spot. A
    /// pooled account has one balance and nothing to move.
    pub fn class_transfer(&self, amount: Amount, to_perp: bool) -> Result<Plan<UsdClassTransfer>, Refused> {
        if self.keeping.pools() {
            return Err(Refused::OneBalance("perps and spot"));
        }
        let value = places(&amount, USDC_DECIMALS)?;
        let change = match to_perp {
            true => Change::Up(value),
            false => Change::Down(value),
        };
        let effect = Effect { balance: Balance::Perp(PERP_DEX.to_owned()), change };
        Ok(self.plan(UsdClassTransfer::new(amount, to_perp, self.nonce), UsdClassTransfer::KEY, effect))
    }

    /// `usdSend`: USDC to another account.
    pub fn usd_send(&self, to: Address, amount: Amount) -> Result<Plan<UsdSend>, Refused> {
        self.recipient(to)?;
        let value = places(&amount, USDC_DECIMALS)?;
        let effect = Effect { balance: self.usdc(), change: Change::DownAtLeast(value) };
        Ok(self.plan(UsdSend::new(to, amount, self.nonce), UsdSend::KEY, effect))
    }

    /// A spot token to another account, by the action the account takes.
    pub fn send_token(&self, to: Address, token: &SpotToken, amount: Amount) -> Result<Plan<SpotOut>, Refused> {
        self.recipient(to)?;
        let value = places(&amount, token.wei_decimals)?;
        let effect = Effect { balance: Balance::Spot(token.index), change: Change::DownAtLeast(value) };
        let action = SpotOut::new(to, &token.wire(), amount, self.keeping, self.nonce);
        Ok(self.plan(action, SpotOut::KEY, effect))
    }

    /// Core to HyperEVM: the token to its system address, arriving at the same address on HyperEVM.
    /// The way back is [`crate::transfer::evm::Linked::to_core`].
    pub fn to_evm(&self, token: &SpotToken, amount: Amount) -> Result<Plan<SpotOut>, Refused> {
        if token.index == USDC_INDEX {
            return Err(Refused::Usdc);
        }
        let most = token.crossing_places(self.network).ok_or_else(|| Refused::NotLinked(token.name.clone()))?;
        let value = places(&amount, most)?;
        let effect = Effect { balance: Balance::Spot(token.index), change: Change::DownAtLeast(value) };
        let action = SpotOut::new(token.system_address(self.network), &token.wire(), amount, self.keeping, self.nonce);
        Ok(self.plan(action, SpotOut::KEY, effect))
    }

    /// `sendAsset` to the account itself, from one of its balances to another
    /// (`""` the main dex, `spot`, or a HIP-3 dex by name), signed by its own
    /// key. A pooled account has one balance and nothing to move.
    pub fn between(&self, from: &str, to: &str, token: &SpotToken, amount: Amount) -> Result<Plan<SendAsset>, Refused> {
        let effect = self.between_effect(from, to, token, &amount)?;
        let action = SendAsset::new(self.account, from, to, &token.wire(), amount, self.nonce);
        Ok(self.plan(action, SendAsset::KEY, effect))
    }

    /// [`Ctx::between`] as `agentSendAsset`, which an agent signs.
    pub fn agent_between(
        &self,
        from: &str,
        to: &str,
        token: &SpotToken,
        amount: Amount,
    ) -> Result<Plan<AgentSendAsset>, Refused> {
        let effect = self.between_effect(from, to, token, &amount)?;
        let action = AgentSendAsset::new(self.account, from, to, &token.wire(), amount, self.nonce);
        Ok(self.plan(action, AgentSendAsset::KEY, effect))
    }

    /// `withdraw3`: USDC out to `to` on Arbitrum. The venue's fee comes out of
    /// the amount, so the balance falls by exactly it.
    pub fn withdraw(&self, to: Address, amount: Amount) -> Result<Plan<Withdraw3>, Refused> {
        if is_system_address(to) {
            return Err(Refused::SystemAddress(to));
        }
        let value = places(&amount, USDC_DECIMALS)?;
        let effect = Effect { balance: self.usdc(), change: Change::Down(value) };
        Ok(self.plan(Withdraw3::new(to, amount, self.nonce), Withdraw3::KEY, effect))
    }

    fn recipient(&self, to: Address) -> Result<(), Refused> {
        match (to == self.account, is_system_address(to)) {
            (true, _) => Err(Refused::ToSelf),
            (false, true) => Err(Refused::SystemAddress(to)),
            (false, false) => Ok(()),
        }
    }

    /// The balance a move between two of the account's own leaves, by exactly
    /// the amount. One side is always a perp dex, which counts in millionths.
    fn between_effect(&self, from: &str, to: &str, token: &SpotToken, amount: &Amount) -> Result<Effect, Refused> {
        if self.keeping.pools() {
            return Err(Refused::OneBalance("dexes"));
        }
        if from == to {
            return Err(Refused::SameDex);
        }
        let value = places(amount, token.wei_decimals.min(USDC_DECIMALS))?;
        let balance = match from {
            SPOT_DEX => Balance::Spot(token.index),
            dex => Balance::Perp(dex.to_owned()),
        };
        Ok(Effect { balance, change: Change::Down(value) })
    }
}

/// The amount's value, refused when it has more than `most` places.
fn places(amount: &Amount, most: u32) -> Result<Dec, Refused> {
    let value = amount.value();
    match value.places() > most {
        true => Err(Refused::TooFine { amount: value, places: most }),
        false => Ok(value),
    }
}

#[cfg(test)]
#[path = "tests/plan.rs"]
mod tests;
