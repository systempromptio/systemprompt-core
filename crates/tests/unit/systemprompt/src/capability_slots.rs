//! The brain and notifications capabilities as the reference composition
//! admits them: each member opens its own slot, so its built seams exist
//! exactly when it is admitted; contributed seams build in composition
//! order; a disabled member's slot entry is never built while an enabled
//! twin's is (the negative control); a failing seam factory refuses the
//! build; the core defaults other members provide are appended after the
//! contributed seams exactly when their owner provided them, and the
//! analytics member alone provides its repositories. `notifications-http` holds
//! its two route sets and one gateway route whose limits carry the bearer
//! window (T18, facade side).

use std::any::TypeId;
use std::sync::Arc;

use systemprompt::analytics::repository::AnalyticsRepositories;
use systemprompt::brain::{BrainHandleSlot, BrainSeams, BrainSlot};
use systemprompt::database::{Database, DbPool};
use systemprompt::events::{EventBroadcasters, EventRouter};
use systemprompt::models::brain::BrainEnvelope;
use systemprompt::notifications::{NotificationSeams, NotificationsSlot, OutboxNotificationSink};
use systemprompt::reference;
use systemprompt::traits::EscalationDelivery;
use systemprompt::users::SessionRepository;
use systemprompt_extension::brain::{
    SourceAccess, SourceAccessError, SourceContent, SourceContentError, SourceVerdict,
};
use systemprompt_extension::contribute::build_slot;
use systemprompt_extension::reserved::CoreId;
use systemprompt_extension::router::{
    Allowance, ApiBase, BasePath, BaseRef, GatewayPath, RouteSpec,
};
use systemprompt_extension::{
    Admitted, AudienceResolvers, BuiltState, EventFilter, FactoryError, FilterVerdict, Membership,
    NotificationAudienceError, NotificationAudienceResolver, NotificationRouter, Resolved,
    ResourceVisibility, RouteGroup, Slot, SlotError, Visibility, VisibilityError,
};
use systemprompt_identifiers::{ExtensionId, InstanceId, UserId};
use systemprompt_manifest::profile::{BrainProfile, NotificationsProfile};
use systemprompt_models::auth::RequestActor;
use systemprompt_models::brain::{FactSource, SourceText};
use systemprompt_models::notifications::{AudienceSubject, SubjectRef};
use systemprompt_provider_contracts::Dependencies;
use systemprompt_traits::core_seam::{
    DocumentSourceAccess, DocumentSourceContent, DocumentVisibility, IssueVisibility, RoleAudience,
};
use systemprompt_traits::{CoreSeam, DynSessionTelemetry};

use crate::probe::{Counter, Probe, build, kernel_with, profile, shape};

struct NamedFilter(&'static str);

impl EventFilter for NamedFilter {
    fn name(&self) -> &'static str {
        self.0
    }

    fn decide(&self, _: &BrainEnvelope) -> FilterVerdict {
        FilterVerdict::Ignore
    }
}

fn filter_probe(id: &'static str, name: &'static str, hits: &Arc<Counter>) -> Probe {
    let hits = Arc::clone(hits);
    Probe::new(id, move |c| {
        let hits = Arc::clone(&hits);
        c.slot::<BrainSlot>().filter(
            move |_: &Resolved<'_>| -> Result<Arc<dyn EventFilter>, FactoryError> {
                hits.hit();
                Ok(Arc::new(NamedFilter(name)))
            },
        );
    })
}

fn failing_router_probe(id: &'static str, hits: &Arc<Counter>) -> Probe {
    let hits = Arc::clone(hits);
    Probe::new(id, move |c| {
        let hits = Arc::clone(&hits);
        c.slot::<NotificationsSlot>().router(
            move |_: &Resolved<'_>| -> Result<Arc<dyn NotificationRouter>, FactoryError> {
                hits.hit();
                Err(FactoryError::Failed("probe router refuses".into()))
            },
        );
    })
}

pub(crate) fn slot_seed() -> Dependencies {
    let pool: DbPool = Arc::new(Database::from_pools(
        systemprompt_test_fixtures::lazy_pg_pool(),
        None,
    ));
    Dependencies::default()
        .with(Arc::clone(&pool))
        .with(Arc::new(EventRouter::local_only(Arc::new(
            EventBroadcasters::new(),
        ))))
        .with(Arc::new(NotificationsProfile::default()))
        .with(EscalationDelivery::InProcess)
        .with(OutboxNotificationSink::over_write_pool(
            &pool,
            &InstanceId::new("capability-slots"),
        ))
        .with(Arc::new(BrainProfile::default()))
}

