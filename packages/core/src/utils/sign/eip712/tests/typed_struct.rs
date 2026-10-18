//! Type strings and hashes as viem writes them, the digest, the typed data a wallet is handed, and
//! one approval signed as another SDK signed it.

use super::*;
use crate::account::{LocalKey, recover};
use crate::client::Network;
use crate::utils::Address;
use crate::utils::hex;
use crate::utils::sign::eip712::wallet::digest_of_wallet_json;

const TO: &str = "0x0000000000000000000000000000000000000001";
const USDC: &str = "USDC:0x6d1e7cde53ba9467b783cb7c530ce054";

fn chain() -> Field<'static> {
    Field::String("Mainnet")
}

/// Type and struct hashes from viem; a renamed, retyped or reordered field
/// changes the type hash and the venue rejects the signature.
#[test]
fn the_type_hashes_are_the_published_constants() {
    let agent =
        TypedStruct::new("Agent").field("source", Field::String("a")).field("connectionId", Field::Bytes32([0; 32]));
    assert_eq!(agent.type_signature(), "Agent(string source,bytes32 connectionId)");
    assert_eq!(hex::encode(&agent.type_hash()), "26f05c2f7239b6983075e58321292d77b3aa173d19b27257ac96ab362570f508");

    let approve = TypedStruct::new("HyperliquidTransaction:ApproveAgent")
        .field("hyperliquidChain", chain())
        .field("agentAddress", Field::Address(Address::ZERO))
        .field("agentName", Field::String(""))
        .field("nonce", Field::Uint64(0));
    assert_eq!(
        approve.type_signature(),
        "HyperliquidTransaction:ApproveAgent(string hyperliquidChain,address agentAddress,string agentName,uint64 nonce)"
    );
    assert_eq!(hex::encode(&approve.type_hash()), "4464abf6148f115d79362d98320081aa2ae8a5c840083cb65820bffc21394378");

    let two = |name: &'static str| {
        TypedStruct::new(name)
            .field("hyperliquidChain", chain())
            .field("destination", Field::String(TO))
            .field("amount", Field::String("100"))
            .field("time", Field::Uint64(1))
    };
    assert_eq!(
        hex::encode(&two("HyperliquidTransaction:UsdSend").type_hash()),
        "c7410964737e2180e258d649f23f74a49460432fecbd94688103ecc63fb66b4c"
    );
    assert_eq!(
        hex::encode(&two("HyperliquidTransaction:Withdraw").type_hash()),
        "dbde952fa0de88158fc71336e44c61876611a2a65553bebd4d52b44a2a000d9a"
    );
    let class = TypedStruct::new("HyperliquidTransaction:UsdClassTransfer")
        .field("hyperliquidChain", chain())
        .field("amount", Field::String("100"))
        .field("toPerp", Field::Bool(true))
        .field("nonce", Field::Uint64(1));
    assert_eq!(hex::encode(&class.type_hash()), "7a3eebb1997d2e459472886f8cf9829a79b1b00c72a06c9fe015c91299aecd43");

    let send = TypedStruct::new("HyperliquidTransaction:SpotSend")
        .field("hyperliquidChain", chain())
        .field("destination", Field::String(TO))
        .field("token", Field::String(USDC))
        .field("amount", Field::String("100"))
        .field("time", Field::Uint64(1));
    assert_eq!(hex::encode(&send.type_hash()), "d6d9472f429b5debb4cc4702fa9925e621498472a979f2581553dd007e273983");
    assert_eq!(hex::encode(&send.hash()), "809c0b67b47a35e22436c0b2da95ca3e6cb3a00b0679ff37ad4e25cbc688980b");

    let asset = TypedStruct::new("HyperliquidTransaction:SendAsset")
        .field("hyperliquidChain", chain())
        .field("destination", Field::String(TO))
        .field("sourceDex", Field::String("spot"))
        .field("destinationDex", Field::String(""))
        .field("token", Field::String(USDC))
        .field("amount", Field::String("100"))
        .field("fromSubAccount", Field::String(""))
        .field("nonce", Field::Uint64(1));
    assert_eq!(hex::encode(&asset.type_hash()), "5f7cf397b977b076f0fbb993badfc7dd11bd71461a3d076c0940eff5f91ac849");
    assert_eq!(hex::encode(&asset.hash()), "2a2c9b5173d950a36113e0f76daed9c19665fc9c390ffcd5729557773f56bc87");
}

