use super::*;
use crate::account::{LocalKey, recover};
use crate::client::Network;
use crate::utils::hex;

const TO: &str = "0x0000000000000000000000000000000000000001";
const USDC: &str = "USDC:0x6d1e7cde53ba9467b783cb7c530ce054";

fn chain() -> Field<'static> {
    Field::String("Mainnet")
}

#[test]
fn the_domain_separators_are_the_published_constants() {
    assert_eq!(
        hex::encode(&Domain::EXCHANGE.separator()),
        "d79297fcdf2ffcd4ae223d01edaa2ba214ff8f401d7c9300d995d17c82aa4040"
    );
    assert_eq!(
        hex::encode(&Domain::user_signed(Network::Mainnet).separator()),
        "2f54e643b38c9c2b65cb731aaf11c54f01705ee0cdf45aea16e2ac0a5ce7531a"
    );
    assert_eq!(Domain::EXCHANGE.chain_id, 1337);
    assert_eq!(Domain::user_signed(Network::Mainnet).chain_id, 42_161);
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
fn the_digest_is_the_prefixed_concatenation() {
    let data =
        TypedStruct::new("Agent").field("source", Field::String("a")).field("connectionId", Field::Bytes32([0xab; 32]));
    let expected = keccak256(&[&[0x19, 0x01][..], &Domain::EXCHANGE.separator(), &data.hash()].concat());
    assert_eq!(digest(&Domain::EXCHANGE, &data), expected);
    assert_eq!(data.digest(&Domain::EXCHANGE), expected);
}

fn every_kind() -> TypedStruct<'static> {
    TypedStruct::new("HyperliquidTransaction:Everything")
        .field("hyperliquidChain", chain())
        .field("label", Field::String("a \"quoted\" name\n\u{e9}"))
        .field("who", Field::Address("0x0D1d9635D0640821d15e323ac8AdADfA9c111414".parse().unwrap()))
        .field("wei", Field::Uint64(u64::MAX))
        .field("id", Field::Bytes32([0x5a; 32]))
        .field("toPerp", Field::Bool(false))
        .field("nonce", Field::Uint64(1_790_412_105_001))
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

#[test]
fn wallet_json_hashes_back_to_the_digest_for_every_field_kind() {
    for domain in [Domain::user_signed(Network::Mainnet), Domain::EXCHANGE] {
        let data = every_kind();
        let json = data.wallet_json(&domain);
        assert_eq!(digest_of_wallet_json(&json), Ok(data.digest(&domain)), "{json}");
    }
}

#[test]
fn the_json_hasher_notices_a_changed_value_or_order() {
    let data = every_kind();
    let json = data.wallet_json(&Domain::user_signed(Network::Mainnet));
    let right = data.digest(&Domain::user_signed(Network::Mainnet));
    for (from, to) in [
        (r#""toPerp":false"#, r#""toPerp":true"#),
        (r#""chainId":42161"#, r#""chainId":1"#),
        ("1790412105001", "1790412105002"),
        (r#""hyperliquidChain":"Mainnet""#, r#""hyperliquidChain":"mainnet""#),
        (
            r#"{"name":"toPerp","type":"bool"},{"name":"nonce","type":"uint64"}"#,
            r#"{"name":"nonce","type":"uint64"},{"name":"toPerp","type":"bool"}"#,
        ),
    ] {
        assert!(json.contains(from), "{from}");
        let changed = json.replacen(from, to, 1);
        assert_ne!(digest_of_wallet_json(&changed), Ok(right), "{from} -> {to}");
    }
    assert!(digest_of_wallet_json(r#"{"types":{}}"#).is_err());
    assert!(digest_of_wallet_json(&json.replacen(r#""toPerp":false"#, r#""toPerp":"false""#, 1)).is_err());
}

/// `@nktkas/hyperliquid` 0.24.3 with the first Hardhat key, digest from viem.
/// A second SDK run and a second key, and an `address` field through the
/// wallet JSON.
#[test]
fn an_approve_agent_signs_as_the_sdk_did_with_the_hardhat_key() {
    let key = LocalKey::from_hex("0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80").unwrap();
    assert_eq!(key.address().to_hex(), "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266");
    let data = TypedStruct::new("HyperliquidTransaction:ApproveAgent")
        .field("hyperliquidChain", chain())
        .field("agentAddress", Field::Address("0x70997970C51812dc3A010C7d01b50e0d17dc79C8".parse().unwrap()))
        .field("agentName", Field::String("globeli"))
        .field("nonce", Field::Uint64(1_790_000_000_000));
    let digest = data.digest(&Domain::user_signed(Network::Mainnet));
    assert_eq!(hex::encode_prefixed(&digest), "0x47449c3d7573a7bae22b7c7e63ee5dac9eb3d62e26994d1ab5fe336b7176efbb");
    assert_eq!(digest_of_wallet_json(&data.wallet_json(&Domain::user_signed(Network::Mainnet))), Ok(digest));
    let signature = key.sign_digest(&digest).unwrap();
    assert_eq!(signature.r_hex(), "0xf8c6a1ca4acd448da6154085b6b273cf3c9539d0ace4431b2edec9ed4bcc2a0e");
    assert_eq!(signature.s_hex(), "0x42b374aecc1c70b44df5106ee801c65fe44bdfa258d68b5c1e7087b15369c4f4");
    assert_eq!(signature.v(), 28);
    assert_eq!(recover(&digest, &signature), Ok(key.address()));
}

/// The wider numbers HyperEVM contracts sign: their words, how a wallet is
/// handed them (a `uint128` or `uint256` as a decimal string, as viem sends a
/// bigint), and a `uint256[]` as the keccak of its words. Each was checked
/// against viem.
#[test]
fn the_wide_numbers_encode_as_solidity_does() {
    assert_eq!(Field::Uint32(7).encode(), uint256(7));
    assert_eq!(Field::Uint128(u128::MAX).encode(), uint256(u128::MAX));
    let ids = [uint256(1), uint256(2)];
    let array = Field::Uint256Array(&ids);
    assert_eq!(array.solidity_type(), "uint256[]");
    assert_eq!(array.encode(), keccak256(&[uint256(1), uint256(2)].concat()));
    assert_eq!(Field::Uint256Array(&[]).encode(), keccak256(&[]));
    assert_eq!(Field::Uint128(10).to_json(), Json::String("10".to_owned()));
    assert_eq!(Field::Uint32(10).to_json(), Json::Number("10".to_owned()));
    assert_eq!(array.to_json().compact(), r#"["1","2"]"#);

    let data = TypedStruct::new("Close")
        .field("user", Field::Address(Address::ZERO))
        .field("positionIds", array)
        .field("size", Field::Uint128(u128::MAX))
        .field("leverage", Field::Uint32(1000))
        .field("deadline", Field::Uint256(uint256(1_791_864_000)));
    let domain = Domain::EXCHANGE;
    assert_eq!(digest_of_wallet_json(&data.wallet_json(&domain)), Ok(data.digest(&domain)));
    let json = data.wallet_json(&domain);
    let as_number = json.replacen(r#""1791864000""#, "1791864000", 1);
    assert_eq!(digest_of_wallet_json(&as_number), Ok(data.digest(&domain)), "a number either way");
    let too_wide = json.replacen(r#""leverage":1000"#, r#""leverage":4294967296"#, 1);
    assert!(digest_of_wallet_json(&too_wide).is_err(), "past a uint32");
}

#[test]
fn a_word_reads_and_writes_as_decimal() {
    for digits in ["0", "1", "10000000000000000000", "340282366920938463463374607431768211455"] {
        assert_eq!(word_decimal(&decimal_word(digits).unwrap()), digits);
    }
    let max = "115792089237316195423570985008687907853269984665640564039457584007913129639935";
    assert_eq!(decimal_word(max), Some([0xff; 32]));
    assert_eq!(word_decimal(&[0xff; 32]), max);
    assert_eq!(decimal_word("115792089237316195423570985008687907853269984665640564039457584007913129639936"), None);
    assert_eq!(decimal_word(""), None);
    assert_eq!(decimal_word("1e3"), None);
    assert_eq!(decimal_word("-1"), None);
}
