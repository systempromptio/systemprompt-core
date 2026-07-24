//! Pre-generated RSA signing keys for tests.
//!
//! Generating an RSA-2048 key costs ~2.2s in an unoptimised test build, and
//! nextest runs every test in its own process, so a `OnceLock` around
//! `RsaSigningKey::generate()` caches nothing across a run — each test pays the
//! full cost. Loading a committed PKCS#8 PEM instead costs under a millisecond.
//!
//! Several distinct keys are provided because some suites (JWKS kid rotation,
//! LRU eviction, RS256 cutover, foreign-issuer rejection) assert on behaviour
//! that only manifests across *different* keys. [`next_test_key`] hands out the
//! rotating keys so a suite can take as many distinct ones as it needs.
//!
//! Index 0 is reserved for the process-wide token authority
//! ([`crate::jwt::install_test_signing_key`]) and is never returned by
//! [`next_test_key`]: a test that installs the authority and then mints a token
//! from a deliberately foreign key must not be handed the authority's own key
//! as the "foreign" one, or the rejection it asserts on would silently become a
//! success.
//!
//! These keys are test fixtures. They are committed deliberately, are reachable
//! only from the test workspace, and must never be used by production code.

use std::sync::atomic::{AtomicUsize, Ordering};

use systemprompt_security::keys::RsaSigningKey;

const TEST_KEY_PEMS: [&str; 6] = [
    include_str!("../keys/test_signing_key_0.pem"),
    include_str!("../keys/test_signing_key_1.pem"),
    include_str!("../keys/test_signing_key_2.pem"),
    include_str!("../keys/test_signing_key_3.pem"),
    include_str!("../keys/test_signing_key_4.pem"),
    include_str!("../keys/test_signing_key_5.pem"),
];

/// Index of the key backing the process-wide token authority.
pub const AUTHORITY_KEY_INDEX: usize = 0;

/// How many distinct keys [`next_test_key`] cycles through before repeating.
pub const ROTATING_KEY_COUNT: usize = TEST_KEY_PEMS.len() - 1;

static NEXT_KEY: AtomicUsize = AtomicUsize::new(0);

/// Load test key `index`, wrapping modulo the number of committed keys.
pub fn test_key(index: usize) -> RsaSigningKey {
    RsaSigningKey::from_pkcs8_pem(TEST_KEY_PEMS[index % TEST_KEY_PEMS.len()])
        .expect("committed test signing key is valid PKCS#8")
}

/// Take the next rotating key, skipping the authority key.
///
/// Callers that need several keys to be distinct from each other must take them
/// all from this function and must not take more than [`ROTATING_KEY_COUNT`]
/// within one assertion.
pub fn next_test_key() -> RsaSigningKey {
    let n = NEXT_KEY.fetch_add(1, Ordering::Relaxed);
    test_key(AUTHORITY_KEY_INDEX + 1 + (n % ROTATING_KEY_COUNT))
}
