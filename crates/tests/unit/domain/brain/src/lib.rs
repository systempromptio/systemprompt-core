//! Unit tests for `systemprompt_brain`: the mapping from each fact change to
//! the revision it records, the row builders the repository tests share, and
//! the `brain` config plane (its file side pure, its apply round trip
//! against the test database in `plane_db`).

#[cfg(test)]
mod change;
#[cfg(test)]
mod fixtures;
#[cfg(test)]
mod plane;
#[cfg(test)]
mod plane_db;
