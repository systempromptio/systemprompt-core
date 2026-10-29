//! The visibility step: may each reader see every resource the event is
//! about, and its excerpt? One `visible_many` call per subject decides all
//! readers still in contention, under the resolver timeout.
//!
//! The first provider naming a subject's kind decides it. A kind no provider
//! names is withheld from every reader ([`Withheld::NoProvider`]): nothing is
//! assumed visible. A provider that times out withholds the subject from
//! every reader it was asked about ([`Withheld::VisibilityTimeout`]). The
//! excerpt is denied by default: it is allowed only when every subject is
//! visible with its excerpt allowed, or, for an event with no subjects, when
//! the reader passed the stream's audience gate. A provider answering fewer
//! verdicts than readers hides the subject from the unanswered readers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use systemprompt_extension::notifications::{ResourceVisibility, Visibility};
use systemprompt_identifiers::UserId;
use systemprompt_models::notifications::SubjectRef;

use super::withheld::Withheld;

#[must_use]
pub fn has_provider(providers: &[Arc<dyn ResourceVisibility>], kind: &str) -> bool {
    providers.iter().any(|p| p.kinds().contains(&kind))
}

pub(crate) async fn check_many(
    providers: &[Arc<dyn ResourceVisibility>],
    subjects: &[SubjectRef],
    readers: &[(UserId, bool)],
    timeout: Duration,
) -> Vec<Result<bool, Withheld>> {
    if subjects.is_empty() {
        return readers
            .iter()
            .map(|(_, in_audience)| Ok(*in_audience))
            .collect();
    }
    let mut verdicts: Vec<Result<bool, Withheld>> = readers.iter().map(|_| Ok(true)).collect();
    for subject in subjects {
        let kind = subject.kind.as_str();
        let open: Vec<usize> = (0..verdicts.len())
            .filter(|i| verdicts[*i].is_ok())
            .collect();
        if open.is_empty() {
            break;
        }
        let Some(provider) = providers.iter().find(|p| p.kinds().contains(&kind)) else {
            for index in open {
                verdicts[index] = Err(Withheld::NoProvider {
                    kind: kind.to_owned(),
                });
            }
            continue;
        };
        let asked: Vec<UserId> = open.iter().map(|i| readers[*i].0.clone()).collect();
        match tokio::time::timeout(timeout, provider.visible_many(subject, &asked)).await {
            Ok(Ok(answers)) => {
                for (position, index) in open.iter().enumerate() {
                    let verdict = &mut verdicts[*index];
                    match answers.get(position) {
                        Some(Visibility::Visible { excerpt_allowed }) => {
                            if let Ok(allowed) = verdict {
                                *allowed &= *excerpt_allowed;
                            }
                        },
                        Some(Visibility::Hidden) | None => {
                            *verdict = Err(Withheld::Hidden {
                                kind: kind.to_owned(),
                            });
                        },
                    }
                }
            },
            Ok(Err(error)) => {
                let error = Arc::new(error);
                for index in open {
                    verdicts[index] = Err(Withheld::Visibility(Arc::clone(&error)));
                }
            },
            Err(_) => {
                tracing::warn!(
                    kind,
                    readers = open.len(),
                    "Notification visibility check timed out"
                );
                for index in open {
                    verdicts[index] = Err(Withheld::VisibilityTimeout {
                        kind: kind.to_owned(),
                    });
                }
            },
        }
    }
    verdicts
}
