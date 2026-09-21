use chrono::{Days, NaiveDate};
use systemprompt_identifiers::{ProviderId, SecretName};
use systemprompt_models::services::{
    ApiSurface, ProviderEntry, ProviderRegistry, VertexRateCard, WireProtocol,
};
use systemprompt_scheduler::jobs::vertex_discovery::{LIFECYCLE_NOTICE_DAYS, lifecycle_notices};

fn registry_with(model: systemprompt_models::services::VertexRateCardEntry) -> ProviderRegistry {
    let provider = ProviderEntry {
        name: ProviderId::new("vertex"),
        display_name: None,
        description: None,
        wire: WireProtocol::Gemini,
        surface: ApiSurface::Gemini,
        endpoint: "https://example.invalid".to_owned(),
        api_key_secret: SecretName::new("vertex_key"),
        extra_headers: Default::default(),
        models: vec![model.to_provider_model()],
        governance: Default::default(),
    };
    ProviderRegistry {
        providers: vec![provider],
    }
}

#[test]
fn lifecycle_notices_include_only_served_models_at_or_before_the_notice_horizon() {
    let today = NaiveDate::from_ymd_opt(2030, 1, 1).unwrap();
    let mut entry = VertexRateCard::embedded().unwrap().entries[0].clone();
    entry.retires_on = today.checked_add_days(Days::new(LIFECYCLE_NOTICE_DAYS));
    entry.price_until = today.checked_add_days(Days::new(LIFECYCLE_NOTICE_DAYS + 1));
    let served = registry_with(entry.clone());
    let card = VertexRateCard {
        entries: vec![entry.clone()],
    };

    assert_eq!(
        lifecycle_notices(&served, &card, today),
        vec![(entry.id.to_string(), "retires", entry.retires_on.unwrap())]
    );

    let absent = ProviderRegistry {
        providers: Vec::new(),
    };
    assert!(lifecycle_notices(&absent, &card, today).is_empty());
}

#[test]
fn lifecycle_notices_report_both_deadlines_when_both_are_in_window() {
    let today = NaiveDate::from_ymd_opt(2031, 6, 1).unwrap();
    let mut entry = VertexRateCard::embedded().unwrap().entries[0].clone();
    entry.retires_on = today.checked_add_days(Days::new(20));
    entry.price_until = today.checked_add_days(Days::new(40));
    let served = registry_with(entry.clone());
    let notices = lifecycle_notices(
        &served,
        &VertexRateCard {
            entries: vec![entry.clone()],
        },
        today,
    );

    assert_eq!(notices.len(), 2);
    assert_eq!(
        notices[0],
        (entry.id.to_string(), "retires", entry.retires_on.unwrap())
    );
    assert_eq!(
        notices[1],
        (
            entry.id.to_string(),
            "price changes after",
            entry.price_until.unwrap()
        )
    );
}
