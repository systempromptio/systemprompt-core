//! Core's defaults for the capture seams.
//!
//! The [`SchemaValidator`] every candidate passes first, the
//! [`FormulaImportance`] used when no extension scores a candidate, and the
//! [`ModelExtractor`] for kinds that declare a model extractor. There is
//! deliberately no default event filter: without an extension filter core
//! gathers nothing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod extractor;
mod importance;
mod validator;

pub use extractor::ModelExtractor;
pub use importance::{FormulaImportance, InContextTerms, age_days};
pub use validator::{SchemaCheck, SchemaValidator};
