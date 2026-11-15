//! The `register_*!` macros for the brain seams. No registry collects
//! their submissions; the seams are contributed through the owning
//! capability's slot.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

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
macro_rules! register_brain_reranker {
    ($($args:tt)*) => {
        $crate::__register_seam!(brain, BrainRerankerRegistration, Reranker, $($args)*);
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

#[macro_export]
macro_rules! register_brain_consolidator {
    ($($args:tt)*) => {
        $crate::__register_seam!(brain, BrainConsolidatorRegistration, FactConsolidator, $($args)*);
    };
}
