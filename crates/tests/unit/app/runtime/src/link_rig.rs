use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_files::documents::DocumentService;
use systemprompt_files::uploads::UploadService;
use systemprompt_identifiers::{JwtToken, McpServerId};
use systemprompt_models::auth::{AuthenticatedUser, McpHopIdentity, TokenAuthority, UserType};
use systemprompt_models::execution::context::RequestContext;
use systemprompt_runtime::documents::links::{
    IssuedLink, LinkIssueError, LinkIssuer, LinkMinter, LinkRequest, LinkTarget, mint_link,
};

pub(crate) const LINKS_ISSUER: &str = "documents-test";
pub(crate) const LINKS_BASE: &str = "https://docs.example.test/";

pub(crate) fn link_authority() -> Arc<dyn TokenAuthority> {
    systemprompt_test_fixtures::test_token_authority(LINKS_ISSUER)
}

#[derive(Debug)]
pub(crate) struct InProcessLinks {
    minter: LinkMinter,
    server: McpServerId,
}

#[async_trait]
impl LinkIssuer for InProcessLinks {
    async fn issue(
        &self,
        hop: Option<&JwtToken>,
        target: LinkTarget,
    ) -> Result<IssuedLink, LinkIssueError> {
        let hop = hop.ok_or(LinkIssueError::MissingHop)?;
        mint_link(
            &self.minter,
            hop.as_str(),
            &LinkRequest {
                server: self.server.clone(),
                target,
            },
        )
        .await
        .map_err(|error| LinkIssueError::Refused {
            status: 403,
            body: error.to_string(),
        })
    }
}

pub(crate) fn in_process_links(
    documents: Arc<DocumentService>,
    uploads: Arc<UploadService>,
    server: &str,
) -> Arc<dyn LinkIssuer> {
    Arc::new(InProcessLinks {
        minter: LinkMinter {
            tokens: link_authority(),
            documents,
            uploads,
            api_external_url: LINKS_BASE.to_owned(),
        },
        server: McpServerId::new(server),
    })
}

pub(crate) fn hop_for(user: &AuthenticatedUser, ctx: &RequestContext, server: &str) -> JwtToken {
    let identity = McpHopIdentity {
        user_id: user.id.clone(),
        user: Some(user.clone()),
        session_id: ctx.session_id().clone(),
        user_type: UserType::User,
        client_id: None,
        act_chain: Vec::new(),
        purpose: None,
        interaction: None,
        mcp_session: None,
    };
    JwtToken::new(
        link_authority()
            .mint_mcp_hop(&McpServerId::new(server), &identity)
            .expect("hop token"),
    )
}
