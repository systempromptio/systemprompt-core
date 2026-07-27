//! Normalising the token-exchange grant's opaque `anyhow::Error` back into the
//! endpoint's `TokenError` wire type.
//!
//! The exchange path builds its failures as `anyhow!(TokenError::…)`, so the
//! RFC 6749 error code the client sees depends entirely on `map_exchange_error`
//! recovering the original variant by downcast. Losing a variant silently
//! downgrades a client-fixable `invalid_request` into a `server_error`, so
//! every variant is round-tripped here.

use systemprompt_api::routes::oauth::OAuthHttpError;
use systemprompt_api::routes::oauth::endpoints::token::TokenError;
use systemprompt_api::routes::oauth::endpoints::token::handler_test_api::map_exchange_error;

fn round_trip(error: TokenError) -> TokenError {
    map_exchange_error(&anyhow::Error::new(error))
}

#[test]
fn an_invalid_request_keeps_its_field_and_message() {
    let mapped = round_trip(TokenError::InvalidRequest {
        field: "subject_token".to_owned(),
        message: "is required".to_owned(),
    });

    match mapped {
        TokenError::InvalidRequest { field, message } => {
            assert_eq!(field, "subject_token");
            assert_eq!(message, "is required");
        },
        other => panic!("expected InvalidRequest, got {other:?}"),
    }
}

#[test]
fn an_invalid_target_survives_as_invalid_target_not_server_error() {
    let mapped = round_trip(TokenError::InvalidTarget {
        message: "'https://evil.test' not in allowed_resource_audiences".to_owned(),
    });

    match mapped {
        TokenError::InvalidTarget { message } => {
            assert!(message.contains("https://evil.test"), "{message}");
        },
        other => panic!("expected InvalidTarget, got {other:?}"),
    }
}

#[test]
fn each_unit_variant_survives_the_downcast_intact() {
    assert!(matches!(
        round_trip(TokenError::InvalidClient),
        TokenError::InvalidClient
    ));
    assert!(matches!(
        round_trip(TokenError::InvalidCredentials),
        TokenError::InvalidCredentials
    ));
    assert!(matches!(
        round_trip(TokenError::InvalidClientSecret),
        TokenError::InvalidClientSecret
    ));
    assert!(matches!(
        round_trip(TokenError::ExpiredCode),
        TokenError::ExpiredCode
    ));
}

#[test]
fn a_recovered_variant_reaches_the_client_as_its_own_oauth_error_code() {
    let response = OAuthHttpError::from(round_trip(TokenError::InvalidTarget {
        message: "unknown resource".to_owned(),
    }));

    let direct = OAuthHttpError::from(TokenError::InvalidTarget {
        message: "unknown resource".to_owned(),
    });

    assert_eq!(
        format!("{response:?}"),
        format!("{direct:?}"),
        "a downcast variant must map to the same HTTP error as the variant itself"
    );
}

#[test]
fn the_grant_and_scope_variants_keep_their_reason_text() {
    match round_trip(TokenError::InvalidGrant {
        reason: "ID-JAG has already been used (replay)".to_owned(),
    }) {
        TokenError::InvalidGrant { reason } => assert!(reason.contains("replay"), "{reason}"),
        other => panic!("expected InvalidGrant, got {other:?}"),
    }

    match round_trip(TokenError::InvalidScope {
        message: "no overlap".to_owned(),
    }) {
        TokenError::InvalidScope { message } => assert_eq!(message, "no overlap"),
        other => panic!("expected InvalidScope, got {other:?}"),
    }

    match round_trip(TokenError::InvalidRefreshToken {
        reason: "rotated".to_owned(),
    }) {
        TokenError::InvalidRefreshToken { reason } => assert_eq!(reason, "rotated"),
        other => panic!("expected InvalidRefreshToken, got {other:?}"),
    }

    match round_trip(TokenError::UnsupportedGrantType {
        grant_type: "password".to_owned(),
    }) {
        TokenError::UnsupportedGrantType { grant_type } => assert_eq!(grant_type, "password"),
        other => panic!("expected UnsupportedGrantType, got {other:?}"),
    }
}

#[test]
fn a_server_error_variant_keeps_its_own_message_rather_than_the_anyhow_display() {
    match round_trip(TokenError::ServerError {
        message: "signing key unavailable".to_owned(),
    }) {
        TokenError::ServerError { message } => assert_eq!(message, "signing key unavailable"),
        other => panic!("expected ServerError, got {other:?}"),
    }
}

#[test]
fn a_foreign_error_becomes_a_server_error_carrying_its_display() {
    match map_exchange_error(&anyhow::anyhow!("client owner is not active")) {
        TokenError::ServerError { message } => {
            assert_eq!(message, "client owner is not active");
        },
        other => panic!("expected ServerError, got {other:?}"),
    }
}

#[test]
fn a_context_wrapped_token_error_is_still_recovered_by_downcast() {
    let wrapped = anyhow::Error::new(TokenError::InvalidGrant {
        reason: "code expired".to_owned(),
    })
    .context("while redeeming the authorization code");

    match map_exchange_error(&wrapped) {
        TokenError::InvalidGrant { reason } => assert_eq!(reason, "code expired"),
        other => panic!("expected InvalidGrant, got {other:?}"),
    }
}
