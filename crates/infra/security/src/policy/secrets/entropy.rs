//! The high-entropy backstop for credentials carrying no vendor prefix.
//!
//! A random base64 blob pasted into a prompt matches none of
//! [`super::SECRET_PATTERNS`] but still reads as machine-generated key
//! material. Randomness alone cannot say so: a serialised protobuf, a base64
//! JSON envelope, and a 32-byte key are all dense mixed-case base64 of similar
//! measured entropy. [`is_structured_payload`] supplies the missing
//! discriminator — key material decodes to bytes with no readable structure,
//! whereas a tool result decodes to text or to a self-consistent wire format.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use base64::Engine as _;
use base64::engine::general_purpose::{STANDARD_NO_PAD, URL_SAFE_NO_PAD};
use regex::Regex;

pub const DEFAULT_MIN_LEN: usize = 32;

pub const DEFAULT_THRESHOLD: f64 = 0.80;

const TOKEN_DELIMITERS: &str = "\"'`()[]{}<>,;:";
const TOKEN_CHARSET_EXTRA: &str = "+/=_-";
const ENTROPY_CEILING_SYMBOLS: usize = 64;

const MIN_STRUCTURED_LEN: usize = 16;
const MAX_PAYLOAD_PREFIX_LEN: usize = 10;
const DIGEST_LENGTHS: [(&str, usize); 3] = [("sha256", 32), ("sha384", 48), ("sha512", 64)];
const MAX_FIELD_NUMBER: u64 = 64;
const MAX_NESTING_DEPTH: u32 = 4;
const MIN_NESTED_PAYLOAD_LEN: usize = 4;
const TEXT_RATIO_NUMERATOR: usize = 9;
const TEXT_RATIO_DENOMINATOR: usize = 10;

/// Tunables for the heuristic, read from the `secret_scan` policy's `entropy`
/// block. [`Default`] reproduces the built-in behaviour, which is what every
/// caller outside the policy chain gets.
#[derive(Debug, Clone)]
pub struct EntropyConfig {
    pub enabled: bool,
    pub min_len: usize,
    pub threshold: f64,
    pub allowlist: Vec<Regex>,
}

impl Default for EntropyConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_len: DEFAULT_MIN_LEN,
            threshold: DEFAULT_THRESHOLD,
            allowlist: Vec::new(),
        }
    }
}

#[must_use]
pub fn find_high_entropy_token<'a>(text: &'a str, config: &EntropyConfig) -> Option<&'a str> {
    if !config.enabled {
        return None;
    }
    text.split(|c: char| c.is_whitespace() || TOKEN_DELIMITERS.contains(c))
        .find(|token| is_credential_shaped(token, config))
}

fn is_credential_shaped(token: &str, config: &EntropyConfig) -> bool {
    token.len() >= config.min_len
        && token
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || TOKEN_CHARSET_EXTRA.contains(c))
        && token.chars().any(|c| c.is_ascii_uppercase())
        && token.chars().any(|c| c.is_ascii_lowercase())
        && token.chars().any(|c| c.is_ascii_digit())
        && entropy_ratio(token) >= config.threshold
        && !config.allowlist.iter().any(|re| re.is_match(token))
        && !is_verified_digest(token)
        && !is_structured_payload(token)
}

// Why: an SRI hash (`sha384-<base64>`) is public integrity metadata, not key
// material, but its payload is dense base64 that clears every entropy check.
// The exoneration is length-verified rather than prefix-trusted: a credential
// smuggled behind a `sha384-` prefix decodes to the wrong byte count and is
// still reported.
fn is_verified_digest(token: &str) -> bool {
    let Some((prefix, payload)) = token.split_once('-') else {
        return false;
    };
    DIGEST_LENGTHS
        .iter()
        .find(|(algo, _)| algo.eq_ignore_ascii_case(prefix))
        .is_some_and(|&(_, digest_len)| {
            decode_base64(payload).is_some_and(|bytes| bytes.len() == digest_len)
        })
}

fn shannon_entropy(s: &str) -> f64 {
    let mut counts = [0u32; 256];
    let bytes = s.as_bytes();
    for &b in bytes {
        counts[usize::from(b)] += 1;
    }
    let len = bytes.len() as f64;
    counts
        .iter()
        .filter(|&&c| c > 0)
        .map(|&c| {
            let p = f64::from(c) / len;
            -p * p.log2()
        })
        .sum()
}

