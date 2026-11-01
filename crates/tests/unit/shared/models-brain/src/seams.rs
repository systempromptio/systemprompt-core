use std::sync::Arc;

use systemprompt_extension::brain::{EventFilter, FilterVerdict, discover_brain_filters};
use systemprompt_extension::notifications::{
    ResourceVisibility, Visibility, VisibilityError, discover_resource_visibilities,
};
use systemprompt_extension::{register_brain_filter, register_resource_visibility};
use systemprompt_identifiers::UserId;
use systemprompt_models::brain::BrainEnvelope;
use systemprompt_models::notifications::SubjectRef;

#[derive(Default)]
struct LateVisibility;

#[derive(Default)]
struct EarlyVisibility;

#[async_trait::async_trait]
impl ResourceVisibility for LateVisibility {
    fn kinds(&self) -> &'static [&'static str] {
        &["late"]
    }

    async fn visible(&self, _: &SubjectRef, _: &UserId) -> Result<Visibility, VisibilityError> {
        Ok(Visibility::Hidden)
    }
}

#[async_trait::async_trait]
impl ResourceVisibility for EarlyVisibility {
    fn kinds(&self) -> &'static [&'static str] {
        &["early"]
    }

    async fn visible(&self, _: &SubjectRef, _: &UserId) -> Result<Visibility, VisibilityError> {
        Ok(Visibility::Hidden)
    }
}

register_resource_visibility!(LateVisibility, priority = 50);
register_resource_visibility!(
    factory = |_| Arc::new(EarlyVisibility) as Arc<dyn ResourceVisibility>,
    priority = -50
);

struct NamedFilter(&'static str);

impl EventFilter for NamedFilter {
    fn name(&self) -> &'static str {
        self.0
    }

    fn decide(&self, _: &BrainEnvelope) -> FilterVerdict {
        FilterVerdict::Abstain
    }
}

#[derive(Default)]
struct DefaultFilter;

impl EventFilter for DefaultFilter {
    fn name(&self) -> &'static str {
        "default"
    }

    fn decide(&self, _: &BrainEnvelope) -> FilterVerdict {
        FilterVerdict::Ignore
    }
}

register_brain_filter!(DefaultFilter, priority = 10);
register_brain_filter!(
    factory = |_| Arc::new(NamedFilter("first")) as Arc<dyn EventFilter>,
    priority = -10
);

#[test]
fn notification_discovery_sorts_by_priority() {
    let priorities: Vec<i32> = discover_resource_visibilities()
        .iter()
        .map(|registration| registration.priority)
        .collect();
    assert!(priorities.contains(&-50) && priorities.contains(&50));
    assert!(priorities.windows(2).all(|pair| pair[0] <= pair[1]));
}

#[test]
fn brain_discovery_sorts_by_priority() {
    let priorities: Vec<i32> = discover_brain_filters()
        .iter()
        .map(|registration| registration.priority)
        .collect();
    assert_eq!(priorities, vec![-10, 10]);
}
