//! Unit tests for the pure parts of `systemprompt_brain`: the mapping from
//! each fact change to the revision it records, the row builders the
//! repository tests share, the `brain` config plane (file side pure, apply
//! round trip against the test database in `plane_db`), the recall stages
//! (fusion, visibility, budget fill, same-turn reuse) and lifecycle authority.

#[cfg(test)]
mod authority;
#[cfg(test)]
mod change;
#[cfg(test)]
mod fixtures;
#[cfg(test)]
mod plane;
#[cfg(test)]
mod plane_db;
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
