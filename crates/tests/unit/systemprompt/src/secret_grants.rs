//! A profile grant in `extensions.secrets` names a composed member; a grant
//! to an id no member has is refused at admission.

use systemprompt::{CompositionError, reference};
use systemprompt_identifiers::ExtensionId;

use crate::probe::{Probe, profile};

#[test]
fn a_secret_grant_to_an_unknown_member_is_refused() {
    let mut granted = profile(&[]);
    granted.extensions.secrets.insert(
        ExtensionId::new("no-such-member-grant"),
        vec![systemprompt_traits::SecretPattern::parse("acme_*").expect("a prefix")],
    );
    assert!(matches!(
        reference().build(&granted),
        Err(CompositionError::UnknownSecretGrant(id)) if id.as_str() == "no-such-member-grant"
    ));
}

#[test]
fn a_secret_grant_to_a_composed_member_is_admitted() {
    let mut granted = profile(&[]);
    granted.extensions.secrets.insert(
        ExtensionId::new("probe-granted-secret"),
        vec![systemprompt_traits::SecretPattern::parse("acme_*").expect("a prefix")],
    );
    reference()
        .with(Probe::new("probe-granted-secret", |c| {
            c.secret_prefix("acme_");
        }))
        .build(&granted)
        .expect("a grant naming a composed member is admitted");
}
