//! Gateway accounting recovery settles into `ai_requests`, which the `ai`
//! member owns: it runs only when that member is admitted.

use systemprompt_extension::admitted::{AdmittedMember, AdmittedParts};
use systemprompt_extension::contribute::ContributionSink;
use systemprompt_extension::{Admitted, ExtensionMetadata, StandingTag};
use systemprompt_gateway::audit::journal::recovery_admitted;
use systemprompt_identifiers::ExtensionId;

fn composition(ids: &[&'static str]) -> Admitted {
    let members = ids
        .iter()
        .map(|id| AdmittedMember {
            secret_grants: Vec::new(),
            owner: ExtensionId::new(*id),
            metadata: ExtensionMetadata {
                id,
                name: id,
                version: "0.0.1",
            },
            standing: StandingTag::Install,
            sink: ContributionSink::default(),
        })
        .collect();
    Admitted::__from_facade(AdmittedParts {
        withheld: Vec::new(),
        members,
        ..AdmittedParts::default()
    })
}

#[test]
fn without_the_ai_member_recovery_does_not_run() {
    assert!(!recovery_admitted(
        composition(&["gateway", "users"]).built()
    ));
}

#[test]
fn with_the_ai_member_recovery_runs() {
    assert!(recovery_admitted(composition(&["gateway", "ai"]).built()));
}
