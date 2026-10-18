//! A post's answer by the id it went out with: what `/info` or `/exchange` would have answered, or
//! what HTTP would have said.

use crate::client::{Accepted, ExchangeResponse, Status};

use super::*;
use crate::live::Frame;
use crate::live::fixtures::{dec, docs, level, mainnet, read, testnet};
use crate::markets::Book;

#[test]
fn post_answers_are_matched_by_id_and_keep_what_was_answered() {
    let Frame::Post(book) = read(&mainnet("post.info.l2Book")) else { panic!("an answer") };
    let Answer::Info { kind, data } = book.answer else { panic!("an info answer") };
    assert_eq!((book.id, kind.as_str()), (1, "l2Book"));
    let book: Book = serde_json::from_str(&data).unwrap();
    assert_eq!(
        (book.coin.as_str(), book.bids[0], book.spread),
        ("ETH", level("2410.2", "655.1567", 58), Some(dec("0.1")))
    );

    let Frame::Post(state) = read(&mainnet("post.info.clearinghouseState")) else { panic!("an answer") };
    let Answer::Info { kind, data } = state.answer else { panic!("an info answer") };
    assert_eq!((state.id, kind.as_str()), (12, "clearinghouseState"));
    let house: crate::account::ClearinghouseState = serde_json::from_str(&data).unwrap();
    assert_eq!(house.margin_summary.account_value, dec("96828.21505"));

    assert_eq!(
        read(&mainnet("post.info.null")),
        Frame::Post(Reply { id: 11, answer: Answer::Info { kind: "l2Book".to_owned(), data: "null".to_owned() } }),
        "an unknown coin's book is null"
    );
    assert_eq!(
        read(&testnet("post.action.err")),
        Frame::Post(Reply {
            id: 22,
            answer: Answer::Action(ExchangeResponse::Err(
                "User or API Wallet 0xf9414817e74c92ea33b3e8614efcc3ed198df423 does not exist.".to_owned()
            )),
        })
    );
    let resting = serde_json::json!({"resting": {"oid": 88383}});
    assert_eq!(
        read(&docs("post.action.ok")),
        Frame::Post(Reply {
            id: 256,
            answer: Answer::Action(ExchangeResponse::Ok(Accepted {
                kind: "order".to_owned(),
                statuses: vec![Status::Ok(resting.clone())],
                data: Some(serde_json::json!({"statuses": [resting]})),
            })),
        })
    );
    // The docs: an error is the text HTTP would have answered.
    assert_eq!(
        read(r#"{"channel":"post","data":{"id":5,"response":{"type":"error","payload":"429 Too Many Requests"}}}"#),
        Frame::Post(Reply { id: 5, answer: Answer::Error("429 Too Many Requests".to_owned()) })
    );
}
