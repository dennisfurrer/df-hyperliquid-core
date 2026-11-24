//! Moving money: sends and moves between balances on the venue, USDC in and out through the
//! Arbitrum bridge, spot tokens between HyperCore and HyperEVM, and the ERC-20 calls they need.

pub mod bridge;
pub mod erc20;
pub mod evm;
pub mod send;

pub use crate::utils::sign::{L1Action, UserAction};
pub use evm::{
    AddressEncoding, Destination, HYPE_SYSTEM, Linked, SendToEvmWithData, is_system_address, system_address,
    system_token,
};
pub use send::{
    AgentSendAsset, Amount, BadAmount, Balance, Change, Ctx, Effect, EvmContract, HYPE_EVM_DECIMALS, MAX_DECIMALS,
    PERP_DEX, Plan, PreTransferCheck, Refused, SPOT_DEX, SendAsset, SpotOut, SpotSend, SpotToken, UsdClassTransfer,
    UsdSend, Withdraw3, clean_amount, key, pre_transfer_check, pre_transfer_check_request,
};