#[test]
fn wallet_json_is_the_shape_eth_sign_typed_data_v4_takes() {
    let data = TypedStruct::new("HyperliquidTransaction:UsdClassTransfer")
        .field("hyperliquidChain", chain())
        .field("amount", Field::String("12.5"))
        .field("toPerp", Field::Bool(true))
        .field("nonce", Field::Uint64(1_790_412_105_001));
    assert_eq!(
        data.wallet_json(&Domain::user_signed(Network::Mainnet)),
        concat!(
            r#"{"types":{"EIP712Domain":[{"name":"name","type":"string"},{"name":"version","type":"string"},"#,
            r#"{"name":"chainId","type":"uint256"},{"name":"verifyingContract","type":"address"}],"#,
            r#""HyperliquidTransaction:UsdClassTransfer":[{"name":"hyperliquidChain","type":"string"},"#,
            r#"{"name":"amount","type":"string"},{"name":"toPerp","type":"bool"},{"name":"nonce","type":"uint64"}]},"#,
            r#""primaryType":"HyperliquidTransaction:UsdClassTransfer","#,
            r#""domain":{"name":"HyperliquidSignTransaction","version":"1","chainId":42161,"#,
            r#""verifyingContract":"0x0000000000000000000000000000000000000000"},"#,
            r#""message":{"hyperliquidChain":"Mainnet","amount":"12.5","toPerp":true,"nonce":1790412105001}}"#,
        )
    );
    let parsed: serde_json::Value =
        serde_json::from_str(&data.wallet_json(&Domain::user_signed(Network::Mainnet))).unwrap();
    assert!(parsed["domain"]["chainId"].is_u64(), "the chain id is a number");
    assert!(parsed["message"]["nonce"].is_u64(), "a uint64 is a number");
    assert!(parsed["message"]["toPerp"].is_boolean(), "a bool is a bool, never the string \"false\"");
}

/// `@nktkas/hyperliquid` 0.24.3 with the first Hardhat key, digest from viem: `0x1901`, the
/// domain separator and the struct hash. A second SDK run and a second key, and an `address`
/// field through the wallet JSON.
#[test]
fn an_approve_agent_digests_and_signs_as_the_sdk_did_with_the_hardhat_key() {
    let key = LocalKey::from_hex("0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80").unwrap();
    assert_eq!(key.address().to_hex(), "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266");
    let data = TypedStruct::new("HyperliquidTransaction:ApproveAgent")
        .field("hyperliquidChain", chain())
        .field("agentAddress", Field::Address("0x70997970C51812dc3A010C7d01b50e0d17dc79C8".parse().unwrap()))
        .field("agentName", Field::String("globeli"))
        .field("nonce", Field::Uint64(1_790_000_000_000));
    let domain = Domain::user_signed(Network::Mainnet);
    let digest = data.digest(&domain);
    assert_eq!(hex::encode_prefixed(&digest), "0x47449c3d7573a7bae22b7c7e63ee5dac9eb3d62e26994d1ab5fe336b7176efbb");
    assert_eq!(digest, keccak256(&[&[0x19, 0x01][..], &domain.separator(), &data.hash()].concat()));
    assert_eq!(digest, super::digest(&domain, &data));
    assert_eq!(digest_of_wallet_json(&data.wallet_json(&Domain::user_signed(Network::Mainnet))), Ok(digest));
    let signature = key.sign_digest(&digest).unwrap();
    assert_eq!(signature.r_hex(), "0xf8c6a1ca4acd448da6154085b6b273cf3c9539d0ace4431b2edec9ed4bcc2a0e");
    assert_eq!(signature.s_hex(), "0x42b374aecc1c70b44df5106ee801c65fe44bdfa258d68b5c1e7087b15369c4f4");
    assert_eq!(signature.v(), 28);
    assert_eq!(recover(&digest, &signature), Ok(key.address()));
}
