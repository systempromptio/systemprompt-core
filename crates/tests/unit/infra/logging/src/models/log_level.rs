//! Unit tests for LogLevel enum

use systemprompt_core_logging::LogLevel;

// ============================================================================
// LogLevel Display Tests
// ============================================================================

#[test]
fn test_log_level_display_error() {
    assert_eq!(LogLevel::Error.to_string(), "ERROR");
}

#[test]
fn test_log_level_display_warn() {
    assert_eq!(LogLevel::Warn.to_string(), "WARN");
}

#[test]
fn test_log_level_display_info() {
    assert_eq!(LogLevel::Info.to_string(), "INFO");
}

#[test]
fn test_log_level_display_debug() {
    assert_eq!(LogLevel::Debug.to_string(), "DEBUG");
}

#[test]
fn test_log_level_display_trace() {
    assert_eq!(LogLevel::Trace.to_string(), "TRACE");
}

// ============================================================================
// LogLevel as_str Tests
// ============================================================================

#[test]
fn test_log_level_as_str_error() {
    assert_eq!(LogLevel::Error.as_str(), "ERROR");
}

#[test]
fn test_log_level_as_str_warn() {
    assert_eq!(LogLevel::Warn.as_str(), "WARN");
}

#[test]
fn test_log_level_as_str_info() {
    assert_eq!(LogLevel::Info.as_str(), "INFO");
}

#[test]
fn test_log_level_as_str_debug() {
    assert_eq!(LogLevel::Debug.as_str(), "DEBUG");
}

#[test]
fn test_log_level_as_str_trace() {
    assert_eq!(LogLevel::Trace.as_str(), "TRACE");
}

// ============================================================================
// LogLevel FromStr Tests
// ============================================================================

#[test]
fn test_log_level_from_str_error() {
    assert_eq!("ERROR".parse::<LogLevel>().unwrap(), LogLevel::Error);
    assert_eq!("error".parse::<LogLevel>().unwrap(), LogLevel::Error);
    assert_eq!("Error".parse::<LogLevel>().unwrap(), LogLevel::Error);
}

#[test]
fn test_log_level_from_str_warn() {
    assert_eq!("WARN".parse::<LogLevel>().unwrap(), LogLevel::Warn);
    assert_eq!("warn".parse::<LogLevel>().unwrap(), LogLevel::Warn);
    assert_eq!("Warn".parse::<LogLevel>().unwrap(), LogLevel::Warn);
}

#[test]
fn test_log_level_from_str_info() {
    assert_eq!("INFO".parse::<LogLevel>().unwrap(), LogLevel::Info);
    assert_eq!("info".parse::<LogLevel>().unwrap(), LogLevel::Info);
    assert_eq!("Info".parse::<LogLevel>().unwrap(), LogLevel::Info);
}

#[test]
fn test_log_level_from_str_debug() {
    assert_eq!("DEBUG".parse::<LogLevel>().unwrap(), LogLevel::Debug);
    assert_eq!("debug".parse::<LogLevel>().unwrap(), LogLevel::Debug);
    assert_eq!("Debug".parse::<LogLevel>().unwrap(), LogLevel::Debug);
}

#[test]
fn test_log_level_from_str_trace() {
    assert_eq!("TRACE".parse::<LogLevel>().unwrap(), LogLevel::Trace);
    assert_eq!("trace".parse::<LogLevel>().unwrap(), LogLevel::Trace);
    assert_eq!("Trace".parse::<LogLevel>().unwrap(), LogLevel::Trace);
}

#[test]
fn test_log_level_from_str_invalid() {
    assert!("INVALID".parse::<LogLevel>().is_err());
    assert!("".parse::<LogLevel>().is_err());
    assert!("warning".parse::<LogLevel>().is_err());
    assert!("err".parse::<LogLevel>().is_err());
}

// ============================================================================
// LogLevel Equality and Clone Tests
// ============================================================================

#[test]
fn test_log_level_equality() {
    assert_eq!(LogLevel::Error, LogLevel::Error);
    assert_eq!(LogLevel::Warn, LogLevel::Warn);
    assert_eq!(LogLevel::Info, LogLevel::Info);
    assert_eq!(LogLevel::Debug, LogLevel::Debug);
    assert_eq!(LogLevel::Trace, LogLevel::Trace);
}

#[test]
fn test_log_level_inequality() {
    assert_ne!(LogLevel::Error, LogLevel::Warn);
    assert_ne!(LogLevel::Warn, LogLevel::Info);
    assert_ne!(LogLevel::Info, LogLevel::Debug);
    assert_ne!(LogLevel::Debug, LogLevel::Trace);
}

#[test]
fn test_log_level_clone() {
    let level = LogLevel::Error;
    let cloned = level.clone();
    assert_eq!(level, cloned);
}

#[test]
fn test_log_level_copy() {
    let level = LogLevel::Info;
    let copied = level;
    assert_eq!(level, copied);
}

// ============================================================================
// LogLevel Debug Tests
// ============================================================================

#[test]
fn test_log_level_debug() {
    assert!(format!("{:?}", LogLevel::Error).contains("Error"));
    assert!(format!("{:?}", LogLevel::Warn).contains("Warn"));
    assert!(format!("{:?}", LogLevel::Info).contains("Info"));
    assert!(format!("{:?}", LogLevel::Debug).contains("Debug"));
    assert!(format!("{:?}", LogLevel::Trace).contains("Trace"));
}

// ============================================================================
// LogLevel Serialization Tests
// ============================================================================

#[test]
fn test_log_level_serialize() {
    let level = LogLevel::Error;
    let json = serde_json::to_string(&level).unwrap();
    assert_eq!(json, "\"ERROR\"");
}

#[test]
fn test_log_level_serialize_all_variants() {
    assert_eq!(serde_json::to_string(&LogLevel::Error).unwrap(), "\"ERROR\"");
    assert_eq!(serde_json::to_string(&LogLevel::Warn).unwrap(), "\"WARN\"");
    assert_eq!(serde_json::to_string(&LogLevel::Info).unwrap(), "\"INFO\"");
    assert_eq!(serde_json::to_string(&LogLevel::Debug).unwrap(), "\"DEBUG\"");
    assert_eq!(serde_json::to_string(&LogLevel::Trace).unwrap(), "\"TRACE\"");
}

#[test]
fn test_log_level_deserialize() {
    let level: LogLevel = serde_json::from_str("\"ERROR\"").unwrap();
    assert_eq!(level, LogLevel::Error);
}

#[test]
fn test_log_level_deserialize_all_variants() {
    assert_eq!(
        serde_json::from_str::<LogLevel>("\"ERROR\"").unwrap(),
        LogLevel::Error
    );
    assert_eq!(
        serde_json::from_str::<LogLevel>("\"WARN\"").unwrap(),
        LogLevel::Warn
    );
    assert_eq!(
        serde_json::from_str::<LogLevel>("\"INFO\"").unwrap(),
        LogLevel::Info
    );
    assert_eq!(
        serde_json::from_str::<LogLevel>("\"DEBUG\"").unwrap(),
        LogLevel::Debug
    );
    assert_eq!(
        serde_json::from_str::<LogLevel>("\"TRACE\"").unwrap(),
        LogLevel::Trace
    );
}

#[test]
fn test_log_level_roundtrip() {
    for level in [
        LogLevel::Error,
        LogLevel::Warn,
        LogLevel::Info,
        LogLevel::Debug,
        LogLevel::Trace,
    ] {
        let json = serde_json::to_string(&level).unwrap();
        let parsed: LogLevel = serde_json::from_str(&json).unwrap();
        assert_eq!(level, parsed);
    }
}