fn slot_owners<S: Slot>(admitted: &Admitted) -> Vec<String> {
    admitted
        .built()
        .members()
        .filter(|member| {
            member
                .sink
                .slots()
                .iter()
                .any(|record| record.slot() == TypeId::of::<S>())
        })
        .map(|member| member.owner.as_str().to_owned())
        .collect()
}

fn with_audiences(seed: Dependencies, built: &BuiltState) -> Result<Dependencies, SlotError> {
    let audiences = AudienceResolvers::from_admitted(built, &kernel_with(seed.clone()).resolved())?;
    Ok(seed.with(Arc::new(audiences)))
}

fn built<S: Slot>(admitted: &Admitted) -> Result<Option<Arc<S::Built>>, SlotError>
where
    S::Built: Clone,
{
    built_over::<S>(admitted, slot_seed())
}

fn built_over<S: Slot>(
    admitted: &Admitted,
    seed: Dependencies,
) -> Result<Option<Arc<S::Built>>, SlotError>
where
    S::Built: Clone,
{
    let kernel = kernel_with(with_audiences(seed, admitted.built())?);
    let resolved = kernel.resolved();
    let mut inputs = Vec::new();
    for member in admitted.built().members() {
        for record in member.sink.slots() {
            if record.slot() == TypeId::of::<S>() {
                inputs.extend(record.take_entry(member.owner.clone(), member.standing));
            }
        }
    }
    let mut state = Dependencies::default();
    if let Some(value) = build_slot(inputs, &resolved)? {
        value
            .insert_into(&mut state)
            .expect("an empty state holds no slot value");
    }
    Ok(state.find::<Arc<S::Built>>().cloned())
}

fn filter_names(brain: &BrainHandleSlot) -> Vec<&'static str> {
    brain.seams().filters.iter().map(|f| f.name()).collect()
}

#[test]
fn brain_and_notifications_open_their_own_slots() {
    let admitted = build(reference()).expect("the reference composition admits");
    assert_eq!(slot_owners::<BrainSlot>(&admitted), ["brain"]);
    assert_eq!(
        slot_owners::<NotificationsSlot>(&admitted),
        ["notifications"]
    );

    let brain = built::<BrainSlot>(&admitted)
        .expect("the brain slot builds")
        .expect("the brain member's slot is built");
    assert!(filter_names(&brain).is_empty(), "core contributes no seam");
    let notifications = built::<NotificationsSlot>(&admitted)
        .expect("the notifications slot builds")
        .expect("the notifications member's slot is built");
    assert_eq!(
        notifications.seams().routers.len(),
        1,
        "core contributes no seam; the default router follows the contributed ones"
    );
}

#[test]
fn disabled_capability_members_leave_no_slot() {
    let admitted = reference()
        .declare()
        .expect("the reference composition declares")
        .admit(&profile(&["brain", "brain-http", "notifications", "notifications-http"]).extensions)
        .expect("disabling the brain and notifications members admits");
    assert!(slot_owners::<BrainSlot>(&admitted).is_empty());
    assert!(slot_owners::<NotificationsSlot>(&admitted).is_empty());
    assert!(
        built::<BrainSlot>(&admitted)
            .expect("an empty slot builds nothing")
            .is_none()
    );
    assert!(
        built::<NotificationsSlot>(&admitted)
            .expect("an empty slot builds nothing")
            .is_none()
    );
}

#[test]
fn contributed_brain_seams_build_in_composition_order() {
    let first = Arc::new(Counter::default());
    let second = Arc::new(Counter::default());
    let admitted = build(
        reference()
            .with(filter_probe("probe-first", "first", &first))
            .with(filter_probe("probe-second", "second", &second)),
    )
    .expect("both probes admit");
    assert_eq!(first.get() + second.get(), 0, "admission only stores");
    let seams = built::<BrainSlot>(&admitted)
        .expect("the brain slot builds")
        .expect("the brain slot is built");
    assert_eq!(filter_names(&seams), ["first", "second"]);
}

