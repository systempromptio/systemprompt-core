//! Synonym expansion at index time: a group contributes every member when
//! any member occurs in the text as whole words, case-insensitively; a
//! document with a short or blank group is refused.

use systemprompt_models::brain::{SynonymsDoc, SynonymsError};

fn doc(groups: &[&[&str]]) -> SynonymsDoc {
    SynonymsDoc {
        groups: groups
            .iter()
            .map(|group| group.iter().map(|m| (*m).to_owned()).collect())
            .collect(),
    }
}

#[test]
fn a_present_member_brings_in_its_whole_group_once() {
    let synonyms = doc(&[
        &["invoice", "bill", "statement of account"],
        &["client", "customer"],
        &["bill", "invoice"],
    ]);
    assert_eq!(
        synonyms.expansions("Acme sends the BILL monthly."),
        vec![
            "invoice".to_owned(),
            "bill".to_owned(),
            "statement of account".to_owned()
        ],
        "case does not matter and a member seen in one group is not repeated"
    );
    assert_eq!(
        synonyms.expansion_text("The statement of account for a customer"),
        "invoice bill statement of account client customer"
    );
}

#[test]
fn matching_is_on_whole_words_and_phrases() {
    let synonyms = doc(&[&["bill", "invoice"], &["statement of account", "invoice"]]);
    assert!(synonyms.expansions("billing runs nightly").is_empty());
    assert!(synonyms.expansions("the account statement").is_empty());
    assert!(synonyms.expansions("").is_empty());
    assert_eq!(
        synonyms.expansions("account: statement-of-account"),
        vec!["statement of account".to_owned(), "invoice".to_owned()]
    );
    assert!(SynonymsDoc::default().expansions("bill").is_empty());
}

#[test]
fn a_short_or_blank_group_is_refused() {
    assert_eq!(
        doc(&[&["only"]]).validate(),
        Err(SynonymsError::ShortGroup(0))
    );
    assert_eq!(
        doc(&[&["a", "b"], &["c", " "]]).validate(),
        Err(SynonymsError::EmptyMember(1))
    );
    assert_eq!(doc(&[&["a", "b"]]).validate(), Ok(()));
    let parsed: SynonymsDoc =
        serde_yaml::from_str("groups:\n  - [invoice, bill]\n").expect("parses");
    assert_eq!(parsed, doc(&[&["invoice", "bill"]]));
    assert!(serde_yaml::from_str::<SynonymsDoc>("group: []\n").is_err());
}
