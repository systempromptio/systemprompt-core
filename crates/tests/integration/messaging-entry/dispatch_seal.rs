//! The Slack and Teams dispatch handles are sealed to their own members: no
//! member command, runtime context or other member's route state obtains one
//! or the messaging dispatcher beneath it, while the owning member's route
//! state reads its own handle.

use systemprompt_extension::{LateRead, Resolved, StandingTag};
use systemprompt_identifiers::ExtensionId;
use systemprompt_slack_entry::http::SlackDispatch;
use systemprompt_teams_entry::http::TeamsDispatch;
use systemprompt_test_fixtures::ensure_messaging_bootstrap;
use systemprompt_traits::MessagingDispatcher;

#[tokio::test]
async fn the_platform_dispatch_handles_are_sealed_to_their_own_members() {
    let b = ensure_messaging_bootstrap();
    let ctx = super::common::messaging_context(&b.database_url).await;
    let kernel = ctx.kernel();
    let state = kernel.state();
    assert!(state.is_sealed::<SlackDispatch>());
    assert!(state.is_sealed::<TeamsDispatch>());

    let member = Resolved::__member_command(kernel.as_ref(), state);
    assert!(member.optional::<SlackDispatch>().is_none());
    assert!(member.optional::<TeamsDispatch>().is_none());
    assert!(
        member
            .optional::<LateRead<dyn MessagingDispatcher>>()
            .is_none()
    );
    let runtime = kernel.runtime_state();
    assert!(runtime.find::<SlackDispatch>().is_none());
    assert!(runtime.find::<TeamsDispatch>().is_none());

    let forged = ExtensionId::new("forged-member");
    for standing in [StandingTag::Core, StandingTag::Install] {
        let route = kernel.route_scoped(&forged, standing, &[]);
        assert!(route.get::<SlackDispatch>().is_err(), "{standing:?}");
        assert!(route.get::<TeamsDispatch>().is_err(), "{standing:?}");
    }
    assert!(
        kernel
            .route_scoped(&forged, StandingTag::Install, &[])
            .get::<LateRead<dyn MessagingDispatcher>>()
            .is_err()
    );

    let slack = ExtensionId::new("slack");
    assert!(
        kernel
            .route_scoped(&slack, StandingTag::Core, &[])
            .get::<SlackDispatch>()
            .is_ok(),
        "the Slack member's own route state reads its handle"
    );
    let teams = ExtensionId::new("teams");
    assert!(
        kernel
            .route_scoped(&teams, StandingTag::Core, &[])
            .get::<TeamsDispatch>()
            .is_ok(),
        "the Teams member's own route state reads its handle"
    );
}
