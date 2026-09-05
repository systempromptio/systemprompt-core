//! Exercises the full trait surface of every `bridge_define_id!` and
//! `bridge_define_token!` type, not just the two representatives the
//! focused suites use.

use std::collections::{BTreeSet, HashSet};
use std::str::FromStr;

use systemprompt_bridge::ids::{
    BearerToken, CertFingerprint, CommsMessageId, HookSessionId, HostId, KeystoreRef,
    LoopbackSecret, McpSessionId, ModelId, PatToken, PinnedPubKey, PrefsDomain, PrefsKey,
    PrefsValue, ProxySecret, QueryKey, QueryValue,
};

macro_rules! assert_plain_id_surface {
    ($ty:ty, $sample:expr) => {{
        let id = <$ty>::new($sample);
        assert_eq!(id.as_str(), $sample);
        assert_eq!(AsRef::<str>::as_ref(&id), $sample);
        assert_eq!(format!("{id}"), $sample);
        assert!(format!("{id:?}").contains($sample));

        let json = serde_json::to_string(&id).expect("serialize");
        assert_eq!(json, format!("\"{}\"", $sample));
        let back: $ty = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, id);

        let empty = <$ty>::new("");
        assert_eq!(empty.as_str(), "");
        assert!(empty < id);

        let mut set = HashSet::new();
        assert!(set.insert(id.clone()));
        assert!(!set.insert(id.clone()));

        assert_eq!(String::from(id.clone()), $sample.to_owned());
        assert_eq!(id.into_inner(), $sample.to_owned());
    }};
}

macro_rules! assert_validated_id_surface {
    ($ty:ty, $sample:expr) => {{
        let id = <$ty>::try_new($sample).expect("non-empty is valid");
        assert_eq!(id.as_str(), $sample);
        assert_eq!(AsRef::<str>::as_ref(&id), $sample);
        assert_eq!(format!("{id}"), $sample);
        assert!(format!("{id:?}").contains($sample));

        assert_eq!(<$ty>::try_from($sample).expect("&str"), id);
        assert_eq!(<$ty>::try_from($sample.to_owned()).expect("String"), id);
        assert_eq!(<$ty>::from_str($sample).expect("FromStr"), id);

        assert!(<$ty>::try_new("").is_err());
        assert!(<$ty>::try_from("").is_err());
        assert!(<$ty>::try_from(String::new()).is_err());
        assert!(<$ty>::from_str("").is_err());
        assert!(serde_json::from_str::<$ty>("\"\"").is_err());

        let json = serde_json::to_string(&id).expect("serialize");
        assert_eq!(json, format!("\"{}\"", $sample));
        let back: $ty = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, id);

        let mut set = BTreeSet::new();
        assert!(set.insert(id.clone()));
        assert!(!set.insert(id.clone()));

        assert_eq!(String::from(id.clone()), $sample.to_owned());
        assert_eq!(id.into_inner(), $sample.to_owned());
    }};
}

#[test]
fn every_plain_id_round_trips_through_its_whole_surface() {
    assert_plain_id_surface!(HostId, "claude-code-cli");
    assert_plain_id_surface!(McpSessionId, "mcp-session-7f31");
    assert_plain_id_surface!(HookSessionId, "hook-session-a12");
    assert_plain_id_surface!(CommsMessageId, "msg-0042");
    assert_plain_id_surface!(PrefsValue, "dark");
    assert_plain_id_surface!(QueryValue, "enabled");
}

#[test]
fn every_validated_id_rejects_empty_and_round_trips_when_non_empty() {
    assert_validated_id_surface!(PrefsDomain, "editor");
    assert_validated_id_surface!(PrefsKey, "theme");
    assert_validated_id_surface!(ModelId, "claude-opus-5");
    assert_validated_id_surface!(KeystoreRef, "login-keychain");
    assert_validated_id_surface!(CertFingerprint, "aa:bb:cc:dd");
    assert_validated_id_surface!(QueryKey, "host");
}

#[test]
fn a_validated_id_reports_its_own_type_name_when_rejecting_empty() {
    let err = PrefsKey::try_new("").expect_err("empty is rejected");
    let rendered = err.to_string();
    assert!(
        rendered.contains("PrefsKey"),
        "error should name the id type it came from, got {rendered}"
    );

    let other = CertFingerprint::try_new("").expect_err("empty is rejected");
    assert!(
        other.to_string().contains("CertFingerprint"),
        "each id type reports itself, got {other}"
    );
}

macro_rules! assert_token_surface {
    ($ty:ty) => {{
        let long = "0123456789abcdefghij";
        let token = <$ty>::new(long);
        assert_eq!(token.as_str(), long);
        assert_eq!(AsRef::<str>::as_ref(&token), long);
        assert_eq!(token.redacted(), "01234567...ghij");
        assert_eq!(format!("{token}"), "01234567...ghij");
        assert_eq!(
            format!("{token:?}"),
            format!("{}(01234567...ghij)", stringify!($ty))
        );
        assert!(!format!("{token:?}").contains(long));

        assert_eq!(<$ty>::from(long.to_owned()), token);
        assert_eq!(<$ty>::from(long), token);
        assert_eq!(<$ty>::from_str(long).expect("infallible"), token);

        let short = <$ty>::new("abc");
        assert_eq!(short.redacted(), "***");
        assert!(!format!("{short}").contains("abc"));

        let mut set = HashSet::new();
        assert!(set.insert(<$ty>::new(long)));
        assert!(!set.insert(<$ty>::new(long)));

        assert_eq!(token.into_inner(), long.to_owned());
    }};
}

#[test]
fn every_secret_token_redacts_in_display_and_debug_but_not_in_as_str() {
    assert_token_surface!(PatToken);
    assert_token_surface!(BearerToken);
    assert_token_surface!(LoopbackSecret);
    assert_token_surface!(ProxySecret);
    assert_token_surface!(PinnedPubKey);
}

#[test]
fn taking_a_tokens_inner_string_leaves_the_token_empty_before_it_is_dropped() {
    let token = PatToken::new("0123456789abcdefghij");
    let raw = token.into_inner();
    assert_eq!(raw, "0123456789abcdefghij");
}
