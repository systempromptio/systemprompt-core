//! Unit tests for FilesConfig
//!
//! Note: Most FilesConfig tests require ProfileBootstrap to be initialized,
//! which is not available in unit test context. These tests focus on
//! the aspects that can be tested in isolation.

// FilesConfig requires ProfileBootstrap initialization which is an external dependency.
// The config module's functionality is primarily tested through integration tests
// where the full application context is available.
//
// The following aspects cannot be unit tested without mocking:
// - FilesConfig::init()
// - FilesConfig::get()
// - FilesConfig::get_optional()
// - FilesConfig::from_profile()
// - FilesConfig::validate()
// - All path and URL generation methods
//
// These are covered in integration tests where the system is properly bootstrapped.
