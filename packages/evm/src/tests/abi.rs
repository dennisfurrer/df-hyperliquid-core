//! Every encoding is viem's: `encodeAbiParameters` over the same types and
//! values (`viem_evm.json`, each case round-tripped by viem first, and
//! hip4-tui's cases in `hip4_tui_evm.json`), selectors and topics from
//! `toFunctionSelector` and `toEventSelector`, calldata from
//! `encodeFunctionData`. The values are built here from the JSON, not by the
//! code under test.

use serde_json::Value;

use super::*;
use crate::fixtures;

/// A token from viem's JSON for a value: integers as decimal strings, hex for
/// addresses and bytes, arrays and tuples as lists (a named tuple as an object).
fn token(kind: &Kind, value: &Value) -> Token {
    match kind {
        Kind::Uint(_) => Token::Uint(fixtures::text(value).parse().unwrap()),
        Kind::Int(_) => {
            let text = fixtures::text(value);
            match text.strip_prefix('-') {
                // Two's complement of the magnitude: 2^256 - m.
                Some(magnitude) => {
                    let magnitude: U256 = magnitude.parse().unwrap();
                    Token::Int(U256::MAX.checked_sub(magnitude.checked_sub(U256::ONE).unwrap()).unwrap())
                }
                None => Token::Int(text.parse().unwrap()),
            }
        }
        Kind::Address => Token::Address(fixtures::address(fixtures::text(value))),
        Kind::Bool => Token::Bool(value.as_bool().unwrap()),
        Kind::String => Token::String(fixtures::text(value).to_owned()),
        Kind::Bytes => Token::Bytes(fixtures::bytes(value)),
        Kind::Fixed(_) => Token::Fixed(fixtures::bytes(value)),
        Kind::Array(inner) | Kind::Sized(inner, _) => {
            Token::Array(value.as_array().unwrap().iter().map(|item| token(inner, item)).collect())
        }
        Kind::Tuple(params) => Token::Tuple(
            params
                .iter()
                .enumerate()
                .map(|(at, param)| match (value, &param.name) {
                    (Value::Object(fields), Some(name)) => token(&param.kind, &fields[name]),
                    _ => token(&param.kind, &value[at]),
                })
                .collect(),
        ),
    }
}

fn hex(bytes: &[u8]) -> String {
    hl_core::hex::encode_prefixed(bytes)
}

#[test]
fn every_viem_case_encodes_and_decodes_byte_for_byte() {
    let viem = fixtures::viem();
    let cases = viem["abi"].as_array().unwrap();
    assert_eq!(cases.len(), 76, "every width of uint and int, and the rest");
    for case in cases {
        let types = fixtures::text(&case["types"]);
        let shape = params(types).unwrap();
        let tokens: Vec<Token> = shape
            .iter()
            .zip(case["values"].as_array().unwrap())
            .map(|(param, value)| token(&param.kind, value))
            .collect();
        let encoded = fixtures::bytes(&case["encoded"]);
        assert_eq!(hex(&encode(&shape, &tokens).unwrap()), fixtures::text(&case["encoded"]), "{types}");
        assert_eq!(decode(&shape, &encoded).unwrap(), tokens, "{types}");
    }
}

#[test]
fn hip4_tuis_cases_encode_from_typed_text() {
    let hip4 = fixtures::hip4_tui();
    for case in hip4["abi"]["encoded"].as_array().unwrap() {
        let types = fixtures::text(&case["types"]);
        let shape = params(types).unwrap();
        let tokens: Vec<Token> = shape
            .iter()
            .zip(case["values"].as_array().unwrap())
            .map(|(param, value)| Token::parse(&param.kind, "value", fixtures::text(value)).unwrap())
            .collect();
        assert_eq!(hex(&encode(&shape, &tokens).unwrap()), fixtures::text(&case["encoded"]), "{types}");
    }
}

#[test]
fn live_and_viem_answers_decode_into_their_structs() {
    let hip4 = fixtures::hip4_tui();
    let answers = &hip4["abi"];
    let read = |types: &str, key: &str| {
        let shape = params(types).unwrap();
        let tokens = decode(&shape, &fixtures::bytes(&answers[key])).unwrap();
        tokens.iter().zip(&shape).map(|(token, param)| token.show(&param.kind)).collect::<Vec<_>>().join("; ")
    };
    // perpAssetInfo(0) and bbo(0), live on mainnet.
    assert_eq!(
        read(
            "(string coin, uint32 marginTableId, uint8 szDecimals, uint8 maxLeverage, bool onlyIsolated)",
            "perpAssetInfo0"
        ),
        "{coin: BTC, marginTableId: 56, szDecimals: 5, maxLeverage: 40, onlyIsolated: false}"
    );
    assert_eq!(read("(uint64 bid, uint64 ask)", "bbo0"), "{bid: 839730, ask: 839740}");
    assert_eq!(read("(string name, uint64[2] tokens)", "spotInfo"), "{name: PURR/USDC, tokens: [1, 0]}");
    assert_eq!(read("(int64 szi, uint64 entryNtl)", "position"), "{szi: -12345, entryNtl: 9}");
    let supply = "(uint64 maxSupply, uint64 totalSupply, uint64 circulatingSupply, uint64 futureEmissions, (address user, uint64 balance)[] nonCirculatingUserBalances)";
    assert_eq!(
        read(supply, "tokenSupply"),
        "{maxSupply: 10, totalSupply: 9, circulatingSupply: 8, futureEmissions: 1, nonCirculatingUserBalances: [{user: 0x00000000000000000000000000000000000000bb, balance: 5}, {user: 0x00000000000000000000000000000000000000cc, balance: 6}]}"
    );
    // And it encodes back to viem's bytes.
    let shape = params(supply).unwrap();
    let bytes = fixtures::bytes(&answers["tokenSupply"]);
    assert_eq!(encode(&shape, &decode(&shape, &bytes).unwrap()).unwrap(), bytes);
}

