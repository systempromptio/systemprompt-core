//! Unit tests for LogLevel enum

use systemprompt_logging::LogLevel;

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
    "INVALID".parse::<LogLevel>().unwrap_err();
    "".parse::<LogLevel>().unwrap_err();
    "warning".parse::<LogLevel>().unwrap_err();
    "err".parse::<LogLevel>().unwrap_err();
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
    assert_ne!(LogLevel::Info, LogLevel::Debug);
    assert_ne!(LogLevel::Trace, LogLevel::Error);
}

#[test]
fn test_log_level_copy() {
    let level = LogLevel::Error;
    let copied = level;
    assert_eq!(level, copied);
}

#[test]
fn test_log_level_clone() {
    let level = LogLevel::Warn;
    let cloned = level.clone();
    assert_eq!(level, cloned);
}

// ============================================================================
// LogLevel Serialization Tests
// ============================================================================

#[test]
fn test_log_level_serialize_error() {
    let json = serde_json::to_string(&LogLevel::Error).unwrap();
    assert_eq!(json, "\"ERROR\"");
}

#[test]
fn test_log_level_serialize_warn() {
    let json = serde_json::to_string(&LogLevel::Warn).unwrap();
    assert_eq!(json, "\"WARN\"");
}

#[test]
fn test_log_level_serialize_info() {
    let json = serde_json::to_string(&LogLevel::Info).unwrap();
    assert_eq!(json, "\"INFO\"");
}

#[test]
fn test_log_level_serialize_debug() {
    let json = serde_json::to_string(&LogLevel::Debug).unwrap();
    assert_eq!(json, "\"DEBUG\"");
}

#[test]
fn test_log_level_serialize_trace() {
    let json = serde_json::to_string(&LogLevel::Trace).unwrap();
    assert_eq!(json, "\"TRACE\"");
}

#[test]
fn test_log_level_deserialize_all() {
    assert_eq!(serde_json::from_str::<LogLevel>("\"ERROR\"").unwrap(), LogLevel::Error);
    assert_eq!(serde_json::from_str::<LogLevel>("\"WARN\"").unwrap(), LogLevel::Warn);
    assert_eq!(serde_json::from_str::<LogLevel>("\"INFO\"").unwrap(), LogLevel::Info);
    assert_eq!(serde_json::from_str::<LogLevel>("\"DEBUG\"").unwrap(), LogLevel::Debug);
    assert_eq!(serde_json::from_str::<LogLevel>("\"TRACE\"").unwrap(), LogLevel::Trace);
}

#[test]
fn test_log_level_roundtrip() {
    let levels = [LogLevel::Error, LogLevel::Warn, LogLevel::Info, LogLevel::Debug, LogLevel::Trace];

    for level in levels {
        let json = serde_json::to_string(&level).unwrap();
        let parsed: LogLevel = serde_json::from_str(&json).unwrap();
        assert_eq!(level, parsed);
    }
}

#[test]
fn test_log_level_display_matches_as_str() {
    let levels = [LogLevel::Error, LogLevel::Warn, LogLevel::Info, LogLevel::Debug, LogLevel::Trace];

    for level in levels {
        assert_eq!(level.to_string(), level.as_str());
    }
}

#[test]
fn test_log_level_debug_format() {
    assert_eq!(format!("{:?}", LogLevel::Error), "Error");
    assert_eq!(format!("{:?}", LogLevel::Warn), "Warn");
    assert_eq!(format!("{:?}", LogLevel::Info), "Info");
    assert_eq!(format!("{:?}", LogLevel::Debug), "Debug");
    assert_eq!(format!("{:?}", LogLevel::Trace), "Trace");
}