fn entropy_ratio(s: &str) -> f64 {
    let symbols = s.len().min(ENTROPY_CEILING_SYMBOLS);
    let ceiling = (symbols as f64).log2();
    if ceiling <= 0.0 {
        return 0.0;
    }
    shannon_entropy(s) / ceiling
}

fn is_structured_payload(token: &str) -> bool {
    decoded_payload(token).is_some_and(|bytes| {
        bytes.len() >= MIN_STRUCTURED_LEN && (is_mostly_text(&bytes) || is_protobuf(&bytes, 0))
    })
}

// Why: a `name-<base64>` token never decodes as a whole — the prefix is not
// base64 — which used to defeat the structured-payload discriminator for
// exactly the prefixed-payload shapes it exists to exonerate. A short
// alphanumeric prefix is stripped and the remainder given the same chance.
fn decoded_payload(token: &str) -> Option<Vec<u8>> {
    decode_base64(token).or_else(|| {
        let (prefix, payload) = token.split_once('-')?;
        let plausible_prefix = prefix.len() <= MAX_PAYLOAD_PREFIX_LEN
            && prefix.chars().all(|c| c.is_ascii_alphanumeric());
        plausible_prefix.then(|| decode_base64(payload)).flatten()
    })
}

fn decode_base64(token: &str) -> Option<Vec<u8>> {
    let body = token.trim_end_matches('=');
    STANDARD_NO_PAD
        .decode(body)
        .or_else(|_| URL_SAFE_NO_PAD.decode(body))
        .ok()
}

fn is_mostly_text(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return false;
    }
    let printable = bytes
        .iter()
        .filter(|&&b| matches!(b, b'\t' | b'\n' | b'\r') || (0x20..0x7f).contains(&b))
        .count();
    printable * TEXT_RATIO_DENOMINATOR >= bytes.len() * TEXT_RATIO_NUMERATOR
}

// Why: exact buffer consumption alone is a weak signal on a short random blob,
// so a decode only counts as protobuf when it also carries at least two fields
// and one length-delimited payload that is itself text or protobuf. Random key
// material clears all three by accident far less than one time in a hundred; a
// real serialised message clears them by construction.
fn is_protobuf(bytes: &[u8], depth: u32) -> bool {
    if depth > MAX_NESTING_DEPTH || bytes.len() < MIN_STRUCTURED_LEN {
        return false;
    }
    parse_protobuf(bytes, depth).is_some_and(|parsed| parsed.fields >= 2 && parsed.nested_structure)
}

struct ParsedMessage {
    fields: usize,
    nested_structure: bool,
}

fn parse_protobuf(bytes: &[u8], depth: u32) -> Option<ParsedMessage> {
    let mut cursor = 0usize;
    let mut parsed = ParsedMessage {
        fields: 0,
        nested_structure: false,
    };
    while cursor < bytes.len() {
        let (tag, after_tag) = read_varint(bytes, cursor)?;
        let field_number = tag >> 3;
        if field_number == 0 || field_number > MAX_FIELD_NUMBER {
            return None;
        }
        cursor = match tag & 7 {
            0 => read_varint(bytes, after_tag)?.1,
            1 => advance(bytes, after_tag, 8)?,
            5 => advance(bytes, after_tag, 4)?,
            2 => {
                let (len, after_len) = read_varint(bytes, after_tag)?;
                let len = usize::try_from(len).ok()?;
                let end = advance(bytes, after_len, len)?;
                let payload = bytes.get(after_len..end)?;
                if payload.len() >= MIN_NESTED_PAYLOAD_LEN
                    && (is_mostly_text(payload) || is_protobuf(payload, depth + 1))
                {
                    parsed.nested_structure = true;
                }
                end
            },
            _ => return None,
        };
        parsed.fields += 1;
    }
    Some(parsed)
}

fn advance(bytes: &[u8], cursor: usize, by: usize) -> Option<usize> {
    let end = cursor.checked_add(by)?;
    (end <= bytes.len()).then_some(end)
}

fn read_varint(bytes: &[u8], start: usize) -> Option<(u64, usize)> {
    let mut value = 0u64;
    let mut shift = 0u32;
    let mut cursor = start;
    loop {
        let byte = *bytes.get(cursor)?;
        cursor += 1;
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Some((value, cursor));
        }
        shift += 7;
        if shift >= 64 {
            return None;
        }
    }
}
