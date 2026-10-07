//! What the venue says about a recipient before a send, and which addresses
//! a send must never go to.

use hl_core::{Address, ApiError, Dec, Json, Transport, info};
use serde::Deserialize;

/// `{"type":"preTransferCheck","user":<recipient>,"source":<sender>}`.
#[must_use]
pub fn pre_transfer_check_request(recipient: Address, sender: Address) -> String {
    Json::Object(vec![
        ("type".to_owned(), Json::String("preTransferCheck".to_owned())),
        ("user".to_owned(), Json::String(recipient.to_hex())),
        ("source".to_owned(), Json::String(sender.to_hex())),
    ])
    .compact()
}

/// The venue's answer about a recipient. A fresh one costs an activation fee
/// (live: `"1.0"`), charged to the sender on top of the amount as its own
/// `accountActivationGas` ledger row.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreTransferCheck {
    /// A send to it is refused.
    pub is_sanctioned: bool,
    pub user_exists: bool,
    pub fee: Dec,
    pub user_has_sent_tx: bool,
}

impl PreTransferCheck {
    /// The fee a first transfer to this address pays, when there is one.
    #[must_use]
    pub fn activation_fee(&self) -> Option<Dec> {
        self.fee.is_positive().then_some(self.fee)
    }
}

pub async fn pre_transfer_check(
    transport: &impl Transport,
    recipient: Address,
    sender: Address,
) -> Result<PreTransferCheck, ApiError> {
    info(transport, pre_transfer_check_request(recipient, sender)).await
}

/// Where native HYPE crosses from Core to HyperEVM.
pub const HYPE_SYSTEM: Address = Address::from_bytes([0x22; 20]);

/// The address a linked spot token crosses to HyperEVM through: `0x20`, then
/// the token index big-endian in the low bytes. Token 1452 is `0x2000…05ac`.
#[must_use]
pub fn system_address(index: u64) -> Address {
    let mut bytes = [0u8; 20];
    bytes[0] = 0x20;
    bytes[12..].copy_from_slice(&index.to_be_bytes());
    Address::from_bytes(bytes)
}

/// Whether a send to `address` would leave Core for HyperEVM rather than
/// reach a person: `0x2222…2222`, or `0x20`, zeros and a token index.
#[must_use]
pub fn is_system_address(address: Address) -> bool {
    let bytes = address.bytes();
    address == HYPE_SYSTEM || (bytes[0] == 0x20 && bytes[1..12].iter().all(|&byte| byte == 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn address(text: &str) -> Address {
        text.parse().unwrap()
    }

    #[test]
    fn the_check_names_the_recipient_as_user_and_the_sender_as_source() {
        assert_eq!(
            pre_transfer_check_request(
                address("0x0D1d9635D0640821d15e323ac8AdADfA9c111414"),
                address("0x14791697260E4c9A71f18484C9f997B308e59325")
            ),
            concat!(
                r#"{"type":"preTransferCheck","user":"0x0d1d9635d0640821d15e323ac8adadfa9c111414","#,
                r#""source":"0x14791697260e4c9a71f18484c9f997b308e59325"}"#
            )
        );
    }

    /// Both live answers, mainnet, 2026-10-06.
    #[test]
    fn a_fresh_recipient_costs_a_fee_and_a_known_one_does_not() {
        let fresh: PreTransferCheck =
            serde_json::from_str(r#"{"isSanctioned":false,"userExists":false,"fee":"1.0","userHasSentTx":false}"#)
                .unwrap();
        assert_eq!(
            fresh,
            PreTransferCheck { is_sanctioned: false, user_exists: false, fee: Dec::ONE, user_has_sent_tx: false }
        );
        assert_eq!(fresh.activation_fee(), Some(Dec::ONE));
        let known: PreTransferCheck =
            serde_json::from_str(r#"{"isSanctioned":false,"userExists":true,"fee":"0.0","userHasSentTx":true}"#)
                .unwrap();
        assert!(known.user_exists && known.user_has_sent_tx && !known.is_sanctioned);
        assert_eq!(known.activation_fee(), None);
    }

    #[test]
    fn system_addresses_are_the_hype_one_and_every_token_s() {
        assert_eq!(system_address(1452).to_hex(), "0x20000000000000000000000000000000000005ac");
        assert_eq!(system_address(0).to_hex(), "0x2000000000000000000000000000000000000000");
        assert_eq!(HYPE_SYSTEM.to_hex(), "0x2222222222222222222222222222222222222222");
        for system in [
            HYPE_SYSTEM,
            system_address(0),
            system_address(150),
            system_address(u64::MAX),
            address("0x200000000000000000000000000000000000007c"),
        ] {
            assert!(is_system_address(system), "{system}");
        }
        for person in [
            address("0x0d1d9635d0640821d15e323ac8adadfa9c111414"),
            address("0x2000000000000000000000010000000000000001"),
            address("0x2100000000000000000000000000000000000001"),
            address("0x2222222222222222222222222222222222222223"),
            Address::ZERO,
        ] {
            assert!(!is_system_address(person), "{person}");
        }
    }
}
