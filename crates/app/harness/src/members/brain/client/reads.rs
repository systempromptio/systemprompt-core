//! Reading the brain: fact listings, one fact, its revision timeline, its
//! why-view, a subject's facts and the open proposals.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use reqwest::Method;
use sp_protocol::wire::brain::{
    BrainFact, BrainFactId, BrainProposal, FactPage, FactRevision, FactWhy, SubjectKey,
};
use sp_protocol::wire::paths;

use super::query::strip_empty_query;
use super::{BrainClient, FactQuery};
use crate::error::BrainError;
use crate::http::Call;

const FACTS: &str = "GET /api/v1/brain/facts";
const FACT: &str = "GET /api/v1/brain/facts/{id}";
const TIMELINE: &str = "GET /api/v1/brain/facts/{id}/timeline";
const WHY: &str = "GET /api/v1/brain/facts/{id}/why";
const SUBJECT: &str = "GET /api/v1/brain/subjects/{kind}/{id}";
const PROPOSALS: &str = "GET /api/v1/brain/proposals";

const LIST_BODY: usize = 8 * 1024 * 1024;

impl BrainClient {
    pub async fn facts(&self, query: &FactQuery) -> Result<FactPage, BrainError> {
        let mut url = self.url(paths::BRAIN_FACTS, &[])?;
        query.encode(&mut url);
        let call = Call::new(Method::GET, url, FACTS).limit(LIST_BODY);
        self.json(call, 200).await
    }

    pub async fn fact(&self, id: &BrainFactId) -> Result<BrainFact, BrainError> {
        let url = self.url(paths::BRAIN_FACTS, &[id.as_str()])?;
        self.json(Call::new(Method::GET, url, FACT), 200).await
    }

    pub async fn timeline(&self, id: &BrainFactId) -> Result<Vec<FactRevision>, BrainError> {
        let url = self.url(paths::BRAIN_FACTS, &[id.as_str(), "timeline"])?;
        let call = Call::new(Method::GET, url, TIMELINE).limit(LIST_BODY);
        self.json(call, 200).await
    }

    pub async fn why(
        &self,
        id: &BrainFactId,
        project: Option<&str>,
    ) -> Result<FactWhy, BrainError> {
        let mut url = self.url(paths::BRAIN_FACTS, &[id.as_str(), "why"])?;
        if let Some(project) = project {
            url.query_pairs_mut().append_pair("project", project);
        }
        self.json(Call::new(Method::GET, url, WHY), 200).await
    }

    pub async fn subject(
        &self,
        subject: &SubjectKey,
        limit: Option<u32>,
        after: Option<&str>,
    ) -> Result<FactPage, BrainError> {
        let mut url = self.url(paths::BRAIN_SUBJECTS, &[&subject.kind, &subject.id])?;
        {
            let mut pairs = url.query_pairs_mut();
            if let Some(limit) = limit {
                pairs.append_pair("limit", &limit.to_string());
            }
            if let Some(after) = after {
                pairs.append_pair("after", after);
            }
        }
        strip_empty_query(&mut url);
        let call = Call::new(Method::GET, url, SUBJECT).limit(LIST_BODY);
        self.json(call, 200).await
    }

    pub async fn proposals(&self, limit: Option<u32>) -> Result<Vec<BrainProposal>, BrainError> {
        let mut url = self.url(paths::BRAIN_PROPOSALS, &[])?;
        if let Some(limit) = limit {
            url.query_pairs_mut()
                .append_pair("limit", &limit.to_string());
        }
        let call = Call::new(Method::GET, url, PROPOSALS).limit(LIST_BODY);
        self.json(call, 200).await
    }
}
