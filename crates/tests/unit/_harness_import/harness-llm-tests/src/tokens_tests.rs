use harness_llm::Message;
use harness_llm::tokens::{estimate, estimate_message};

#[test]
fn four_characters_per_token_rounded_up() {
    assert_eq!(estimate(""), 0);
    assert_eq!(estimate("abcd"), 1);
    assert_eq!(estimate("abcde"), 2);
    assert_eq!(estimate("ééééé"), 2);
}

#[test]
fn messages_add_overhead() {
    let overhead = 4;
    assert_eq!(
        estimate_message(&Message::user_text("abcdefgh")),
        overhead + 2
    );
}
