//! The subjects a message names, resolved once per recall and handed to
//! the subject and linked tiers as parallel arrays.
//!
//! A subject is matched when one of its aliases occurs in, or is
//! trigram-similar to, the normalised hints, or its id occurs in them
//! (score `1.0` for an occurrence, the word similarity otherwise). A
//! subject is named only when its best score reaches `alias_min_sim`,
//! and the `max_subjects` best-scoring subjects (ties by kind, then id)
//! are the ones both tiers read. The trigram operator keeps its own
//! threshold (`pg_trgm.word_similarity_threshold`, 0.6), so a lower
//! `alias_min_sim` admits nothing more; a higher one narrows the match.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgPool;
use systemprompt_traits::RepositoryError;

use super::TierFilter;

/// The matched subjects as the tier SQL unnests them: one entry per
/// subject, kinds, ids and scores at the same index.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MatchedSubjects {
    pub kinds: Vec<String>,
    pub ids: Vec<String>,
    pub scores: Vec<f64>,
}

impl MatchedSubjects {
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.kinds.is_empty()
    }
}

pub async fn resolve(
    pool: &PgPool,
    f: &TierFilter,
    hints: &str,
) -> Result<MatchedSubjects, RepositoryError> {
    if hints.is_empty() {
        return Ok(MatchedSubjects::default());
    }
    let rows = sqlx::query!(
        r#"SELECT subject_kind AS "subject_kind!", subject_id AS "subject_id!", max(score) AS "score!" FROM (
               SELECT a.subject_kind, a.subject_id,
                      CASE WHEN strpos($1, a.alias_norm) > 0 THEN 1.0::float8
                           ELSE word_similarity(a.alias_norm, $1)::float8 END AS score
               FROM brain_aliases a
               WHERE strpos($1, a.alias_norm) > 0 OR a.alias_norm <% $1
               UNION ALL
               SELECT s.subject_kind, s.subject_id, 1.0::float8
               FROM brain_subjects s
               WHERE length(s.subject_id) >= 3 AND strpos($1, lower(s.subject_id)) > 0
           ) hits GROUP BY subject_kind, subject_id
           HAVING max(score) >= $2
           ORDER BY max(score) DESC, subject_kind, subject_id
           LIMIT $3"#,
        hints,
        f.alias_min_sim,
        f.max_subjects,
    )
    .fetch_all(pool)
    .await?;
    let mut matched = MatchedSubjects::default();
    for row in rows {
        matched.kinds.push(row.subject_kind);
        matched.ids.push(row.subject_id);
        matched.scores.push(row.score);
    }
    Ok(matched)
}
