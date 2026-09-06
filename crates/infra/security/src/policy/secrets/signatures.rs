//! Structural exemption for provider-signed reasoning blobs.
//!
//! Every reasoning-capable provider hands the client an opaque signed blob and
//! requires it back verbatim on the next turn: Gemini's `thoughtSignature`,
//! Anthropic's `signature` on a `thinking` block and `data` on a
//! `redacted_thinking` block, and `OpenAI`'s `encrypted_content` on a
//! `reasoning` item. All four are dense random-looking base64, so the
//! high-entropy backstop reads them as credentials — and under an enforcing
//! `secret_scan` stage that denies every multi-turn thinking continuation.
//!
//! The exemption is structural and narrow: it suppresses the entropy backstop
//! at those JSON paths only. Every vendor pattern still runs there, so a PEM
//! block or an AWS key smuggled into a `thoughtSignature` field is still
//! reported.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use super::super::governed::GovernedString;

// Why: these key names carry no meaning other than "provider-signed reasoning
// blob the client must echo back", so no sibling evidence is needed to
// recognise one.
const UNCONDITIONAL_KEYS: [&str; 2] = ["thoughtSignature", "thought_signature"];

// Why: these key names are generic enough to appear on unrelated objects, so
// the exemption is granted only when the enclosing object declares the
// reasoning content type that owns them.
const TYPED_KEYS: [(&str, &str); 3] = [
    ("signature", "thinking"),
    ("data", "redacted_thinking"),
    ("encrypted_content", "reasoning"),
];

const TYPE_KEY: &str = "type";

/// The `type` discriminator of every object in a governed input, keyed by that
/// object's path, so a generically named field can be tested against the block
/// it belongs to.
#[derive(Debug, Default)]
pub struct SignatureExemptions {
    types: HashMap<String, String>,
}

impl SignatureExemptions {
    #[must_use]
    pub fn from_strings(strings: &[GovernedString<'_>]) -> Self {
        let mut types = HashMap::new();
        for found in strings {
            if let Some((parent, key)) = split_path(&found.path)
                && key == TYPE_KEY
            {
                types.insert(parent.to_owned(), found.value.to_owned());
            }
        }
        Self { types }
    }

    // Why: the entropy backstop is the only detector suppressed here; callers
    // still run every vendor pattern against an exempted path.
    #[must_use]
    pub fn exempts_entropy(&self, path: &str) -> bool {
        let Some((parent, key)) = split_path(path) else {
            return false;
        };
        if UNCONDITIONAL_KEYS.contains(&key) {
            return true;
        }
        TYPED_KEYS.iter().any(|&(name, block_type)| {
            key == name && self.types.get(parent).is_some_and(|t| t == block_type)
        })
    }
}

fn split_path(path: &str) -> Option<(&str, &str)> {
    let (parent, key) = path.rsplit_once('.')?;
    (!key.is_empty() && !key.contains('[')).then_some((parent, key))
}