#[test]
fn disabled_member_brain_seam_is_never_built() {
    let on = Arc::new(Counter::default());
    let off = Arc::new(Counter::default());
    let admitted = reference()
        .with(filter_probe("probe-on", "on", &on))
        .with(filter_probe("probe-off", "off", &off))
        .declare()
        .expect("both probes declare")
        .admit(&profile(&["probe-off"]).extensions)
        .expect("admission succeeds");
    let seams = built::<BrainSlot>(&admitted)
        .expect("the brain slot builds")
        .expect("the brain slot is built");
    assert_eq!(filter_names(&seams), ["on"]);
    assert_eq!(off.get(), 0, "a disabled member's seam factory never runs");
    assert_eq!(on.get(), 1, "the enabled member's seam factory runs");
}

#[test]
fn disabled_member_notification_seam_is_never_built() {
    let off = Arc::new(Counter::default());
    let admitted = reference()
        .with(failing_router_probe("probe-off", &off))
        .declare()
        .expect("the probe declares")
        .admit(&profile(&["probe-off"]).extensions)
        .expect("admission succeeds");
    let built = built::<NotificationsSlot>(&admitted)
        .expect("the disabled member's failing factory is dropped before the build")
        .expect("the notifications slot is built");
    assert_eq!(built.seams().routers.len(), 1, "only the default router");
    assert_eq!(off.get(), 0, "a disabled member's seam factory never runs");
}

#[test]
fn failing_notification_seam_refuses_the_build() {
    let hits = Arc::new(Counter::default());
    let admitted = build(reference().with(failing_router_probe("probe-failing", &hits)))
        .expect("the probe admits");
    let error = built::<NotificationsSlot>(&admitted).expect_err("a failing seam refuses");
    assert!(
        matches!(
            &error,
            SlotError::Entry { slot: "notifications", owner, .. } if owner.as_str() == "probe-failing"
        ),
        "{error:?}"
    );
    assert_eq!(hits.get(), 1, "negative control: the enabled factory runs");
}

#[test]
fn notifications_http_holds_its_two_route_sets() {
    let admitted = build(reference()).expect("the reference composition admits");
    let sets: Vec<_> = admitted
        .routes()
        .iter()
        .filter(|set| set.owner.as_str() == "notifications-http")
        .collect();
    assert_eq!(sets.len(), 2, "{sets:?}");
    let at = |base: BasePath| {
        sets.iter()
            .find(|set| set.set.base() == BaseRef::Path(base))
            .unwrap_or_else(|| panic!("notifications-http holds a set at {base:?}"))
    };
    let user = at(BasePath::Api(ApiBase::Notifications));
    assert_eq!(user.set.group(), RouteGroup::ContentAndMisc);
    assert_eq!(user.set.defs().len(), 12);
    let admin = at(BasePath::Api(ApiBase::AdminNotifications));
    assert_eq!(admin.set.group(), RouteGroup::ContentAndMisc);
    assert_eq!(admin.set.defs().len(), 1);
    assert!(user.handles.is_empty() && admin.handles.is_empty());
}

#[test]
fn bridge_notifications_gateway_route_carries_the_bearer_window() {
    let admitted = build(reference()).expect("the reference composition admits");
    let routes: Vec<_> = admitted
        .gateway_routes()
        .iter()
        .filter(|route| route.owner.as_str() == "notifications-http")
        .collect();
    assert_eq!(routes.len(), 1, "one gateway route for the bridge inbox");
    let route = routes[0];
    assert_eq!(route.decl.path(), Some(GatewayPath::BridgeNotifications));
    assert!(
        route.decl.limits.rate.bearer_window,
        "the admitted limits carry the bearer window"
    );

    let listed = shape(
        CoreId::NotificationsHttp,
        BaseRef::Gateway(GatewayPath::BridgeNotifications),
        Allowance::GatewayRoute,
    );
    assert_eq!(route.decl.limits, listed.shape.limits);
    assert_eq!(route.decl.router.group(), listed.group);
    let listed_spec = RouteSpec::new(
        listed.shape.policy,
        listed.shape.context,
        listed.shape.limits,
    );
    assert_eq!(route.decl.router.defs().len(), 8);
    assert!(
        route
            .decl
            .router
            .defs()
            .iter()
            .all(|def| def.spec == listed_spec),
        "every bridge def carries the conferred handler-verified shape"
    );
}

struct StubAudience;

