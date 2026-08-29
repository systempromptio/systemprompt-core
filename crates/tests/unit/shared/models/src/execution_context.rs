//! `AuthContext` and `RequestMetadata` — what survives being serialised
//! between service hops.
//!
//! These structs cross process boundaries, so a field silently dropped by
//! serde is a fact the next hop never learns. The delegation chain is the one
//! that matters: `act_chain` records who acted on whose behalf, and it is
//! skipped when empty — so the test that counts is whether a non-empty chain
//! survives, not whether an empty one is tidy.

use systemprompt_identifiers::{Actor, ClientId, JwtToken, SessionId, UserId};
use systemprompt_models::auth::UserType;
use systemprompt_models::execution::context::{AuthContext, RequestMetadata};

fn auth_context() -> AuthContext {
    AuthContext {
        auth_token: JwtToken::new("token"),
        actor: Actor::user(UserId::new("caller")),
        user_type: UserType::User,
        act_chain: Vec::new(),
        jti: String::new(),
        token_exp: 0,
    }
}

fn round_trip(ctx: &AuthContext) -> AuthContext {
    let json = serde_json::to_string(ctx).expect("serialise auth context");
    serde_json::from_str(&json).expect("deserialise auth context")
}

// Why: `act_chain` is the record of who acted on whose behalf. Dropped in
// transit, the next hop sees only the final actor and cannot tell a delegated
// call from a direct one.
#[test]
fn a_delegation_chain_survives_a_round_trip() {
    let mut ctx = auth_context();
    ctx.act_chain = vec![
        Actor::user(UserId::new("principal")),
        Actor::user(UserId::new("delegate")),
    ];

    let back = round_trip(&ctx);

    assert_eq!(
        back.act_chain, ctx.act_chain,
        "the delegation chain must reach the next hop intact"
    );
    assert_eq!(back.actor, ctx.actor);
}

// Why: the skip conditions are for wire tidiness, not for discarding data. An
// empty chain is genuinely absent; asserting it stays off the wire pins that
// the omission is the empty case only.
#[test]
fn the_optional_fields_are_omitted_only_when_they_carry_nothing() {
    let json = serde_json::to_value(auth_context()).expect("serialise");

    assert!(json.get("act_chain").is_none(), "an empty chain is omitted");
    assert!(json.get("jti").is_none(), "an empty jti is omitted");
    assert!(json.get("token_exp").is_none(), "a zero expiry is omitted");

    let mut populated = auth_context();
    populated.jti = "jti-1".to_owned();
    populated.token_exp = 1_800_000_000;
    let json = serde_json::to_value(&populated).expect("serialise");

    assert_eq!(json["jti"], "jti-1", "a set jti must reach the wire");
    assert_eq!(
        json["token_exp"], 1_800_000_000i64,
        "a real expiry must reach the wire"
    );
}

// Why: an omitted field must deserialise to its empty form rather than
// failing. A hop that rejects a context with no delegation chain rejects every
// ordinary direct call.
#[test]
fn a_context_without_the_optional_fields_still_deserialises() {
    let json = serde_json::json!({
        "auth_token": "token",
        "actor": Actor::user(UserId::new("caller")),
        "user_type": UserType::User,
    });

    let ctx: AuthContext = serde_json::from_value(json).expect("a minimal context must parse");

    assert!(ctx.act_chain.is_empty());
    assert!(ctx.jti.is_empty());
    assert_eq!(ctx.token_exp, 0);
}

// Why: the default is the untracked-session sentinel, and it is deliberately
// `is_tracked: true` — the degraded path sets it false explicitly. If the
// default were false, every ordinary request would look degraded and drop out
// of session analytics.
#[test]
fn default_request_metadata_is_tracked_with_the_unknown_session_sentinel() {
    let metadata = RequestMetadata::default();

    assert_eq!(metadata.session_id, SessionId::new("unknown".to_owned()));
    assert!(
        metadata.is_tracked,
        "the default must be tracked; only the degraded path opts out"
    );
    assert!(metadata.client_id.is_none());
    assert!(metadata.fingerprint_hash.is_none());
}

// Why: the surviving fields are the point here. `timestamp` cannot reach the
// wire at all — `Instant` does not implement `Serialize`, so the skip is
// enforced by the type rather than by the attribute, and asserting its absence
// cannot fail. What can regress is everything beside it, in particular
// `is_tracked`: a request explicitly marked untracked must stay untracked
// across the hop rather than reverting to the tracked default.
#[test]
fn an_untracked_request_stays_untracked_across_a_round_trip() {
    let metadata = RequestMetadata {
        session_id: SessionId::new("sess-1".to_owned()),
        client_id: Some(ClientId::new("client-1")),
        is_tracked: false,
        fingerprint_hash: Some("fp".to_owned()),
        ..RequestMetadata::default()
    };

    let json = serde_json::to_value(&metadata).expect("serialise metadata");
    assert!(json.get("timestamp").is_none());

    let back: RequestMetadata = serde_json::from_value(json).expect("deserialise metadata");

    assert_eq!(back.session_id, metadata.session_id);
    assert_eq!(back.client_id, metadata.client_id);
    assert!(
        !back.is_tracked,
        "an explicitly untracked request must stay untracked across the hop"
    );
    assert_eq!(back.fingerprint_hash, metadata.fingerprint_hash);
}
