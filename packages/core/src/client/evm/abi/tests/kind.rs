//! Types in every spelling Solidity and viem write them, against their canonical form.

use super::*;

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
