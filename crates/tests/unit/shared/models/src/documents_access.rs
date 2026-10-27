use std::str::FromStr;

use systemprompt_models::documents::{AccessLevel, ActorKind, DocumentAction, RevisionOriginKind};

#[test]
fn access_levels_are_ordered() {
    assert!(AccessLevel::View < AccessLevel::Comment);
    assert!(AccessLevel::Comment < AccessLevel::Edit);
    assert!(AccessLevel::Edit < AccessLevel::Share);
    assert!(AccessLevel::Share < AccessLevel::Own);
}

#[test]
fn actions_require_expected_level() {
    let cases = [
        (DocumentAction::View, AccessLevel::View),
        (DocumentAction::Comment, AccessLevel::Comment),
        (DocumentAction::Edit, AccessLevel::Edit),
        (DocumentAction::Create, AccessLevel::Edit),
        (DocumentAction::Share, AccessLevel::Share),
        (DocumentAction::Own, AccessLevel::Own),
        (DocumentAction::Delete, AccessLevel::Own),
    ];
    for (action, level) in cases {
        assert_eq!(action.required_level(), level);
    }
}

#[test]
fn wire_names_round_trip() {
    for level in AccessLevel::ALL {
        assert_eq!(AccessLevel::from_str(level.as_str()).as_ref(), Ok(level));
        assert_eq!(serde_json::to_value(level).expect("ser"), level.as_str());
    }
    for action in DocumentAction::ALL {
        assert_eq!(
            DocumentAction::from_str(action.as_str()).as_ref(),
            Ok(action)
        );
    }
    for kind in ActorKind::ALL {
        assert_eq!(ActorKind::from_str(kind.as_str()).as_ref(), Ok(kind));
    }
    for origin in RevisionOriginKind::ALL {
        assert_eq!(
            RevisionOriginKind::from_str(origin.as_str()).as_ref(),
            Ok(origin)
        );
        assert_eq!(serde_json::to_value(origin).expect("ser"), origin.as_str());
    }
    assert_eq!(RevisionOriginKind::McpExecution.as_str(), "mcp_execution");
}

#[test]
fn unknown_values_are_rejected() {
    let err = AccessLevel::from_str("admin").expect_err("unknown");
    assert!(err.to_string().contains("admin"));
    assert!(DocumentAction::from_str("").is_err());
}
