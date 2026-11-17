//! Core alone answers the reserved audience subjects: an install resolver
//! claiming `User`, `Role`, `Everyone` or a reserved dimension refuses the
//! set, an install resolver is never asked a reserved subject, and its
//! custom dimension is still consulted.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_extension::{
    AudienceEntry, AudienceResolvers, Membership, NotificationAudienceError,
    NotificationAudienceResolver, SlotError, StandingTag,
};
use systemprompt_identifiers::{ExtensionId, UserId};
use systemprompt_models::notifications::AudienceSubject;

struct Claims {
    name: &'static str,
    claims: fn(&AudienceSubject) -> bool,
    dimensions: &'static [&'static str],
    held: Vec<AudienceSubject>,
}

#[async_trait]
impl NotificationAudienceResolver for Claims {
    fn name(&self) -> &'static str {
        self.name
    }

    fn handles(&self, subject: &AudienceSubject) -> bool {
        (self.claims)(subject)
    }

    fn dimensions(&self) -> &'static [&'static str] {
        self.dimensions
    }

    async fn members(
        &self,
        _subject: &AudienceSubject,
        _limit: usize,
    ) -> Result<Vec<UserId>, NotificationAudienceError> {
        Ok(vec![UserId::new(self.name)])
    }

    async fn is_member(
        &self,
        _subject: &AudienceSubject,
        _user: &UserId,
    ) -> Result<Membership, NotificationAudienceError> {
        Ok(Membership::Member)
    }

    async fn held(&self, _user: &UserId) -> Option<Vec<AudienceSubject>> {
        Some(self.held.clone())
    }
}

fn resolver(
    name: &'static str,
    claims: fn(&AudienceSubject) -> bool,
    dimensions: &'static [&'static str],
) -> Arc<dyn NotificationAudienceResolver> {
    Arc::new(Claims {
        name,
        claims,
        dimensions,
        held: Vec::new(),
    })
}

const DEPARTMENT: &[&str] = &["department"];

fn roles(subject: &AudienceSubject) -> bool {
    !matches!(subject, AudienceSubject::Dimension { .. })
}

fn any_role(subject: &AudienceSubject) -> bool {
    matches!(subject, AudienceSubject::Role(_))
}

fn department(subject: &AudienceSubject) -> bool {
    matches!(subject, AudienceSubject::Dimension { kind, .. } if kind == "department")
}

fn role_dimension(subject: &AudienceSubject) -> bool {
    matches!(subject, AudienceSubject::Dimension { kind, .. } if kind == "role")
}

fn core(name: &'static str, claims: fn(&AudienceSubject) -> bool) -> AudienceEntry {
    AudienceEntry {
        owner: ExtensionId::new("users"),
        standing: StandingTag::Core,
        resolver: resolver(name, claims, &[]),
    }
}

fn install(name: &'static str, claims: fn(&AudienceSubject) -> bool) -> AudienceEntry {
    let dimensions: &'static [&'static str] = if claims(&dimension("department")) {
        DEPARTMENT
    } else {
        &[]
    };
    install_of("acme", name, claims, dimensions)
}

fn install_of(
    owner: &'static str,
    name: &'static str,
    claims: fn(&AudienceSubject) -> bool,
    dimensions: &'static [&'static str],
) -> AudienceEntry {
    AudienceEntry {
        owner: ExtensionId::new(owner),
        standing: StandingTag::Install,
        resolver: resolver(name, claims, dimensions),
    }
}

fn dimension(kind: &str) -> AudienceSubject {
    AudienceSubject::Dimension {
        kind: kind.to_owned(),
        value: "probe".to_owned(),
    }
}

fn team(subject: &AudienceSubject) -> bool {
    matches!(subject, AudienceSubject::Dimension { kind, .. } if kind == "team")
}

fn region(subject: &AudienceSubject) -> bool {
    matches!(subject, AudienceSubject::Dimension { kind, .. } if kind == "region")
}

fn ask<'a>(
    set: &'a AudienceResolvers,
    subject: &AudienceSubject,
) -> Option<&'a Arc<dyn NotificationAudienceResolver>> {
    set.resolvers().iter().find(|r| r.handles(subject))
}

#[test]
fn an_install_resolver_claiming_a_role_is_refused() {
    let err = AudienceResolvers::new(vec![
        install("acme-roles", any_role),
        core("core-roles", roles),
    ])
    .expect_err("an install cannot answer role subjects");
    assert!(
        matches!(
            &err,
            SlotError::ReservedAudience { owner, resolver, subject }
                if owner.as_str() == "acme" && *resolver == "acme-roles" && subject.starts_with("role:")
        ),
        "unexpected error: {err:?}"
    );
}

#[test]
fn an_install_resolver_claiming_a_reserved_dimension_kind_is_refused() {
    let err = AudienceResolvers::new(vec![install("acme-role-dim", role_dimension)])
        .expect_err("the role dimension is reserved");
    assert!(matches!(err, SlotError::ReservedAudience { .. }), "{err:?}");
}

#[test]
fn an_install_resolver_named_like_a_core_resolver_is_refused() {
    let err = AudienceResolvers::new(vec![
        install("core-roles", department),
        core("core-roles", roles),
    ])
    .expect_err("an install cannot answer under a core resolver's name");
    assert!(matches!(err, SlotError::DuplicateId { .. }), "{err:?}");
}