#[async_trait::async_trait]
impl NotificationAudienceResolver for StubAudience {
    fn name(&self) -> &'static str {
        "stub-roles"
    }

    fn handles(&self, subject: &AudienceSubject) -> bool {
        !matches!(subject, AudienceSubject::Dimension { .. })
    }

    fn dimensions(&self) -> &'static [&'static str] {
        &[]
    }

    async fn members(
        &self,
        _: &AudienceSubject,
        _: usize,
    ) -> Result<Vec<UserId>, NotificationAudienceError> {
        Ok(Vec::new())
    }

    async fn is_member(
        &self,
        _: &AudienceSubject,
        _: &UserId,
    ) -> Result<Membership, NotificationAudienceError> {
        Ok(Membership::NotMember)
    }

    async fn held(&self, _: &UserId) -> Option<Vec<AudienceSubject>> {
        None
    }
}

struct StubVisibility(&'static [&'static str]);

#[async_trait::async_trait]
impl ResourceVisibility for StubVisibility {
    fn kinds(&self) -> &'static [&'static str] {
        self.0
    }

    async fn visible(&self, _: &SubjectRef, _: &UserId) -> Result<Visibility, VisibilityError> {
        Ok(Visibility::Hidden)
    }
}

struct StubSource;

#[async_trait::async_trait]
impl SourceContent for StubSource {
    fn name(&self) -> &'static str {
        "stub-source"
    }

    fn handles(&self, _: &FactSource) -> bool {
        false
    }

    async fn fetch(&self, _: &FactSource) -> Result<Option<SourceText>, SourceContentError> {
        Ok(None)
    }
}

struct StubAccess;

#[async_trait::async_trait]
impl SourceAccess for StubAccess {
    async fn readable(
        &self,
        _: &SubjectRef,
        _: &UserId,
        _: &RequestActor,
    ) -> Result<SourceVerdict, SourceAccessError> {
        Ok(SourceVerdict::Hidden)
    }
}

fn with_owner_seams(mut seed: Dependencies, users: bool, files: bool) -> Dependencies {
    if users {
        let roles: Arc<dyn NotificationAudienceResolver> = Arc::new(StubAudience);
        seed.insert(CoreSeam::<dyn NotificationAudienceResolver, RoleAudience>::new(roles));
    }
    if files {
        let documents: Arc<dyn ResourceVisibility> = Arc::new(StubVisibility(&["document"]));
        let issues: Arc<dyn ResourceVisibility> = Arc::new(StubVisibility(&["issue_report"]));
        let content: Arc<dyn SourceContent> = Arc::new(StubSource);
        let access: Arc<dyn SourceAccess> = Arc::new(StubAccess);
        seed.insert(CoreSeam::<dyn ResourceVisibility, DocumentVisibility>::new(
            documents,
        ));
        seed.insert(CoreSeam::<dyn ResourceVisibility, IssueVisibility>::new(
            issues,
        ));
        seed.insert(CoreSeam::<dyn SourceContent, DocumentSourceContent>::new(
            content,
        ));
        seed.insert(CoreSeam::<dyn SourceAccess, DocumentSourceAccess>::new(
            access,
        ));
    }
    seed
}

fn notification_seams(users: bool, files: bool) -> NotificationSeams {
    let seed = with_owner_seams(slot_seed(), users, files);
    let kernel =
        kernel_with(with_audiences(seed, &BuiltState::default()).expect("audiences build"));
    let built = NotificationsSlot::build(Vec::new(), &kernel.resolved())
        .expect("the notifications slot builds");
    (**built.seams()).clone()
}

fn brain_seams(users: bool, files: bool) -> BrainSeams {
    let seed = with_owner_seams(slot_seed(), users, files);
    let kernel =
        kernel_with(with_audiences(seed, &BuiltState::default()).expect("audiences build"));
    let built = BrainSlot::build(Vec::new(), &kernel.resolved()).expect("the brain slot builds");
    (**built.seams()).clone()
}

fn kinds(seams: &NotificationSeams) -> Vec<&'static str> {
    seams
        .visibilities
        .iter()
        .flat_map(|v| v.kinds().iter().copied())
        .collect()
}

#[test]
fn the_notification_core_defaults_follow_their_owners() {
    let enabled = notification_seams(true, true);
    let audiences: Vec<&str> = enabled.audiences.iter().map(|a| a.name()).collect();
    assert_eq!(
        audiences,
        ["stub-roles"],
        "the users member's role resolver is appended"
    );
    assert_eq!(kinds(&enabled), ["document", "issue_report"]);
    assert_eq!(enabled.channels.len(), 1, "the AG-UI channel");
    assert_eq!(
        enabled.publish_policies.len(),
        1,
        "the default publish policy"
    );

    let no_users = notification_seams(false, true);
    assert!(
        no_users.audiences.is_empty(),
        "no users member, no role resolver"
    );
    assert_eq!(kinds(&no_users), ["document", "issue_report"]);

    let no_files = notification_seams(true, false);
    assert!(
        no_files.visibilities.is_empty(),
        "no files member, no visibility"
    );
    assert_eq!(no_files.audiences.len(), 1);
}

