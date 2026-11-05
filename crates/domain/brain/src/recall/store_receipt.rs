//! The `brain_recalls` row a recall receipt becomes: the audit (injected
//! refs, withheld counts, tokens, `as_of`), the rendered block a same-turn
//! request reuses, the reranker record (`reranked`) and the skip reason.
//! A receipt that carries a late block records `skipped=deadline`
//! together with that block.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use systemprompt_extension::brain::{RecallBlock, RecallOutcome, RecallReceipt};
use systemprompt_models::brain::{FactRef, RecallAudit};

use crate::models::NewRecall;

fn refs(block: &RecallBlock) -> Vec<FactRef> {
    block
        .facts
        .iter()
        .map(|fact| FactRef {
            id: fact.id.clone(),
            revision: fact.revision,
        })
        .collect()
}

fn audit_of(receipt: &RecallReceipt, block: &RecallBlock) -> RecallAudit {
    RecallAudit {
        recall_id: receipt.recall_id.clone(),
        injected: refs(block),
        withheld: block.withheld.by_reason.clone(),
        tokens: block.tokens,
        as_of: block.as_of,
    }
}

#[must_use]
pub fn new_recall(receipt: &RecallReceipt) -> NewRecall {
    let mut conversation = receipt.conversation.clone();
    conversation.ai_request_id = Some(receipt.ai_request_id.clone());
    let (audit, block, skipped) = match (&receipt.outcome, &receipt.late) {
        (RecallOutcome::Injected(block), _) => (audit_of(receipt, block), Some(block), None),
        (RecallOutcome::Skipped { reason }, Some(late)) => {
            (audit_of(receipt, late), Some(late), Some(reason.as_str()))
        },
        (RecallOutcome::Skipped { reason }, None) => (
            RecallAudit {
                recall_id: receipt.recall_id.clone(),
                injected: Vec::new(),
                withheld: BTreeMap::new(),
                tokens: 0,
                as_of: receipt.recorded_at,
            },
            None,
            Some(reason.as_str()),
        ),
    };
    let mut builder = NewRecall::builder(
        receipt.principal.user_id.clone(),
        receipt.principal.client,
        audit,
    )
    .conversation(conversation);
    if let Some(block) = block {
        builder = builder.rendered_block(block.render());
        if let Some(reranked) = &block.reranked {
            builder = builder.reranked(reranked.clone());
        }
    }
    if let Some(reason) = skipped {
        builder = builder.skipped(reason);
    }
    if let Some(project) = &receipt.principal.project {
        builder = builder.project(project.clone());
    }
    if let Some(workflow) = &receipt.workflow {
        builder = builder.workflow(workflow.clone());
    }
    builder.build()
}
