//! `ContentSlot::build`: every entry's factories run against the resolved
//! state in composition order, content data providers come out ordered by
//! priority, a factory's missing dependency is the slot's `Missing`, and a
//! failing factory is an `Entry` error naming its owner.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use systemprompt_content::{ContentHooks, ContentSlot, ContentSlotBuilder};
use systemprompt_extension::{FactoryError, Resolved, Slot, SlotEntry, SlotError};
use systemprompt_identifiers::ExtensionId;
use systemprompt_provider_contracts::{
    ContentDataContext, ContentDataProvider, Dependencies, ProviderResult,
};
use systemprompt_runtime::Kernel;
use systemprompt_test_fixtures::test_db_pool;
use systemprompt_traits::{BackgroundTasks, DatabaseHandle};

struct Ranked {
    id: &'static str,
    priority: u32,
}

#[async_trait]
impl ContentDataProvider for Ranked {
    fn provider_id(&self) -> &'static str {
        self.id
    }

    async fn enrich_content(
        &self,
        _ctx: &ContentDataContext<'_>,
        _item: &mut Value,
    ) -> ProviderResult<()> {
        Ok(())
    }

    fn priority(&self) -> u32 {
        self.priority
    }
}

fn ranked(id: &'static str, priority: u32) -> ContentSlotBuilder {
    let mut builder = ContentSlotBuilder::default();
    builder.content_data_provider(move |_: &Resolved<'_>| {
        Ok(Some(
            Arc::new(Ranked { id, priority }) as Arc<dyn ContentDataProvider>
        ))
    });
    builder
}

fn entry(owner: &str, builder: ContentSlotBuilder) -> SlotEntry<ContentSlotBuilder> {
    SlotEntry::install(ExtensionId::new(owner), builder)
}

async fn build(entries: Vec<SlotEntry<ContentSlotBuilder>>) -> Result<ContentHooks, SlotError> {
    let pool = test_db_pool().await;
    let handle: Arc<dyn DatabaseHandle> = pool;
    let kernel = Kernel::new(handle, BackgroundTasks::new(), Dependencies::default());
    ContentSlot::build(entries, &kernel.resolved())
}

#[tokio::test]
async fn content_data_providers_are_ordered_by_priority_across_members() {
    let hooks = build(vec![
        entry("first-member", ranked("late", 200)),
        entry("second-member", ranked("early", 10)),
    ])
    .await
    .expect("the slot builds");
    let ids: Vec<&str> = hooks
        .content_data_providers()
        .iter()
        .map(|p| p.provider_id())
        .collect();
    assert_eq!(ids, ["early", "late"]);
}

#[tokio::test]
async fn an_empty_slot_builds_empty_hooks() {
    let hooks = build(Vec::new()).await.expect("the slot builds");
    assert!(hooks.content_data_providers().is_empty());
    assert!(hooks.page_prerenderers().is_empty());
}

#[tokio::test]
async fn a_factory_that_declines_contributes_nothing_and_the_slot_builds() {
    let mut declining = ContentSlotBuilder::default();
    declining
        .content_data_provider(|_: &Resolved<'_>| Ok(None))
        .page_prerenderer(|_: &Resolved<'_>| Ok(None));
    let hooks = build(vec![
        entry("declining-member", declining),
        entry("present-member", ranked("present", 5)),
    ])
    .await
    .expect("a declined factory does not refuse the slot");
    let ids: Vec<&str> = hooks
        .content_data_providers()
        .iter()
        .map(|p| p.provider_id())
        .collect();
    assert_eq!(
        ids,
        ["present"],
        "the declined provider is absent, not defaulted"
    );
    assert!(hooks.page_prerenderers().is_empty());
}

#[tokio::test]
async fn a_factory_missing_a_dependency_is_the_slots_missing() {
    let mut builder = ContentSlotBuilder::default();
    builder.content_data_provider(|r: &Resolved<'_>| {
        let ranked = r.get::<Arc<Ranked>>()?;
        Ok(Some(ranked as Arc<dyn ContentDataProvider>))
    });
    let error = build(vec![entry("needy-member", builder)])
        .await
        .expect_err("an unresolved dependency refuses the slot");
    assert!(matches!(error, SlotError::Missing(_)), "{error:?}");
}

#[tokio::test]
async fn a_failing_factory_names_its_owner() {
    let mut builder = ContentSlotBuilder::default();
    builder.page_prerenderer(|_: &Resolved<'_>| {
        Err(FactoryError::Failed(Box::new(std::io::Error::other(
            "factory refused",
        ))))
    });
    let error = build(vec![entry("failing-member", builder)])
        .await
        .expect_err("a failing factory refuses the slot");
    assert!(
        matches!(
            &error,
            SlotError::Entry { slot: "content", owner, .. } if owner.as_str() == "failing-member"
        ),
        "{error:?}"
    );
}
