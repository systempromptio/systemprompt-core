//! What a request served with its client's own credential carries upstream:
//! the client's headers, and none this platform names for itself.

use std::collections::BTreeSet;
use std::path::Path;

use systemprompt_identifiers::headers;
use systemprompt_wire::upstream::is_presented_request_header;

#[test]
fn every_platform_header_is_withheld() {
    for name in headers::ALL {
        if headers::CLIENT_TO_VENDOR.contains(name) {
            continue;
        }
        assert!(
            !is_presented_request_header(name),
            "{name} reached the upstream"
        );
        assert!(
            !is_presented_request_header(&name.to_ascii_uppercase()),
            "{name} in upper case reached the upstream"
        );
    }
}

#[test]
fn every_platform_namespace_is_withheld() {
    for prefix in headers::PREFIXES {
        let name = format!("{prefix}anything");
        assert!(!is_presented_request_header(&name), "{name}");
    }
}

#[test]
fn a_header_the_client_addresses_to_the_vendor_is_kept() {
    assert!(!headers::CLIENT_TO_VENDOR.is_empty());
    for name in headers::CLIENT_TO_VENDOR {
        assert!(
            headers::ALL.contains(name),
            "{name} is not a defined header"
        );
        assert!(is_presented_request_header(name), "{name}");
    }
    for name in [
        "user-agent",
        "accept",
        "anthropic-beta",
        "anthropic-version",
        "x-stainless-os",
        "x-app",
        "openai-organization",
    ] {
        assert!(is_presented_request_header(name), "{name}");
    }
}

#[test]
fn no_vendor_api_key_header_is_presented() {
    for name in [
        "x-api-key",
        "x-goog-api-key",
        "X-Goog-Api-Key",
        "authorization",
    ] {
        assert!(!is_presented_request_header(name), "{name}");
    }
}

#[test]
fn hop_framing_and_the_network_path_are_withheld() {
    for name in [
        "host",
        "content-length",
        "content-type",
        "transfer-encoding",
        "accept-encoding",
        "cookie",
        "x-forwarded-for",
        "x-real-ip",
        "fly-client-ip",
        "cf-connecting-ip",
        "traceparent",
        "tracestate",
        "baggage",
    ] {
        assert!(!is_presented_request_header(name), "{name}");
    }
}

fn declared_constants() -> Vec<(String, String)> {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../shared/identifiers/src/headers.rs");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{} must be readable: {e}", path.display()));
    source
        .lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("pub const ")?;
            let (name, value) = rest.split_once(": &str = \"")?;
            Some((name.to_owned(), value.strip_suffix("\";")?.to_owned()))
        })
        .collect()
}

#[test]
fn the_lists_name_every_constant_the_module_declares() {
    let declared = declared_constants();
    assert!(
        declared.len() >= 30,
        "the source scan found {} constants; it is reading the wrong file or shape",
        declared.len()
    );
    assert!(
        declared
            .iter()
            .any(|(name, value)| name == "TRACE_ID" && value == "x-trace-id"),
        "the scan reads names and values"
    );
    for (name, value) in &declared {
        assert!(
            headers::ALL.contains(&value.as_str()) || headers::PREFIXES.contains(&value.as_str()),
            "headers::{name} (\"{value}\") is in neither ALL nor PREFIXES"
        );
    }
    let unique: BTreeSet<&str> = headers::ALL.iter().copied().collect();
    assert_eq!(unique.len(), headers::ALL.len(), "ALL repeats a name");
    assert_eq!(
        headers::ALL.len() + headers::PREFIXES.len(),
        declared.len(),
        "the lists name something the module does not declare"
    );
    for value in headers::ALL {
        assert_eq!(
            *value,
            value.to_ascii_lowercase(),
            "{value} is not lower case"
        );
    }
}
