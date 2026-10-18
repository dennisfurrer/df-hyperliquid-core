//! Live answers and viem's decodings of the same bytes, and words that do not fit the
//! shape they are read against.

use crate::client::evm::abi::{encode, params};
use crate::client::evm::fixtures;

use super::*;

#[test]
fn live_and_viem_answers_decode_into_their_structs() {
    let file = fixtures::cases();
    let answers = &file["abi"];
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