#[test]
fn an_install_custom_dimension_is_consulted_as_the_control() {
    let set = AudienceResolvers::new(vec![
        install("acme-departments", department),
        core("core-roles", roles),
    ])
    .expect("a custom dimension is the install's to answer");
    let subject = AudienceSubject::Dimension {
        kind: "department".to_owned(),
        value: "sales".to_owned(),
    };
    assert_eq!(
        ask(&set, &subject).map(|r| r.name()),
        Some("acme-departments")
    );
}

#[test]
fn the_core_role_resolver_answers_role_questions_beside_an_install() {
    let set = AudienceResolvers::new(vec![
        install("acme-departments", department),
        core("core-roles", roles),
    ])
    .expect("the set builds");
    let subject = AudienceSubject::Role("admin".to_owned());
    assert_eq!(ask(&set, &subject).map(|r| r.name()), Some("core-roles"));
}

struct Sly;

#[async_trait]
impl NotificationAudienceResolver for Sly {
    fn name(&self) -> &'static str {
        "acme-sly"
    }

    fn handles(&self, subject: &AudienceSubject) -> bool {
        matches!(subject, AudienceSubject::Role(role) if role == "admin") || department(subject)
    }

    fn dimensions(&self) -> &'static [&'static str] {
        &["department"]
    }

    async fn members(
        &self,
        _subject: &AudienceSubject,
        _limit: usize,
    ) -> Result<Vec<UserId>, NotificationAudienceError> {
        Ok(vec![UserId::new("everyone")])
    }

    async fn is_member(
        &self,
        _subject: &AudienceSubject,
        _user: &UserId,
    ) -> Result<Membership, NotificationAudienceError> {
        Ok(Membership::Member)
    }

    async fn held(&self, _user: &UserId) -> Option<Vec<AudienceSubject>> {
        Some(vec![
            AudienceSubject::Role("admin".to_owned()),
            AudienceSubject::Dimension {
                kind: "department".to_owned(),
                value: "sales".to_owned(),
            },
        ])
    }
}

#[tokio::test]
async fn an_install_resolver_is_never_asked_a_reserved_subject() {
    let set = AudienceResolvers::new(vec![AudienceEntry {
        owner: ExtensionId::new("acme"),
        standing: StandingTag::Install,
        resolver: Arc::new(Sly),
    }])
    .expect("the probe passes construction by claiming only role admin");
    let admin = AudienceSubject::Role("admin".to_owned());
    let guarded = set.resolvers().first().expect("the guarded resolver");
    assert!(!guarded.handles(&admin));
    assert!(
        guarded
            .members(&admin, 10)
            .await
            .expect("members answers")
            .is_empty()
    );
    assert_eq!(
        guarded
            .is_member(&admin, &UserId::new("u"))
            .await
            .expect("is_member answers"),
        Membership::Unknown
    );
    let held = guarded.held(&UserId::new("u")).await.expect("held answers");
    assert_eq!(held.len(), 1);
    assert!(
        matches!(held.first(), Some(AudienceSubject::Dimension { kind, .. }) if kind == "department")
    );
}

#[test]
fn two_installs_declaring_the_same_dimension_are_refused_naming_both() {
    let err = AudienceResolvers::new(vec![
        install_of("acme", "acme-teams", team, &["team"]),
        install_of("globex", "globex-teams", team, &["team"]),
    ])
    .expect_err("two resolvers cannot both declare team");
    assert!(
        matches!(
            &err,
            SlotError::SharedAudienceDimension { dimension, first_owner, first, second_owner, second }
                if dimension == "team"
                    && first_owner.as_str() == "acme"
                    && *first == "acme-teams"
                    && second_owner.as_str() == "globex"
                    && *second == "globex-teams"
        ),
        "unexpected error: {err:?}"
    );
}

#[test]
fn a_dimension_one_install_declares_and_another_handles_is_refused() {
    let err = AudienceResolvers::new(vec![
        install_of("acme", "acme-teams", team, &["team"]),
        install_of("globex", "globex-teams", team, &[]),
    ])
    .expect_err("a resolver handling a declared dimension shadows it");
    assert!(
        matches!(
            &err,
            SlotError::SharedAudienceDimension { dimension, first_owner, second_owner, .. }
                if dimension == "team"
                    && first_owner.as_str() == "acme"
                    && second_owner.as_str() == "globex"
        ),
        "unexpected error: {err:?}"
    );
}

#[test]
fn distinct_declared_dimensions_build_the_set() {
    let set = AudienceResolvers::new(vec![
        install_of("acme", "acme-teams", team, &["team"]),
        install_of("globex", "globex-regions", region, &["region"]),
    ])
    .expect("distinct dimensions are each their member's own");
    assert_eq!(
        ask(&set, &dimension("team")).map(|r| r.name()),
        Some("acme-teams")
    );
    assert_eq!(
        ask(&set, &dimension("region")).map(|r| r.name()),
        Some("globex-regions")
    );
}

#[test]
fn an_install_is_not_consulted_for_a_dimension_it_does_not_declare() {
    let set = AudienceResolvers::new(vec![install_of("acme", "acme-teams", team, &[])])
        .expect("an undeclared handler builds");
    assert!(ask(&set, &dimension("team")).is_none());
}
