//! Tests for fingerprint model types.

use systemprompt_core_analytics::{FingerprintAnalysisResult, FlagReason};

mod flag_reason_tests {
    use super::*;

    #[test]
    fn high_request_count_as_str() {
        let reason = FlagReason::HighRequestCount;
        assert_eq!(reason.as_str(), "request_count_exceeded_100");
    }

    #[test]
    fn sustained_velocity_as_str() {
        let reason = FlagReason::SustainedVelocity;
        assert_eq!(reason.as_str(), "sustained_velocity_10rpm_1hr");
    }

    #[test]
    fn excessive_sessions_as_str() {
        let reason = FlagReason::ExcessiveSessions;
        assert_eq!(reason.as_str(), "session_count_exceeded_10");
    }

    #[test]
    fn reputation_decay_as_str() {
        let reason = FlagReason::ReputationDecay;
        assert_eq!(reason.as_str(), "reputation_score_below_threshold");
    }

    #[test]
    fn display_returns_same_as_str() {
        let reasons = [
            FlagReason::HighRequestCount,
            FlagReason::SustainedVelocity,
            FlagReason::ExcessiveSessions,
            FlagReason::ReputationDecay,
        ];

        for reason in reasons {
            assert_eq!(format!("{}", reason), reason.as_str());
        }
    }

    #[test]
    fn flag_reasons_are_eq() {
        assert_eq!(FlagReason::HighRequestCount, FlagReason::HighRequestCount);
        assert_ne!(FlagReason::HighRequestCount, FlagReason::SustainedVelocity);
    }

    #[test]
    fn flag_reasons_are_copy() {
        let reason = FlagReason::ExcessiveSessions;
        let copied = reason;
        assert_eq!(reason, copied);
    }

    #[test]
    fn flag_reasons_are_debug() {
        let debug_str = format!("{:?}", FlagReason::ReputationDecay);
        assert!(debug_str.contains("ReputationDecay"));
    }
}

mod fingerprint_analysis_result_tests {
    use super::*;

    fn create_result(should_flag: bool, reasons: Vec<FlagReason>) -> FingerprintAnalysisResult {
        FingerprintAnalysisResult {
            fingerprint_hash: "test_hash_123".to_string(),
            should_flag,
            flag_reasons: reasons,
            new_reputation_score: 75,
            should_ban_ip: false,
            ip_to_ban: None,
        }
    }

    #[test]
    fn result_stores_fingerprint_hash() {
        let result = create_result(false, vec![]);
        assert_eq!(result.fingerprint_hash, "test_hash_123");
    }

    #[test]
    fn result_stores_should_flag() {
        let result = create_result(true, vec![FlagReason::HighRequestCount]);
        assert!(result.should_flag);
    }

    #[test]
    fn result_stores_multiple_flag_reasons() {
        let reasons = vec![
            FlagReason::HighRequestCount,
            FlagReason::SustainedVelocity,
            FlagReason::ExcessiveSessions,
        ];
        let result = create_result(true, reasons.clone());

        assert_eq!(result.flag_reasons.len(), 3);
        assert!(result.flag_reasons.contains(&FlagReason::HighRequestCount));
        assert!(result.flag_reasons.contains(&FlagReason::SustainedVelocity));
        assert!(result.flag_reasons.contains(&FlagReason::ExcessiveSessions));
    }

    #[test]
    fn result_stores_reputation_score() {
        let result = create_result(false, vec![]);
        assert_eq!(result.new_reputation_score, 75);
    }

    #[test]
    fn result_with_ip_ban() {
        let mut result = create_result(true, vec![FlagReason::HighRequestCount]);
        result.should_ban_ip = true;
        result.ip_to_ban = Some("192.168.1.1".to_string());

        assert!(result.should_ban_ip);
        assert_eq!(result.ip_to_ban, Some("192.168.1.1".to_string()));
    }

    #[test]
    fn result_is_clone() {
        let result = create_result(true, vec![FlagReason::HighRequestCount]);
        let cloned = result.clone();

        assert_eq!(result.fingerprint_hash, cloned.fingerprint_hash);
        assert_eq!(result.should_flag, cloned.should_flag);
        assert_eq!(result.flag_reasons.len(), cloned.flag_reasons.len());
    }

    #[test]
    fn result_is_debug() {
        let result = create_result(true, vec![FlagReason::HighRequestCount]);
        let debug_str = format!("{:?}", result);

        assert!(debug_str.contains("FingerprintAnalysisResult"));
        assert!(debug_str.contains("test_hash_123"));
    }
}
