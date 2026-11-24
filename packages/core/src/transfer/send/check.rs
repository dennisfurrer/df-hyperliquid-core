//! What the venue says about a recipient before a send (`preTransferCheck`): whether it is
//! sanctioned, whether it exists, and the fee a first transfer to it pays.

use crate::client::{ApiError, Transport, info, info_request};
use crate::utils::{Address, Dec, Json};
use serde::Deserialize;

/// `{"type":"preTransferCheck","user":<recipient>,"source":<sender>}`.
#[must_use]
pub fn pre_transfer_check_request(recipient: Address, sender: Address) -> String {
    info_request(
        "preTransferCheck",
        &[("user", Json::String(recipient.to_hex())), ("source", Json::String(sender.to_hex()))],
    )
}

/// The venue's answer about a recipient. A fresh one costs an activation fee (live: `"1.0"`),
/// charged to the sender on top of the amount as its own `accountActivationGas` ledger row.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct PreTransferCheck {
    /// A send to it is refused.
    pub is_sanctioned: bool,
    /// Whether the venue has seen the address before (`userExists`).
    pub user_exists: bool,
    /// The activation fee a first transfer to it pays, in USDC; zero when none.
    pub fee: Dec,
    /// Whether the address has sent a transaction of its own (`userHasSentTx`).
    pub user_has_sent_tx: bool,
}

impl PreTransferCheck {
    /// The fee a first transfer to this address pays, when there is one.
    #[must_use]
    pub fn activation_fee(&self) -> Option<Dec> {
        self.fee.is_positive().then_some(self.fee)
    }
}

/// Reads `preTransferCheck`; see [`pre_transfer_check_request`].
pub async fn pre_transfer_check(
    transport: &impl Transport,
    recipient: Address,
    sender: Address,
) -> Result<PreTransferCheck, ApiError> {
    info(transport, pre_transfer_check_request(recipient, sender)).await
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    /// Both answers in `pre_transfer_check_mainnet.json`, live on mainnet, 2026-10-18.
    #[test]
    fn a_fresh_recipient_costs_a_fee_and_a_known_one_does_not() {
        let recorded: BTreeMap<String, PreTransferCheck> = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/fixtures/transfer/pre_transfer_check_mainnet.json"
        )))
        .unwrap();
        let fresh = &recorded["fresh"];
        assert_eq!(
            *fresh,
            PreTransferCheck { is_sanctioned: false, user_exists: false, fee: Dec::ONE, user_has_sent_tx: false }
        );
        assert_eq!(fresh.activation_fee(), Some(Dec::ONE));
        let known = &recorded["known"];
        assert!(known.user_exists && known.user_has_sent_tx && !known.is_sanctioned);
        assert_eq!(known.activation_fee(), None);
    }
}
