//! The direct reader gives the frame the serde reader gives, or declines. Every recorded `l2Book`,
//! `trades` and `bbo` frame reads here, and so does every changed copy of one that the serde
//! reader also reads the same way; a copy it declines goes to the serde reader in `decode`.

use super::*;
use crate::live::fixtures::all;
use crate::live::frame::one_pass::decode_in_one;
use crate::live::frame::read::decode_in_two;

fn hot_frames() -> Vec<(String, String)> {
    all()
        .into_iter()
        .filter(|(_, text)| {
            ["l2Book", "trades", "bbo"].iter().any(|c| text.starts_with(&format!(r#"{{"channel":"{c}""#)))
        })
        .collect()
}

/// Copies of `text` with one thing changed: whitespace, an escape, another spelling of a number
/// or a decimal, a key renamed, added or dropped, a cut.
fn changed(text: &str) -> Vec<String> {
    let mut copies = Vec::new();
    let step = if text.len() > 2_000 { 61 } else { 1 };
    for at in (0..text.len()).step_by(step).filter(|at| text.is_char_boundary(*at)) {
        copies.push(text[..at].to_owned());
        copies.push(format!("{} {}", &text[..at], &text[at..]));
    }
    for (from, to) in [
        (r#"":"#, r#"": "#),
        (r#""BTC""#, r#""BTC ""#),
        (r#""n":"#, r#""n":0"#),
        (r#""n":"#, r#""n":-"#),
        (r#""n":"#, r#""n":4294967296"#),
        (r#""time":"#, r#""time":0"#),
        (r#""time":1"#, r#""time":1.0"#),
        (r#""tid":"#, r#""tid":99999999999999999999"#),
        (r#""px":""#, r#""px":"+"#),
        (r#""px":""#, r#""px":" "#),
        (r#""px":""#, r#""px":"-"#),
        (r#""sz":""#, r#""sz":"0000000000000000000000"#),
        (r#""px":"#, r#""px":7,"x":"#),
        (r#""px":""#, r#""px":1,"y":""#),
        (r#""side":"A""#, r#""side":"C""#),
        (r#""side":"B""#, r#""side":"b""#),
        (r#""users":[""#, r#""users":[" "#),
        (r#""users":[""#, r#""users":["0X"#),
        (r#""coin":"#, r#""coin2":"#),
        (r#""coin":"#, r#""extra":1,"coin":"#),
        (r#""levels":[["#, r#""levels":[[],["#),
        ("]]", "],[]]"),
        ("]]}", r#"]],"spread":"1"}"#),
        ("]]}", r#"]],"spread":"x"}"#),
        ("]]}", r#"]],"spread":null}"#),
        (",null]", r#",{"px":"1","sz":"1","n":1}]"#),
        ("}]", r#","liquidation":null}]"#),
        ("}}", "},\"more\":1}"),
        (r#""fast":true"#, r#""fast":false"#),
        (r#""fast":true"#, r#""fast":1"#),
        (r#""fast":true"#, r#""fast":null"#),
        (r#""fast":true"#, r#""fast":true,"fast":true"#),
        ("}}", "}} "),
    ] {
        assert_ne!(from, to, "a change that changes nothing");
        if text.contains(from) {
            copies.push(text.replacen(from, to, 1));
            copies.push(text.replace(from, to));
        }
    }
    let bs = char::from(0x5c);
    let escaped = |c: char| format!("{bs}u{:04x}", u32::from(c));
    for (from, to) in [
        (r#""B""#.to_owned(), format!("\"{}\"", escaped('B'))),
        (r#""BTC""#.to_owned(), format!("\"B{}C\"", escaped('T'))),
        (r#""px":""#.to_owned(), format!("\"px\":\"{}", escaped('1'))),
    ] {
        assert_ne!(from, to, "a change that changes nothing");
        if text.contains(&from) {
            copies.push(text.replacen(&from, &to, 1));
        }
    }
    for copy in &copies {
        assert_ne!(copy, text, "a copy the same as the frame it was made from");
    }
    copies
}

#[test]
fn every_recorded_hot_frame_reads_here_as_serde_reads_it() {
    let frames = hot_frames();
    assert!(frames.len() >= 8, "only {} hot frames recorded", frames.len());
    for (name, text) in &frames {
        let direct = read(text).unwrap_or_else(|| panic!("{name} did not read directly"));
        assert_eq!(Ok(&direct), decode_in_one(text).as_ref().map_err(|_| ()), "{name}");
        assert_eq!(Ok(direct), decode_in_two(text), "{name}");
    }
}

#[test]
fn a_changed_frame_reads_here_only_as_serde_reads_it() {
    let (mut taken, mut declined) = (0, 0);
    for (name, text) in hot_frames() {
        for copy in changed(&text) {
            match read(&copy) {
                Some(direct) => {
                    taken += 1;
                    assert_eq!(Ok(direct), decode_in_two(&copy), "{name}: {copy}");
                }
                None => declined += 1,
            }
        }
    }
    assert!(taken > 0 && declined > 0, "taken {taken}, declined {declined}");
}

/// Every frame of the recorded mainnet corpora in `bench/data` reads here, as serde reads it:
/// `frames.jsonl.gz` (2,000 `l2Book` frames from 150 perps and 2,000 `trades` frames, the
/// benchmark's corpus) and `coverage.jsonl.gz` (500 `bbo` frames, 500 fast books and 500 books at
/// five figures with `spread`). The corpora are not part of the published crate, so plain
/// `cargo test` skips this; `just test` runs it.
#[test]
#[ignore = "reads bench/data; run by just test"]
#[cfg(not(target_arch = "wasm32"))]
fn every_frame_of_the_mainnet_corpus_reads_here_as_serde_reads_it() {
    for (name, expected) in [("frames", 4_000), ("coverage", 1_500)] {
        let path = format!("{}/../../bench/data/{name}.jsonl.gz", env!("CARGO_MANIFEST_DIR"));
        let file = std::fs::File::open(&path).unwrap_or_else(|error| panic!("{path}: {error}"));
        let corpus = std::io::read_to_string(flate2::read::GzDecoder::new(file)).unwrap();
        let mut frames = 0;
        for text in corpus.lines().filter(|line| !line.is_empty()) {
            let shown: String = text.chars().take(200).collect();
            let direct = read(text).unwrap_or_else(|| panic!("{name}: did not read directly: {shown}"));
            assert_eq!(Ok(direct), decode_in_two(text), "{name}: {shown}");
            frames += 1;
        }
        assert_eq!(frames, expected, "{name}");
    }
}