#[test]
fn selectors_and_topics_are_computed_from_any_spelling() {
    let viem = fixtures::viem();
    for case in viem["selectors"].as_array().unwrap() {
        let signature = fixtures::text(&case["signature"]);
        assert_eq!(hex(&selector(signature).unwrap()), fixtures::text(&case["selector"]), "{signature}");
    }
    for case in viem["topics"].as_array().unwrap() {
        let signature = fixtures::text(&case["signature"]);
        assert_eq!(event_topic(signature).unwrap().to_hex(), fixtures::text(&case["topic"]), "{signature}");
    }
    // The ABI spec hashes `uint` as `uint256`; viem's toFunctionSelector hashes the shorthand as written.
    assert_eq!(selector("g(uint, int)").unwrap(), selector("g(uint256,int256)").unwrap());
    let transfer = Function::parse("function transfer(address to, uint256 amount) external returns (bool)").unwrap();
    assert_eq!(transfer.signature(), "transfer(address,uint256)");
    assert_eq!(transfer.name, "transfer");
    assert_eq!(transfer.outputs.len(), 1);
    assert!(selector("transfer(address,uint256").is_err());
    assert!(selector("(address)").is_err(), "a function has a name");
}

#[test]
fn a_function_writes_viems_calldata_and_reads_its_answer() {
    let viem = fixtures::viem();
    let functions = [
        "transfer(address to, uint256 amount) returns (bool)",
        "balanceOf(address owner) view returns (uint256)",
        "approve(address spender, uint256 amount) returns (bool)",
        "decimals() view returns (uint8)",
        "symbol() view returns (string)",
    ];
    for case in viem["erc20"].as_array().unwrap() {
        let name = fixtures::text(&case["function"]);
        let function =
            functions.iter().map(|text| Function::parse(text).unwrap()).find(|one| one.name == name).unwrap();
        let args: Vec<Token> = function
            .inputs
            .iter()
            .zip(case["args"].as_array().unwrap())
            .map(|(param, value)| token(&param.kind, value))
            .collect();
        let calldata = function.encode(&args).unwrap();
        assert_eq!(hex(&calldata), fixtures::text(&case["calldata"]), "{name}");
        assert_eq!(function.decode_input(&calldata).unwrap(), args, "{name}");
    }
    // symbol() of USDC on HyperEVM, live.
    let symbol = Function::parse("symbol() returns (string)").unwrap();
    let answer: Value = serde_json::from_str(&fixtures::answer("eth_call", 1)).unwrap();
    let said = symbol.decode_output(&fixtures::bytes(&answer["result"])).unwrap();
    assert_eq!(said, [Token::String("USDC".to_owned())]);
    let decimals = Function::parse("decimals() returns (uint8)").unwrap();
    let answer: Value = serde_json::from_str(&fixtures::answer("eth_call", 0)).unwrap();
    assert_eq!(decimals.decode_output(&fixtures::bytes(&answer["result"])).unwrap()[0].as_u8(), Some(6));
    assert!(decimals.decode_input(&[0xde, 0xad, 0xbe, 0xef]).is_err(), "another function's selector");
}

#[test]
fn types_read_in_every_spelling_and_write_canonically() {
    let shape =
        params("(uint64 bid, uint64 ask) bbo, uint, int8[2][] grid, address indexed user, bytes memory data").unwrap();
    let canonical: Vec<String> = shape.iter().map(|param| param.kind.canonical()).collect();
    assert_eq!(canonical, ["(uint64,uint64)", "uint256", "int8[2][]", "address", "bytes"]);
    let names: Vec<Option<&str>> = shape.iter().map(|param| param.name.as_deref()).collect();
    assert_eq!(names, [Some("bbo"), None, Some("grid"), Some("user"), Some("data")]);
    assert!(shape[3].indexed && !shape[4].indexed);
    assert!(Kind::parse("string").unwrap().is_dynamic());
    assert!(Kind::parse("(uint8,string)").unwrap().is_dynamic());
    assert!(Kind::parse("string[2]").unwrap().is_dynamic());
    assert!(!Kind::parse("(uint8,bytes32)[3]").unwrap().is_dynamic());
    for bad in
        ["uint7", "uint264", "int0", "bytes0", "bytes33", "foo", "uint8[x]", "(uint8", "address payable extra name"]
    {
        assert!(params(bad).is_err(), "{bad} is not a type");
    }
    assert!(params(&format!("{}uint8{}", "(".repeat(100), ")".repeat(100))).is_err(), "nesting is bounded");
    assert_eq!(params("  ").unwrap(), []);
}