#[test]
fn the_brain_core_defaults_follow_their_owners() {
    let enabled = brain_seams(true, true);
    let sources: Vec<&str> = enabled.sources.iter().map(|s| s.name()).collect();
    assert_eq!(sources, ["stub-source"]);
    assert_eq!(enabled.audiences.len(), 1);
    assert!(enabled.source_access.is_some());

    let disabled = brain_seams(false, false);
    assert!(
        disabled.sources.is_empty(),
        "no files member, no source reader"
    );
    assert!(
        disabled.audiences.is_empty(),
        "no users member, no role resolver"
    );
    assert!(
        disabled.source_access.is_none(),
        "the brain hides sourced facts"
    );
}

#[test]
fn the_owning_members_provide_the_core_seams() {
    let admitted = build(reference()).expect("the reference composition admits");
    let provider_of = |ty: TypeId| -> Vec<String> {
        admitted
            .built()
            .members()
            .filter(|member| {
                member
                    .sink
                    .provides()
                    .iter()
                    .any(|entry| entry.type_id() == ty)
            })
            .map(|member| member.owner.as_str().to_owned())
            .collect()
    };
    assert_eq!(
        provider_of(TypeId::of::<
            CoreSeam<dyn NotificationAudienceResolver, RoleAudience>,
        >()),
        ["users"]
    );
    for ty in [
        TypeId::of::<CoreSeam<dyn ResourceVisibility, DocumentVisibility>>(),
        TypeId::of::<CoreSeam<dyn ResourceVisibility, IssueVisibility>>(),
        TypeId::of::<CoreSeam<dyn SourceContent, DocumentSourceContent>>(),
        TypeId::of::<CoreSeam<dyn SourceAccess, DocumentSourceAccess>>(),
    ] {
        assert_eq!(provider_of(ty), ["files"]);
    }
    assert!(
        provider_of(TypeId::of::<CoreSeam<dyn ResourceVisibility, RoleAudience>>()).is_empty(),
        "negative control: a kind nobody provides has no provider"
    );
}

#[test]
fn analytics_alone_provides_its_repositories() {
    let admitted = build(reference()).expect("the reference composition admits");
    let analytics = admitted
        .built()
        .members()
        .find(|member| member.owner.as_str() == "analytics")
        .expect("analytics is a required member");
    let ty = TypeId::of::<Arc<AnalyticsRepositories>>();
    let providers = admitted
        .built()
        .members()
        .filter(|member| {
            member
                .sink
                .provides()
                .iter()
                .any(|entry| entry.type_id() == ty)
        })
        .count();
    assert_eq!(providers, 1);

    let mut seed = slot_seed();
    let pool = seed
        .find::<DbPool>()
        .cloned()
        .expect("the seed holds the pool");
    let sessions: DynSessionTelemetry = Arc::new(SessionRepository::new(&pool));
    seed.insert(sessions);
    let kernel = kernel_with(seed);
    let entry = analytics
        .sink
        .provides()
        .iter()
        .find(|entry| entry.type_id() == ty)
        .expect("analytics provides its repositories");
    let mut state = Dependencies::default();
    entry
        .compute(&kernel.resolved())
        .expect("the repositories build from the pool and session store")
        .insert_into(&mut state, &analytics.owner)
        .expect("the first provider inserts");
    assert!(state.find::<Arc<AnalyticsRepositories>>().is_some());
    let second = entry
        .compute(&kernel.resolved())
        .expect("a second build")
        .insert_into(&mut state, &ExtensionId::new("probe-analytics"));
    assert!(
        second.is_err(),
        "negative control: a second provider is a duplicate"
    );
}

struct DimensionAudience(&'static str);

#[async_trait::async_trait]
impl NotificationAudienceResolver for DimensionAudience {
    fn name(&self) -> &'static str {
        self.0
    }

    fn handles(&self, subject: &AudienceSubject) -> bool {
        matches!(subject, AudienceSubject::Dimension { kind, .. } if kind == "group")
    }

    fn dimensions(&self) -> &'static [&'static str] {
        &["group"]
    }

    async fn members(
        &self,
        _: &AudienceSubject,
        _: usize,
    ) -> Result<Vec<UserId>, NotificationAudienceError> {
        Ok(Vec::new())
    }

    async fn is_member(
        &self,
        _: &AudienceSubject,
        _: &UserId,
    ) -> Result<Membership, NotificationAudienceError> {
        Ok(Membership::Member)
    }
}

