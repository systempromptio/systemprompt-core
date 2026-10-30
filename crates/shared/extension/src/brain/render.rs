//! The `<team_memory>` block injected after the client's system prompt and
//! the `x-systemprompt-memory-ids` header value naming its facts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use chrono::{DateTime, SecondsFormat, Utc};
use systemprompt_identifiers::BrainFactId;
use systemprompt_models::brain::WithheldReason;

/// One injected fact at the revision that was read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InjectedFact {
    pub id: BrainFactId,
    pub revision: i32,
    pub source: String,
    pub text: String,
}

/// How many facts were withheld, by reason.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WithheldSummary {
    pub by_reason: BTreeMap<WithheldReason, u32>,
}

impl WithheldSummary {
    pub fn add(&mut self, reason: WithheldReason) {
        *self.by_reason.entry(reason).or_default() += 1;
    }

    #[must_use]
    pub fn total(&self) -> u32 {
        self.by_reason.values().sum()
    }
}

/// The facts injected into one request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecallBlock {
    pub scope: String,
    pub as_of: DateTime<Utc>,
    pub facts: Vec<InjectedFact>,
    pub withheld: WithheldSummary,
    pub tokens: u32,
}

impl RecallBlock {
    #[must_use]
    pub fn new(scope: impl Into<String>, as_of: DateTime<Utc>) -> Self {
        Self {
            scope: scope.into(),
            as_of,
            facts: Vec::new(),
            withheld: WithheldSummary::default(),
            tokens: 0,
        }
    }

    #[must_use]
    pub fn with_fact(mut self, fact: InjectedFact) -> Self {
        self.facts.push(fact);
        self
    }

    #[must_use]
    pub fn with_withheld(mut self, withheld: WithheldSummary) -> Self {
        self.withheld = withheld;
        self
    }

    #[must_use]
    pub const fn with_tokens(mut self, tokens: u32) -> Self {
        self.tokens = tokens;
        self
    }

    #[must_use]
    pub fn render(&self) -> String {
        let mut out = format!(
            "<team_memory scope=\"{}\" as_of=\"{}\">\n",
            escape(&self.scope, true),
            self.as_of.to_rfc3339_opts(SecondsFormat::Secs, true)
        );
        for fact in &self.facts {
            out.push_str(&format!(
                "- [{} · {}] {}\n",
                fact.id,
                escape(&one_line(&fact.source), false),
                escape(&one_line(&fact.text), false)
            ));
        }
        out.push_str("</team_memory>");
        out
    }

    #[must_use]
    pub fn ids(&self) -> String {
        self.facts
            .iter()
            .map(|fact| format!("{}@{}", fact.id, fact.revision))
            .collect::<Vec<_>>()
            .join(",")
    }

    #[must_use]
    pub fn descriptor(&self) -> String {
        format!(
            "injected={} withheld={} tokens={} as_of={}",
            self.facts.len(),
            self.withheld.total(),
            self.tokens,
            self.as_of.to_rfc3339_opts(SecondsFormat::Secs, true)
        )
    }
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn escape(text: &str, attribute: bool) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' if attribute => out.push_str("&quot;"),
            other => out.push(other),
        }
    }
    out
}
