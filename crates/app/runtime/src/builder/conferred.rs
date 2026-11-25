//! The issuers of the capabilities the kernel confers on a member that
//! declared them (`systemprompt_extension::reserved::INSTALL_CONFERRALS`),
//! built from the sealed seeds they use: the member cipher over the
//! `encryption_master_key`, the share-token signer over the
//! `manifest_signing_secret_seed`, and (on the API) the scoped token minter
//! over the token authority. Only the API seeds them; a child host confers
//! nothing. An API whose secret store lacks a key seeds no issuer for it, so a
//! member that declared the row is refused at its read.
//!
//! [`KernelTokenMinters`] mints a member's tokens: an RS256 JWT under the
//! deployment's issuer whose `aud` is exactly one audience the member
//! declared, whose `scope` holds only scopes declared for it, which lives at
//! most `SCOPED_TOKEN_MAX_TTL_SECS` and names the member in `azp`. The
//! system admin is never a subject. Each mint writes a `governance_decisions`
//! row (policy `conferred_token_mint`) before the token is returned, with
//! the member as the actor (`member:<id>`) and the subject beside it; when
//! that write fails, the token is dropped and the mint fails. No handle's
//! `Debug` prints the authority, the issuer or the audit repository.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_governance::authz::{
    DecisionTag, GovernanceDecisionRecord, GovernanceDecisionRepository,
};
use systemprompt_identifiers::{AccessTokenId, Actor, ContextId, ExtensionId, UserId};
use systemprompt_models::auth::{
    SCOPED_TOKEN_MAX_TTL_SECS, ScopedClaims, ScopedMintError, ScopedToken, ScopedTokenMinter,
    ScopedTokenRequest, ScopedVerifyError, TokenAuthority, is_core_audience,
};
use systemprompt_provider_contracts::Dependencies;
use systemprompt_security::AuthError;
use systemprompt_security::conferred::{MemberCiphers, MemberShareTokens};
use systemprompt_security::jwt::decode_scoped_token;
use systemprompt_security::keys::VerificationKeyring;
use systemprompt_traits::{
    ConferralIssuer, DynConferralIssuer, MemberCipher, MemberGrant, SecretStore, ShareTokenSigner,
    TokenAudienceGrant,
};

const MINT_POLICY: &str = "conferred_token_mint";

pub(super) fn with_secret_issuers(
    mut state: Dependencies,
    secrets: &dyn SecretStore,
) -> Dependencies {
    if let Some(ciphers) = secrets
        .encryption_master_key()
        .ok()
        .and_then(|key| MemberCiphers::from_master_key(&key).ok())
    {
        let issuer: DynConferralIssuer<dyn MemberCipher> = Arc::new(ciphers);
        state.insert_sealed(issuer);
    }
    if let Some(tokens) = secrets
        .manifest_signing_secret_seed()
        .ok()
        .and_then(|seed| MemberShareTokens::from_seed(&seed).ok())
    {
        let issuer: DynConferralIssuer<dyn ShareTokenSigner> = Arc::new(tokens);
        state.insert_sealed(issuer);
    }
    state
}

/// The sealed issuer of every member's scoped token minter.
#[derive(Clone)]
pub struct KernelTokenMinters {
    authority: Arc<dyn TokenAuthority>,
    keyring: VerificationKeyring,
    issuer: String,
    audit: GovernanceDecisionRepository,
    system_admin: UserId,
}

impl std::fmt::Debug for KernelTokenMinters {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("KernelTokenMinters(<redacted>)")
    }
}

impl KernelTokenMinters {
    #[must_use]
    pub const fn new(
        authority: Arc<dyn TokenAuthority>,
        keyring: VerificationKeyring,
        issuer: String,
        audit: GovernanceDecisionRepository,
        system_admin: UserId,
    ) -> Self {
        Self {
            authority,
            keyring,
            issuer,
            audit,
            system_admin,
        }
    }
}

impl ConferralIssuer<dyn ScopedTokenMinter> for KernelTokenMinters {
    fn issue(&self, grant: &MemberGrant) -> Arc<dyn ScopedTokenMinter> {
        Arc::new(MemberTokenMinter {
            minters: self.clone(),
            member: grant.member.clone(),
            grants: Arc::clone(&grant.token_audiences),
        })
    }
}

struct MemberTokenMinter {
    minters: KernelTokenMinters,
    member: ExtensionId,
    grants: Arc<[TokenAudienceGrant]>,
}