fn audience_probe(id: &'static str, name: &'static str, hits: &Arc<Counter>) -> Probe {
    let hits = Arc::clone(hits);
    Probe::new(id, move |c| {
        let hits = Arc::clone(&hits);
        c.audience_resolver(
            move |_: &Resolved<'_>| -> Result<Arc<dyn NotificationAudienceResolver>, FactoryError> {
                hits.hit();
                Ok(Arc::new(DimensionAudience(name)))
            },
        );
    })
}

fn audience_names(resolvers: &[Arc<dyn NotificationAudienceResolver>]) -> Vec<&'static str> {
    resolvers.iter().map(|resolver| resolver.name()).collect()
}

fn group(value: &str) -> AudienceSubject {
    AudienceSubject::Dimension {
        kind: "group".to_owned(),
        value: value.to_owned(),
    }
}

fn handler_of<'a>(
    resolvers: &'a [Arc<dyn NotificationAudienceResolver>],
    subject: &AudienceSubject,
) -> Option<&'a str> {
    resolvers
        .iter()
        .find(|resolver| resolver.handles(subject))
        .map(|resolver| resolver.name())
}

#[test]
fn one_audience_registration_serves_notifications_and_brain_recall() {
    let hits = Arc::new(Counter::default());
    let admitted =
        build(reference().with(audience_probe("probe-audience", "install-groups", &hits)))
            .expect("the probe admits");
    let seed = with_owner_seams(slot_seed(), true, false);
    let notifications = built_over::<NotificationsSlot>(&admitted, seed.clone())
        .expect("the notifications slot builds")
        .expect("the notifications slot is built");
    let brain = built_over::<BrainSlot>(&admitted, seed)
        .expect("the brain slot builds")
        .expect("the brain slot is built");
    assert_eq!(
        audience_names(&notifications.seams().audiences),
        ["install-groups", "stub-roles"],
        "contributed resolvers come first, the core role resolver last"
    );
    assert_eq!(
        audience_names(&brain.seams().audiences),
        ["install-groups", "stub-roles"],
        "brain recall reads the same set in the same order"
    );
    assert_eq!(
        handler_of(&brain.seams().audiences, &group("platform")),
        Some("install-groups")
    );
    assert_eq!(
        handler_of(&notifications.seams().audiences, &group("platform")),
        Some("install-groups")
    );
    assert_eq!(hits.get(), 2, "the factory runs once per build of the set");
}

#[test]
fn a_disabled_audience_member_leaves_both_readers_withholding() {
    let hits = Arc::new(Counter::default());
    let admitted = reference()
        .with(audience_probe("probe-audience", "install-groups", &hits))
        .declare()
        .expect("the probe declares")
        .admit(&profile(&["probe-audience"]).extensions)
        .expect("admission succeeds");
    let seed = with_owner_seams(slot_seed(), false, false);
    let notifications = built_over::<NotificationsSlot>(&admitted, seed.clone())
        .expect("the notifications slot builds")
        .expect("the notifications slot is built");
    let brain = built_over::<BrainSlot>(&admitted, seed)
        .expect("the brain slot builds")
        .expect("the brain slot is built");
    assert!(audience_names(&notifications.seams().audiences).is_empty());
    assert!(audience_names(&brain.seams().audiences).is_empty());
    assert_eq!(
        handler_of(&brain.seams().audiences, &group("platform")),
        None
    );
    assert_eq!(
        handler_of(&notifications.seams().audiences, &group("platform")),
        None
    );
    assert_eq!(
        hits.get(),
        0,
        "a disabled member's resolver factory never runs"
    );
}

#[test]
fn the_slots_refuse_to_build_without_the_audience_set() {
    let kernel = kernel_with(with_owner_seams(slot_seed(), true, true));
    assert!(matches!(
        NotificationsSlot::build(Vec::new(), &kernel.resolved()),
        Err(SlotError::Missing(_))
    ));
    assert!(matches!(
        BrainSlot::build(Vec::new(), &kernel.resolved()),
        Err(SlotError::Missing(_))
    ));
}
