//! Two-phase rotation: a staged key is verified and published before it
//! signs, so a verifier loaded between `stage` and `promote` accepts tokens
//! the promoted key signs, while one loaded before `stage` does not — which is
//! why `stage` must be rolled out to every replica before `promote`.

use chrono::{Duration, Utc};
use systemprompt_security::AuthError;
use systemprompt_security::keys::{RotationMode, SigningKeysError, promote, stage};

use crate::signing_key_rotation::{keys_after, mint, state_of, verify};

const OVERLAP: RotationMode = RotationMode::Overlap(Duration::hours(1));

#[test]
fn a_verifier_loaded_after_stage_accepts_a_token_signed_after_promote() {
    let old = systemprompt_test_fixtures::next_test_key();
    let (staged_key, staged) = stage(&state_of(&old, Vec::new())).expect("stage");
    let verifier = keys_after(old.clone(), &staged);

    let (promoted, _) = promote(&staged, OVERLAP, Utc::now()).expect("promote");
    assert_eq!(promoted.kid(), staged_key.kid());
    verify(&verifier, &mint(&promoted)).expect("staged key accepted before promote");
    verify(&verifier, &mint(&old)).expect("active key still accepted");
    let kids: Vec<String> = verifier.jwks().keys.into_iter().map(|k| k.kid).collect();
    assert_eq!(
        kids,
        vec![old.kid().to_owned(), staged_key.kid().to_owned()]
    );
}

#[test]
fn a_verifier_loaded_before_stage_rejects_a_token_signed_after_promote() {
    let old = systemprompt_test_fixtures::next_test_key();
    let before = state_of(&old, Vec::new());
    let verifier = keys_after(old.clone(), &before);

    let (_, staged) = stage(&before).expect("stage");
    let (promoted, _) = promote(&staged, OVERLAP, Utc::now()).expect("promote");
    let err = verify(&verifier, &mint(&promoted)).expect_err("unknown key accepted");
    assert!(matches!(err, AuthError::UnknownKid(_)), "{err}");
}

#[test]
fn after_promote_the_outgoing_key_still_verifies_and_signing_moved_to_the_staged_key() {
    let old = systemprompt_test_fixtures::next_test_key();
    let (_, staged) = stage(&state_of(&old, Vec::new())).expect("stage");
    let (promoted, after) = promote(&staged, OVERLAP, Utc::now()).expect("promote");
    assert!(after.staged_private_pem.is_none());
    let keys = keys_after(promoted, &after);
    assert_ne!(keys.active_kid(), old.kid());
    verify(&keys, &mint(&old)).expect("outgoing key verifies during the overlap");
}

#[test]
fn promote_with_nothing_staged_is_refused() {
    let old = systemprompt_test_fixtures::next_test_key();
    let err = promote(&state_of(&old, Vec::new()), OVERLAP, Utc::now())
        .expect_err("promoted without a staged key");
    assert!(matches!(err, SigningKeysError::NothingStaged), "{err}");
}

#[test]
fn staging_twice_is_refused_until_the_staged_key_is_promoted() {
    let old = systemprompt_test_fixtures::next_test_key();
    let (staged_key, staged) = stage(&state_of(&old, Vec::new())).expect("stage");
    let err = stage(&staged).expect_err("second stage accepted");
    assert!(
        matches!(&err, SigningKeysError::AlreadyStaged { kid } if kid == staged_key.kid()),
        "{err}"
    );
}
