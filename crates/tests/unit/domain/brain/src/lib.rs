//! Unit tests for the pure parts of `systemprompt_brain`: the mapping from
//! each fact change to the revision it records, the row builders the
//! repository tests share, the `brain` config plane (file side pure, apply
//! round trip against the test database in `plane_db`), the recall stages
//! (fusion, visibility, budget fill, same-turn reuse), outcome citation
//! scanning (recorded against the test database in `outcomes_db`), lifecycle
//! authority, the capture pipeline's pure stages (composite filter, dedupe
//! decision, schema validator, importance formula, extraction schema and
//! parser), the analyses' threshold decisions and proposal dedupe, and the
//! replay diff (dry runs against a disposable database in `replay_db`).

#[cfg(test)]
mod analyses_bloat;
#[cfg(test)]
mod authority;
#[cfg(test)]
mod change;
#[cfg(test)]
mod defaults_importance;
#[cfg(test)]
mod defaults_validator;
#[cfg(test)]
mod fixtures;
#[cfg(test)]
mod model_schema;
#[cfg(test)]
mod outcomes_cited;
#[cfg(test)]
mod outcomes_db;
#[cfg(test)]
mod pipeline_dedupe;
#[cfg(test)]
mod pipeline_filter;
#[cfg(test)]
mod pipeline_fixtures;
#[cfg(test)]
mod plane;
#[cfg(test)]
mod plane_db;
#[cfg(test)]
mod recall_budget;
#[cfg(test)]
mod recall_fakes;
#[cfg(test)]
mod recall_fixtures;
#[cfg(test)]
mod recall_fusion;
#[cfg(test)]
mod recall_reuse;
#[cfg(test)]
mod recall_visible;
#[cfg(test)]
mod replay_db;
#[cfg(test)]
mod replay_diff;
