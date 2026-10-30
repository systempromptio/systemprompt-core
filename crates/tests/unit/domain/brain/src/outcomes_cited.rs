use systemprompt_brain::outcomes::cited::{cited_facts, mentions};
use systemprompt_identifiers::BrainFactId;
use systemprompt_models::brain::FactRef;

fn fact(id: &str) -> FactRef {
    FactRef {
        id: BrainFactId::new(id),
        revision: 1,
    }
}

const A: &str = "m_0b8f0a52-55a1-4f3e-9e0e-0a0f6d1c2b3a";
const B: &str = "m_6c1d2e3f-4a5b-4c6d-8e7f-9a0b1c2d3e4f";

#[test]
fn whole_ids_are_found_between_punctuation_and_whitespace() {
    assert!(mentions(&format!("per {A}, the build uses nix"), A));
    assert!(mentions(&format!("[{A}]"), A));
    assert!(mentions(A, A));
    assert!(mentions(&format!("see ({A}).\n"), A));
}

#[test]
fn an_id_inside_a_longer_token_is_not_a_citation() {
    assert!(!mentions(&format!("{A}0"), A));
    assert!(!mentions(&format!("x{A}"), A));
    assert!(!mentions(&format!("{A}-extra"), A));
    assert!(!mentions(&format!("_{A}"), A));
    assert!(!mentions("m_0b8f0a52", A));
    assert!(!mentions("anything", ""));
}

#[test]
fn a_prefix_match_does_not_cite_the_longer_id_but_a_later_whole_match_does() {
    let text = format!("{A}x then {A}");
    assert!(mentions(&text, A));
}

#[test]
fn only_injected_facts_named_in_the_text_are_cited_once_each() {
    let injected = vec![fact(A), fact(B), fact(A)];
    let cited = cited_facts(&format!("{A} and again {A}"), &injected);
    assert_eq!(cited, vec![BrainFactId::new(A)]);

    let both = cited_facts(&format!("{B}; {A}"), &injected);
    assert_eq!(both, vec![BrainFactId::new(A), BrainFactId::new(B)]);

    assert!(cited_facts("no memory was used", &injected).is_empty());
}

#[test]
fn citation_is_case_sensitive_like_the_id() {
    assert!(!mentions(&A.to_uppercase(), A));
}
