//! Compile-time registrations, discovery functions and registration macros
//! for the extension-contributed brain seams.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::retrieval::{BrainAnalysis, FactRetriever, InjectionPolicy, OutcomeSignal};
use super::traits::{EventFilter, FactExtractor, FactValidator, ImportanceModel, SourceContent};
use crate::seam::seam_registration;

seam_registration!(BrainFilterRegistration, EventFilter, discover_brain_filters);
seam_registration!(
    BrainExtractorRegistration,
    FactExtractor,
    discover_brain_extractors
);
seam_registration!(
    BrainValidatorRegistration,
    FactValidator,
    discover_brain_validators
);
seam_registration!(
    BrainImportanceRegistration,
    ImportanceModel,
    discover_brain_importance_models
);
seam_registration!(
    BrainSourceContentRegistration,
    SourceContent,
    discover_brain_source_contents
);
seam_registration!(
    BrainRetrieverRegistration,
    FactRetriever,
    discover_brain_retrievers
);
seam_registration!(
    BrainInjectionPolicyRegistration,
    InjectionPolicy,
    discover_brain_injection_policies
);
seam_registration!(
    BrainOutcomeSignalRegistration,
    OutcomeSignal,
    discover_brain_outcome_signals
);
seam_registration!(
    BrainAnalysisRegistration,
    BrainAnalysis,
    discover_brain_analyses
);

#[macro_export]
macro_rules! register_brain_filter {
    ($($args:tt)*) => {
        $crate::__register_seam!(brain, BrainFilterRegistration, EventFilter, $($args)*);
    };
}

#[macro_export]
macro_rules! register_brain_extractor {
    ($($args:tt)*) => {
        $crate::__register_seam!(brain, BrainExtractorRegistration, FactExtractor, $($args)*);
    };
}

#[macro_export]
macro_rules! register_brain_validator {
    ($($args:tt)*) => {
        $crate::__register_seam!(brain, BrainValidatorRegistration, FactValidator, $($args)*);
    };
}

#[macro_export]
macro_rules! register_brain_importance {
    ($($args:tt)*) => {
        $crate::__register_seam!(brain, BrainImportanceRegistration, ImportanceModel, $($args)*);
    };
}

#[macro_export]
macro_rules! register_brain_source_content {
    ($($args:tt)*) => {
        $crate::__register_seam!(brain, BrainSourceContentRegistration, SourceContent, $($args)*);
    };
}

#[macro_export]
macro_rules! register_brain_retriever {
    ($($args:tt)*) => {
        $crate::__register_seam!(brain, BrainRetrieverRegistration, FactRetriever, $($args)*);
    };
}

#[macro_export]
macro_rules! register_brain_injection_policy {
    ($($args:tt)*) => {
        $crate::__register_seam!(brain, BrainInjectionPolicyRegistration, InjectionPolicy, $($args)*);
    };
}

#[macro_export]
macro_rules! register_brain_outcome_signal {
    ($($args:tt)*) => {
        $crate::__register_seam!(brain, BrainOutcomeSignalRegistration, OutcomeSignal, $($args)*);
    };
}

#[macro_export]
macro_rules! register_brain_analysis {
    ($($args:tt)*) => {
        $crate::__register_seam!(brain, BrainAnalysisRegistration, BrainAnalysis, $($args)*);
    };
}
