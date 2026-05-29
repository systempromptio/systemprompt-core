//! Tests for context.rs types: ContextPropagationError and ConfigProvider defaults.

use systemprompt_traits::context::{ConfigProvider, ContextPropagationError, ContextPropagationResult};

// --- ContextPropagationError display ---

#[test]
fn missing_header_display_contains_name() {
    let e = ContextPropagationError::MissingHeader("x-tenant-id".to_owned());
    assert!(format!("{e}").contains("x-tenant-id"));
}

#[test]
fn invalid_header_display_contains_name_and_message() {
    let e = ContextPropagationError::InvalidHeader {
        name: "x-user-id".to_owned(),
        message: "not a valid UUID".to_owned(),
    };
    let s = format!("{e}");
    assert!(s.contains("x-user-id"));
    assert!(s.contains("not a valid UUID"));
}

#[test]
fn invalid_context_display_contains_detail() {
    let e = ContextPropagationError::Invalid("context expired".to_owned());
    assert!(format!("{e}").contains("context expired"));
}

#[test]
fn context_propagation_errors_are_debug() {
    let variants: &[ContextPropagationError] = &[
        ContextPropagationError::MissingHeader("h".into()),
        ContextPropagationError::InvalidHeader {
            name: "n".into(),
            message: "m".into(),
        },
        ContextPropagationError::Invalid("i".into()),
    ];
    for e in variants {
        let s = format!("{e:?}");
        assert!(!s.is_empty());
    }
}

#[test]
fn context_propagation_error_is_error_trait() {
    let e: Box<dyn std::error::Error> =
        Box::new(ContextPropagationError::MissingHeader("hdr".into()));
    assert!(e.to_string().contains("hdr"));
}

// --- ContextPropagationResult type alias ---

#[test]
fn result_alias_ok_carries_value() {
    let r: ContextPropagationResult<u32> = Ok(42);
    assert_eq!(r.unwrap(), 42);
}

#[test]
fn result_alias_err_carries_error() {
    let r: ContextPropagationResult<()> =
        Err(ContextPropagationError::Invalid("bad".into()));
    assert!(r.is_err());
}

// --- ConfigProvider: database_write_url default ---

struct MinimalConfig;

impl systemprompt_traits::context::ConfigProvider for MinimalConfig {
    fn get(&self, _key: &str) -> Option<String> {
        None
    }
    fn database_url(&self) -> &str {
        "postgres://localhost/mydb"
    }
    fn system_path(&self) -> &str {
        "/srv"
    }
    fn api_port(&self) -> u16 {
        8080
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[test]
fn config_provider_database_write_url_default_is_none() {
    let c = MinimalConfig;
    assert!(c.database_write_url().is_none());
}

#[test]
fn config_provider_required_methods_work() {
    let c = MinimalConfig;
    assert_eq!(c.database_url(), "postgres://localhost/mydb");
    assert_eq!(c.system_path(), "/srv");
    assert_eq!(c.api_port(), 8080);
    assert!(c.get("any").is_none());
}
