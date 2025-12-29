//! Re-export validation report types from traits.
//!
//! These types are defined in `systemprompt-traits` and re-exported here
//! for backward compatibility.

pub use systemprompt_traits::validation_report::{
    StartupValidationError, StartupValidationReport, ValidationError, ValidationReport,
    ValidationWarning,
};
