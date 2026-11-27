//! The filter of a fact listing, encoded as the instance's query parameters.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, SecondsFormat, Utc};
use systemprompt_identifiers::FactKindId;
use systemprompt_models::brain::{FactStatus, SubjectKey};
use url::Url;

/// Which facts a listing returns.
///
/// A kind, a subject, any of several statuses, a page size (1 to 500; the
/// instance's default is 100), the project whose facts are visible, the
/// instant to read the facts as of, and the cursor of the page to continue
/// from. Every filter left out is not sent.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FactQuery {
    pub kind: Option<FactKindId>,
    pub subject: Option<SubjectKey>,
    pub statuses: Vec<FactStatus>,
    pub limit: Option<u32>,
    pub project: Option<String>,
    pub as_of: Option<DateTime<Utc>>,
    pub after: Option<String>,
}

impl FactQuery {
    pub(crate) fn encode(&self, url: &mut Url) {
        {
            let mut pairs = url.query_pairs_mut();
            if let Some(kind) = &self.kind {
                pairs.append_pair("kind", kind.as_str());
            }
            if let Some(subject) = &self.subject {
                pairs.append_pair("subject", &subject.to_string());
            }
            if !self.statuses.is_empty() {
                let statuses: Vec<&str> =
                    self.statuses.iter().map(|status| status.as_str()).collect();
                pairs.append_pair("status", &statuses.join(","));
            }
            if let Some(limit) = self.limit {
                pairs.append_pair("limit", &limit.to_string());
            }
            if let Some(project) = &self.project {
                pairs.append_pair("project", project);
            }
            if let Some(as_of) = self.as_of {
                pairs.append_pair("as_of", &as_of.to_rfc3339_opts(SecondsFormat::AutoSi, true));
            }
            if let Some(after) = &self.after {
                pairs.append_pair("after", after);
            }
        }
        strip_empty_query(url);
    }
}

pub(crate) fn strip_empty_query(url: &mut Url) {
    if url.query().is_some_and(str::is_empty) {
        url.set_query(None);
    }
}
