use proptest::prelude::*;
use systemprompt_identifiers::{AgentName, Email, TaskId};

proptest! {
    #[test]
    fn task_id_generate_produces_valid_uuid(_seed in any::<u64>()) {
        let id = TaskId::generate();
        let s = id.as_str();
        prop_assert!(!s.is_empty());
        prop_assert!(uuid::Uuid::parse_str(s).is_ok(), "TaskId should be valid UUID: {}", s);
    }

    #[test]
    fn valid_email_roundtrips(
        local in "[a-z]{1,10}",
        domain in "[a-z]{1,8}",
        tld in "[a-z]{2,4}"
    ) {
        let addr = format!("{local}@{domain}.{tld}");
        let email = Email::try_new(&addr);
        prop_assert!(email.is_ok(), "Should be valid email: {}", addr);
        let email = email.unwrap();
        prop_assert_eq!(email.as_str(), addr.as_str());
    }

    #[test]
    fn email_without_at_fails(s in "[a-zA-Z0-9]{1,30}") {
        if !s.contains('@') {
            prop_assert!(Email::try_new(&s).is_err(), "Should reject email without @: {}", s);
        }
    }

    #[test]
    fn empty_string_fails_email(_seed in any::<u8>()) {
        prop_assert!(Email::try_new("").is_err());
    }

    #[test]
    fn agent_name_rejects_empty(_seed in any::<u8>()) {
        prop_assert!(AgentName::try_new("").is_err());
    }

    #[test]
    fn agent_name_rejects_unknown(s in "(unknown|UNKNOWN|Unknown)") {
        prop_assert!(
            AgentName::try_new(&s).is_err(),
            "Should reject 'unknown' variant: {}",
            s
        );
    }

    #[test]
    fn valid_agent_name_roundtrips(name in "[a-z][a-z0-9-]{0,19}") {
        if name != "unknown" {
            let result = AgentName::try_new(&name);
            prop_assert!(result.is_ok(), "Should accept agent name: {}", name);
            let agent_name = result.unwrap();
            prop_assert_eq!(agent_name.as_str(), name.as_str());
        }
    }

    #[test]
    fn email_rejects_leading_dot_local(
        domain in "[a-z]{1,8}",
        tld in "[a-z]{2,4}"
    ) {
        let addr = format!(".bad@{domain}.{tld}");
        prop_assert!(Email::try_new(&addr).is_err(), "Should reject leading dot: {}", addr);
    }

    #[test]
    fn email_rejects_trailing_dot_local(
        local in "[a-z]{1,8}",
        domain in "[a-z]{1,8}",
        tld in "[a-z]{2,4}"
    ) {
        let addr = format!("{local}.@{domain}.{tld}");
        prop_assert!(Email::try_new(&addr).is_err(), "Should reject trailing dot: {}", addr);
    }

    #[test]
    fn email_rejects_consecutive_dots(
        domain in "[a-z]{1,8}",
        tld in "[a-z]{2,4}"
    ) {
        let addr = format!("a..b@{domain}.{tld}");
        prop_assert!(Email::try_new(&addr).is_err(), "Should reject consecutive dots: {}", addr);
    }
}