impl std::fmt::Debug for MemberTokenMinter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemberTokenMinter")
            .field("member", &self.member)
            .finish_non_exhaustive()
    }
}

impl MemberTokenMinter {
    fn check(&self, request: &ScopedTokenRequest) -> Result<(), ScopedMintError> {
        if request.subject == self.minters.system_admin {
            return Err(ScopedMintError::ReservedSubject);
        }
        let grant = self
            .grants
            .iter()
            .find(|grant| grant.audience == request.audience)
            .ok_or_else(|| ScopedMintError::UndeclaredAudience(request.audience.clone()))?;
        if let Some(scope) = request
            .scopes
            .iter()
            .find(|scope| !grant.scopes.contains(scope))
        {
            return Err(ScopedMintError::UndeclaredScope {
                audience: request.audience.clone(),
                scope: scope.clone(),
            });
        }
        if !(1..=SCOPED_TOKEN_MAX_TTL_SECS).contains(&request.ttl_secs) {
            return Err(ScopedMintError::Lifetime(request.ttl_secs));
        }
        Ok(())
    }
}

#[async_trait]
impl ScopedTokenMinter for MemberTokenMinter {
    async fn mint(&self, request: &ScopedTokenRequest) -> Result<ScopedToken, ScopedMintError> {
        self.check(request)?;
        let jti = AccessTokenId::generate();
        let issued_at = chrono::Utc::now().timestamp();
        let expires_at = issued_at.saturating_add(request.ttl_secs);
        let scope = request.scopes.join(" ");
        let token = self
            .minters
            .authority
            .sign_claims(&serde_json::json!({
                "iss": self.minters.issuer,
                "sub": request.subject.as_str(),
                "aud": [request.audience],
                "scope": scope,
                "azp": self.member.as_str(),
                "jti": jti.as_str(),
                "iat": issued_at,
                "exp": expires_at,
            }))
            .map_err(|error| ScopedMintError::Signing(Box::new(error)))?;
        let rules = serde_json::json!({
            "member": self.member.as_str(),
            "subject": request.subject.as_str(),
            "audience": request.audience,
            "scopes": request.scopes,
            "jti": jti.as_str(),
            "expires_at": expires_at,
        });
        let actor = Actor::job(request.subject.clone(), format!("member:{}", self.member));
        let id = format!("mint-{}", jti.as_str());
        let reason = format!("{} minted a token for {}", self.member, request.audience);
        self.minters
            .audit
            .insert(&GovernanceDecisionRecord {
                id: &id,
                actor: &actor,
                session_id: None,
                tool_name: MINT_POLICY,
                agent_id: None,
                agent_scope: None,
                decision: DecisionTag::Allow,
                policy: MINT_POLICY,
                reason: &reason,
                evaluated_rules: &rules,
                plugin_id: None,
                act_chain: &[],
                context_id: &ContextId::generate(),
                task_id: None,
                trace_id: None,
                client_id: None,
                tool_use_id: None,
            })
            .await
            .map_err(|error| ScopedMintError::Audit(Box::new(error)))?;
        Ok(ScopedToken {
            token,
            jti,
            expires_at,
        })
    }

    fn verify(&self, audience: &str, token: &str) -> Result<ScopedClaims, ScopedVerifyError> {
        let grant = self
            .grants
            .iter()
            .find(|grant| grant.audience == audience && !is_core_audience(audience))
            .ok_or_else(|| ScopedVerifyError::UndeclaredAudience(audience.to_owned()))?;
        let claims =
            decode_scoped_token(&self.minters.keyring, &self.minters.issuer, audience, token)
                .map_err(|error| match error {
                    AuthError::InvalidToken(inner)
                        if matches!(
                            inner.kind(),
                            jsonwebtoken::errors::ErrorKind::ExpiredSignature
                        ) =>
                    {
                        ScopedVerifyError::Expired
                    },
                    other => ScopedVerifyError::NotAuthentic(Box::new(other)),
                })?;
        if claims.azp != self.member.as_str() {
            return Err(ScopedVerifyError::OtherMember);
        }
        let scopes: Vec<String> = claims.scope.split_whitespace().map(str::to_owned).collect();
        if scopes.iter().any(|scope| !grant.scopes.contains(scope)) {
            return Err(ScopedVerifyError::UndeclaredScope(audience.to_owned()));
        }
        Ok(ScopedClaims {
            subject: UserId::new(claims.sub),
            audience: audience.to_owned(),
            scopes,
            jti: AccessTokenId::new(claims.jti),
            issued_at: claims.iat,
            expires_at: claims.exp,
        })
    }
}
