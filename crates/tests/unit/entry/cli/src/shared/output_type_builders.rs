//! Tests for the small serialisable output builders.
//!
//! Each is a builder whose product is what a command serialises, so the
//! assertions are on the serialised shape rather than the struct.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::shared::{KeyValueOutput, SuccessOutput, TextOutput};

#[test]
fn a_text_output_carries_its_message() {
    let output = TextOutput::new("plain message");
    let json = serde_json::to_value(&output).unwrap();

    assert_eq!(json["message"], "plain message");
}

#[test]
fn a_success_output_omits_absent_details() {
    let bare = SuccessOutput::new("done");
    let json = serde_json::to_value(&bare).unwrap();

    assert_eq!(json["message"], "done");
    assert!(
        json.get("details").is_none(),
        "absent details are skipped in serialisation: {json}"
    );
}

#[test]
fn a_success_output_carries_the_details_it_is_given() {
    let detailed =
        SuccessOutput::new("done").with_details(vec!["first".to_owned(), "second".to_owned()]);
    let json = serde_json::to_value(&detailed).unwrap();

    assert_eq!(json["details"].as_array().unwrap().len(), 2);
    assert_eq!(json["details"][1], "second");
}

#[test]
fn key_value_pairs_accumulate_in_insertion_order() {
    let output = KeyValueOutput::new()
        .add("first", "1")
        .add("second", "2")
        .add("third", "3");

    let json = serde_json::to_value(&output).unwrap();
    let items = json["items"].as_array().unwrap();

    assert_eq!(items.len(), 3);
    assert_eq!(items[0]["key"], "first");
    assert_eq!(items[0]["value"], "1");
    assert_eq!(items[2]["key"], "third");
}

#[test]
fn an_empty_key_value_output_serialises_to_an_empty_list() {
    let json = serde_json::to_value(KeyValueOutput::new()).unwrap();

    assert!(json["items"].as_array().unwrap().is_empty());
}
