//! What to print when the venue answers "User or API Wallet 0x... does not exist": the likely
//! causes first, then every input that went into the hash, then each stage of the signature from
//! the crate's `Breakdown`, so a reader can see where what was signed and what the venue expected
//! differ.
//!
//! The text is built from the breakdown, the recovered address and public facts only. It is never
//! handed the key, so no build can print it.

use std::fmt::Write as _;

use df_hyperliquid_core::utils::sign::Breakdown;
use df_hyperliquid_core::{Address, Network};

/// Whether `message` is the venue's answer for a signature whose address it does not know.
pub fn does_not_exist(message: &str) -> bool {
    message.contains("does not exist")
}

/// The inputs of the hash and the public addresses the printout shows. No key.
#[derive(Debug, Clone)]
pub struct Context {
    /// The network the request was signed for.
    pub network: Network,
    /// Where the request was sent.
    pub sent_to: String,
    /// The request's nonce.
    pub nonce: u64,
    /// The vault or sub-account the request acted for; `None` when it acted for the account.
    pub vault: Option<Address>,
    /// The request's `expiresAfter`, in milliseconds; `None` when it had none.
    pub expires_after: Option<u64>,
    /// The address of the key that signed.
    pub key_address: Address,
    /// The account traded.
    pub account: Address,
}

/// The printout: the likely causes, the inputs of the hash, then the stages of the signature.
pub fn explain(message: &str, breakdown: &Breakdown, recovered: Result<Address, String>, context: &Context) -> String {
    let mut text = String::new();
    let none = || "none".to_owned();
    let _ = writeln!(text, "the venue answered: {message}");
    let _ = writeln!(text, "likely causes:");
    let _ = writeln!(text, "  - The account has never deposited, so the venue does not know the address.");
    let _ = writeln!(
        text,
        "  - An agent (API) wallet is signing, but it has not been approved for this account, or it was approved on the other network."
    );
    let _ = writeln!(text, "  - The request was signed for mainnet and sent to testnet, or the other way round.");
    let _ = writeln!(
        text,
        "  - A vault or sub-account address is set when it should not be, or missing when it should be."
    );
    let _ = writeln!(text, "what went into the hash:");
    let _ = writeln!(text, "  nonce          {}", context.nonce);
    let _ = writeln!(text, "  vault          {}", context.vault.map_or_else(none, |vault| vault.to_string()));
    let _ = writeln!(text, "  expiresAfter   {}", context.expires_after.map_or_else(none, |at| at.to_string()));
    let _ =
        writeln!(text, "  network        {}, source \"{}\"", context.network.name(), context.network.agent_source());
    let _ = writeln!(text, "  sent to        {}", context.sent_to);
    let _ = writeln!(text, "the signature, stage by stage:");
    let _ = writeln!(text, "  msgpack        {} bytes: {}", breakdown.msgpack.len(), hex(&breakdown.msgpack));
    let _ = writeln!(text, "  action hash    0x{}", hex(&breakdown.action_hash));
    let _ = writeln!(text, "  EIP-712        {}", breakdown.payload);
    let _ = writeln!(text, "  digest         0x{}", hex(&breakdown.digest));
    let recovered = match recovered {
        Ok(address) => address.to_string(),
        Err(error) => format!("none ({error})"),
    };
    let _ = writeln!(text, "  recovered      {recovered}");
    let _ = writeln!(text, "  key address    {}", context.key_address);
    let _ = write!(text, "  account        {}", context.account);
    text
}

/// Lowercase hex, two characters a byte.
fn hex(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(text, "{byte:02x}");
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use df_hyperliquid_core::account::LocalKey;
    use df_hyperliquid_core::trade::{L1Action, ScheduleCancel};

    /// A key made for this test only.
    const KEY: &str = "4c0883a69102937d6231471b5dbb6204fe5129617082792ae468d01a3f362318";

    #[test]
    fn the_printout_has_the_causes_inputs_and_stages_but_never_the_key() {
        let key = LocalKey::from_hex(KEY).unwrap();
        let action = ScheduleCancel::at(1_700_000_060_000);
        let nonce = 1_700_000_000_000;
        let digest = action.digest(nonce, None, Network::Testnet);
        let signature = key.sign_digest(&digest).unwrap();
        let breakdown = Breakdown::of(&action, nonce, None, Network::Testnet);
        assert_eq!(breakdown.digest, digest);
        let recovered = breakdown.recover(&signature).map_err(|error| error.to_string());
        let context = Context {
            network: Network::Testnet,
            sent_to: "https://api.hyperliquid-testnet.xyz/exchange".to_owned(),
            nonce,
            vault: None,
            expires_after: None,
            key_address: key.address(),
            account: key.address(),
        };
        let text = explain("User or API Wallet 0x0 does not exist.", &breakdown, recovered, &context);

        assert!(text.contains("has never deposited"));
        assert!(text.contains("signed for mainnet and sent to testnet"));
        assert!(text.contains("nonce          1700000000000"));
        assert!(text.contains("source \"b\""));
        assert!(text.contains(&hex(&breakdown.digest)));
        assert!(text.contains(&format!("recovered      {}", key.address())));
        assert!(text.find("never deposited") < text.find("msgpack"));
        let lower = text.to_lowercase();
        assert!(!lower.contains(KEY));
        assert!(!lower.contains(&KEY[..16]));
        assert!(!lower.contains(&KEY[KEY.len() - 16..]));
    }

    #[test]
    fn only_the_unknown_address_answer_is_explained() {
        assert!(does_not_exist("User or API Wallet 0x1234 does not exist."));
        assert!(!does_not_exist("Post only order would have immediately matched"));
    }
}
