use systemprompt_notifications::repository::{FailureStage, FailureStageError};

fn every_stage() -> Vec<FailureStage> {
    vec![
        FailureStage::Decode,
        FailureStage::Process,
        FailureStage::Route,
        FailureStage::Rules,
        FailureStage::Audience,
        FailureStage::Visibility,
        FailureStage::Deliver {
            surface: "agui".to_owned(),
        },
        FailureStage::DeliverRetry {
            surface: "task".to_owned(),
        },
        FailureStage::DeliverAbandoned,
    ]
}

#[test]
fn every_stage_round_trips_through_its_column_text() {
    for stage in every_stage() {
        let text = stage.to_string();
        assert_eq!(
            text.parse::<FailureStage>().expect("parse"),
            stage,
            "{text}"
        );
    }
}

#[test]
fn the_column_text_keeps_the_stored_vocabulary() {
    let texts: Vec<String> = every_stage().iter().map(ToString::to_string).collect();
    assert_eq!(
        texts,
        [
            "process",
            "route",
            "rules",
            "audience",
            "visibility",
            "deliver:agui",
            "deliver_retry:task",
            "deliver"
        ]
    );
}

#[test]
fn a_stored_stage_nothing_writes_is_a_typed_error_never_a_default() {
    for text in ["", "test", "deliver:", "deliver_retry:", "Process"] {
        let error: FailureStageError = text.parse::<FailureStage>().expect_err(text);
        assert!(error.to_string().contains(text), "{error}");
    }
}

#[test]
fn a_stage_serialises_as_its_column_text() {
    let stage = FailureStage::DeliverRetry {
        surface: "scripted".to_owned(),
    };
    assert_eq!(
        serde_json::to_string(&stage).expect("json"),
        "\"deliver_retry:scripted\""
    );
    assert_eq!(
        serde_json::from_str::<FailureStage>("\"deliver:agui\"").expect("parse"),
        FailureStage::Deliver {
            surface: "agui".to_owned()
        }
    );
    assert!(serde_json::from_str::<FailureStage>("\"nonsense\"").is_err());
}
