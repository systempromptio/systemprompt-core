//! `AuthContext` and `RequestMetadata` — what survives being serialised
//! between service hops.
//!
//! These structs cross process boundaries, so a field silently dropped by
//! serde is a fact the next hop never learns. The delegation chain is the one
//! that matters: `act_chain` records who acted on whose behalf, and it is
//! skipped when empty — so the test that counts is whether a non-empty chain
//! survives, not whether an empty one is tidy.

use systemprompt_identifiers::{AccessTokenId, Actor, ClientId, JwtToken, SessionId, UserId};
use systemprompt_models::auth::UserType;
use systemprompt_models::execution::context::{AuthContext, RequestMetadata};

const CALLER: &str = "00000000-0000-4000-8000-0000000000c1";
const PRINCIPAL: &str = "00000000-0000-4000-8000-0000000000a1";
const DELEGATE: &str = "00000000-0000-4000-8000-0000000000d1";

fn auth_context() -> AuthContext {
    AuthContext {
        auth_token: Some(JwtToken::new("token")),
        actor: Actor::user(UserId::new(CALLER)),
        user_type: UserType::User,
        act_chain: Vec::new(),
        jti: None,
        token_exp: None,
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
        Actor::user(UserId::new(PRINCIPAL)),
        Actor::user(UserId::new(DELEGATE)),
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
    assert!(json.get("jti").is_none(), "an absent jti is omitted");
    assert!(
        json.get("token_exp").is_none(),
        "an absent expiry is omitted"
    );

    let mut populated = auth_context();
    populated.jti = Some(AccessTokenId::new("jti-1"));
    populated.token_exp = Some(1_800_000_000);
    let json = serde_json::to_value(&populated).expect("serialise");

    assert_eq!(json["jti"], "jti-1", "a set jti must reach the wire");
    assert_eq!(
        json["token_exp"], 1_800_000_000i64,
        "a real expiry must reach the wire"
    );
}

// Why: an omitted field must deserialise to its absent form rather than
// failing. A hop that rejects a context with no delegation chain rejects every
// ordinary direct call.
#[test]
fn a_context_without_the_optional_fields_still_deserialises() {
    let json = serde_json::json!({
        "actor": Actor::user(UserId::new(CALLER)),
        "user_type": UserType::User,
    });

    let ctx: AuthContext = serde_json::from_value(json).expect("a minimal context must parse");

    assert!(ctx.act_chain.is_empty());
    assert!(ctx.auth_token.is_none());
    assert!(ctx.jti.is_none());
    assert!(ctx.token_exp.is_none());
}

#[test]
fn a_context_whose_actor_is_not_a_uuid_is_rejected() {
    let mut actor = serde_json::to_value(Actor::user(UserId::new(CALLER))).expect("serialise");
    actor["user_id"] = serde_json::json!("unset");
    let json = serde_json::json!({
        "actor": actor,
        "user_type": UserType::User,
    });

    assert!(serde_json::from_value::<AuthContext>(json).is_err());
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
        timestamp: std::time::Instant::now(),
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
