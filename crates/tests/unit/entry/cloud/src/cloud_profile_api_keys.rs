//! Unit tests for `cloud::profile::ApiKeys` construction and provider choice.
//!
//! Only the non-interactive helpers are exercised — `collect_api_keys` drives
//! dialoguer prompts and is out of scope here.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::cloud::profile::ApiKeys;

#[test]
fn from_options_rejects_all_none() {
    let err = ApiKeys::from_options(None, None, None).unwrap_err();
    assert!(err.to_string().contains("At least one AI provider API key"));
}

#[test]
fn from_options_accepts_a_single_key() {
    let keys = ApiKeys::from_options(Some("g".to_owned()), None, None).expect("gemini only is ok");
    assert_eq!(keys.gemini.as_deref(), Some("g"));
    assert!(keys.anthropic.is_none());
    assert!(keys.openai.is_none());
}

#[test]
fn selected_provider_prefers_anthropic() {
    let keys = ApiKeys::from_options(
        Some("g".to_owned()),
        Some("a".to_owned()),
        Some("o".to_owned()),
    )
    .expect("multiple keys allowed");
    assert_eq!(keys.selected_provider(), "anthropic");
}

#[test]
fn selected_provider_falls_back_to_openai_then_gemini() {
    let openai = ApiKeys::from_options(None, None, Some("o".to_owned())).unwrap();
    assert_eq!(openai.selected_provider(), "openai");

    let gemini = ApiKeys::from_options(Some("g".to_owned()), None, None).unwrap();
    assert_eq!(gemini.selected_provider(), "gemini");
}
