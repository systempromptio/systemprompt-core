//! The agent member's server is provided even on a host that lacks what an
//! agent needs: building it never fails, and serving refuses with the typed
//! reason naming the first missing collaborator.

use systemprompt_agent_entry::a2a_server::serve::{AgentServerUnavailable, HostedAgentServer};
use systemprompt_identifiers::AgentName;
use systemprompt_runtime::Kernel;
use systemprompt_traits::{AgentServeError, AgentServer, BackgroundTasks, Dependencies};

#[tokio::test]
async fn a_host_without_an_ai_provider_builds_the_server_and_refuses_to_serve() {
    let kernel = Kernel::detached(BackgroundTasks::new(), Dependencies::new());
    let server = HostedAgentServer::from_resolved(&kernel.resolved())
        .expect("building the server never fails on a host missing its inputs");

    let agent = AgentName::new("unavailable_agent");
    let err = server
        .serve(&agent, 0)
        .await
        .expect_err("a host with no AI provider cannot serve an agent");

    let AgentServeError::Serve {
        agent: named,
        source,
    } = &err;
    assert_eq!(named, &agent);
    assert_eq!(
        source.downcast_ref::<AgentServerUnavailable>(),
        Some(&AgentServerUnavailable::AiProvider),
        "{err:?}"
    );
}