#[test]
fn a_value_that_does_not_fit_its_type_is_refused_rather_than_encoded() {
    let uint8 = params("uint8").unwrap();
    assert!(matches!(encode(&uint8, &[Token::uint(256u64)]), Err(AbiError::Mismatch { .. })));
    assert!(matches!(encode(&uint8, &[Token::Bool(true)]), Err(AbiError::Mismatch { .. })));
    assert!(matches!(encode(&uint8, &[]), Err(AbiError::Count { wants: 1, got: 0 })));
    let int8 = params("int8").unwrap();
    assert!(encode(&int8, &[Token::int(-128)]).is_ok());
    assert!(matches!(encode(&int8, &[Token::int(128)]), Err(AbiError::Mismatch { .. })));
    assert!(matches!(encode(&int8, &[Token::int(-129)]), Err(AbiError::Mismatch { .. })));
    let fixed = params("bytes4").unwrap();
    assert!(matches!(encode(&fixed, &[Token::Fixed(vec![1, 2, 3])]), Err(AbiError::Mismatch { .. })));
    let sized = params("uint8[2]").unwrap();
    assert!(matches!(encode(&sized, &[Token::Array(vec![Token::uint(1u8)])]), Err(AbiError::Mismatch { .. })));
}

#[test]
fn a_word_wider_than_its_type_is_refused_rather_than_read() {
    let uint8 = params("uint8").unwrap();
    assert!(matches!(decode(&uint8, &[0xff; 32]), Err(AbiError::Misfit { .. })));
    assert!(matches!(decode(&uint8, &[0; 8]), Err(AbiError::Short(8))));
    let mut two = [0u8; 32];
    two[31] = 2;
    assert!(matches!(decode(&params("bool").unwrap(), &two), Err(AbiError::Misfit { .. })));
    assert!(matches!(decode(&params("address").unwrap(), &[0xff; 32]), Err(AbiError::Misfit { .. })));
    let mut dirty = [0u8; 32];
    dirty[0] = 0xaa;
    dirty[31] = 0x01;
    assert!(matches!(decode(&params("bytes1").unwrap(), &dirty), Err(AbiError::Misfit { .. })), "padding is zero");
    // A string whose length runs past the data.
    let mut long = vec![0u8; 64];
    long[31] = 0x20;
    long[63] = 0xff;
    assert!(matches!(decode(&params("string").unwrap(), &long), Err(AbiError::Short(64))));
    // An array claiming more items than the data could hold.
    let mut many = vec![0u8; 64];
    many[31] = 0x20;
    many[60] = 0xff;
    assert!(decode(&params("uint256[]").unwrap(), &many).is_err());
}

#[test]
fn typed_text_is_range_checked_and_shown_back() {
    assert!(Token::parse(&Kind::Uint(8), "x", "256").is_err());
    assert!(Token::parse(&Kind::Uint(8), "x", "255").is_ok());
    assert!(Token::parse(&Kind::Int(8), "x", "-128").is_ok());
    assert!(Token::parse(&Kind::Int(8), "x", "128").is_err());
    assert!(Token::parse(&Kind::Int(8), "x", "-129").is_err());
    assert!(Token::parse(&Kind::Address, "x", "0x12").is_err());
    assert_eq!(Token::parse(&Kind::Int(64), "x", "-12345").unwrap(), Token::int(-12345));
    let min = "-57896044618658097711785492504343953926634992332820282019728792003956564819968";
    let int256 = Token::parse(&Kind::Int(256), "x", min).unwrap();
    assert_eq!(int256.show(&Kind::Int(256)), min);
    assert_eq!(Token::int(i128::MIN).as_i128(), Some(i128::MIN));
    assert_eq!(Token::int(-1).as_i64(), Some(-1));
    assert_eq!(int256.as_i128(), None, "past what an i128 holds");
    assert_eq!(Token::uint(u64::MAX).as_u64(), Some(u64::MAX));
    assert_eq!(Token::uint(u64::MAX).as_u32(), None);
    let error = Token::parse(&Kind::Uint(64), "limitPx", "1.5").unwrap_err();
    assert_eq!(error.to_string(), "limitPx must be a whole number that fits uint64, and `1.5` is not");
}
