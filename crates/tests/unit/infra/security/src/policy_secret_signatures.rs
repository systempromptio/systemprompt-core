//! The secret scanner must not read a provider-signed reasoning blob as a
//! credential, and must still read a real credential parked in that field.

use systemprompt_security::policy::detect_secrets;
use systemprompt_security::policy::governed::GovernedInput;

const BLOB: &str = "PHL+ERIbxzlQOeiiRybQwgV7GvYmIclsJe1zsFIyuuM";

const PEM: &str =
    "-----BEGIN RSA PRIVATE KEY-----\nMIIEowIBAAKCAQEA\n-----END RSA PRIVATE KEY-----";

fn parts(pairs: &[(&str, &str)]) -> GovernedInput {
    GovernedInput::prompt_parts(
        pairs
            .iter()
            .map(|(path, value)| ((*path).to_owned(), (*value).to_owned())),
    )
}

#[test]
fn a_gemini_thought_signature_is_not_a_finding() {
    let input = parts(&[(
        "prompt.forwarded.$.contents[1].parts[0].thoughtSignature",
        BLOB,
    )]);
    assert!(
        detect_secrets(&input).is_none(),
        "a thoughtSignature the client must echo back is not a credential"
    );
}

#[test]
fn the_same_blob_in_user_text_is_still_a_finding() {
    let input = parts(&[("prompt.forwarded.$.contents[1].parts[0].text", BLOB)]);
    let hit = detect_secrets(&input).expect("the exemption is keyed on the field, not the value");
    assert_eq!(hit.pattern.id, "high-entropy-token");
}

#[test]
fn an_anthropic_thinking_signature_is_not_a_finding() {
    let input = parts(&[
        ("prompt.forwarded.$.messages[0].content[0].type", "thinking"),
        ("prompt.forwarded.$.messages[0].content[0].signature", BLOB),
    ]);
    assert!(
        detect_secrets(&input).is_none(),
        "a signature on a thinking block is a provider-signed blob"
    );
}

// Why: `signature` is a generic name, so the exemption is granted only when
// the enclosing block declares the reasoning type that owns it.
#[test]
fn a_signature_on_an_unrelated_block_is_still_a_finding() {
    let input = parts(&[
        ("prompt.forwarded.$.messages[0].content[0].type", "text"),
        ("prompt.forwarded.$.messages[0].content[0].signature", BLOB),
    ]);
    assert!(detect_secrets(&input).is_some());
}

#[test]
fn a_pem_key_inside_a_thought_signature_is_still_a_finding() {
    let input = parts(&[(
        "prompt.forwarded.$.contents[0].parts[0].thoughtSignature",
        PEM,
    )]);
    let hit = detect_secrets(&input).expect("vendor patterns still run on an exempted path");
    assert_ne!(hit.pattern.id, "high-entropy-token");
}
