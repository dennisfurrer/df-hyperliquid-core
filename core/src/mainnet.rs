//! Mainnet, the only network this family speaks. There is no testnet switch:
//! a constant cannot be pointed at the wrong chain by a setting.

/// Both endpoints live under it: `/info` and `/exchange`.
pub const API_URL: &str = "https://api.hyperliquid.xyz";

/// Arbitrum One. Every user-signed action is signed under this chain and
/// names it as its `signatureChainId`.
pub const SIGNATURE_CHAIN_ID: u64 = 42_161;

/// [`SIGNATURE_CHAIN_ID`] as an action spells it.
pub const SIGNATURE_CHAIN_ID_HEX: &str = "0xa4b1";

/// What a user-signed action names as its `hyperliquidChain`.
pub const HYPERLIQUID_CHAIN: &str = "Mainnet";

/// The phantom agent's `source` for an L1 action on mainnet.
pub const AGENT_SOURCE: &str = "a";

/// The chain id an L1 action's domain carries: a literal, not a real chain.
pub const L1_CHAIN_ID: u64 = 1337;

/// A spot token as the venue names it in each place it appears.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Token {
    pub name: &'static str,
    /// `token` in `spotClearinghouseState`, and the index in `spotMeta`.
    pub index: u32,
    pub token_id: &'static str,
    /// `NAME:0x<tokenId>`, how a transfer names the token.
    pub wire: &'static str,
    /// The decimals the venue counts the token in.
    pub wei_decimals: u32,
}

/// Spot USDC, checked against `spotMeta` on 2026-10-06.
pub const USDC: Token = Token {
    name: "USDC",
    index: 0,
    token_id: "0x6d1e7cde53ba9467b783cb7c530ce054",
    wire: "USDC:0x6d1e7cde53ba9467b783cb7c530ce054",
    wei_decimals: 8,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_chain_id_and_its_spelling_agree() {
        assert_eq!(format!("{SIGNATURE_CHAIN_ID:#x}"), SIGNATURE_CHAIN_ID_HEX);
    }

    #[test]
    fn usdc_s_wire_name_is_its_name_and_token_id() {
        assert_eq!(USDC.wire, format!("{}:{}", USDC.name, USDC.token_id));
        assert_eq!(USDC.token_id.len(), 34);
    }
}
