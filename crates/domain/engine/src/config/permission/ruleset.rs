//! The ordered rule list, how settings spell it, and the two base sets agents
//! start from.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::matching::{inside, wildcard};
use super::{Action, EDIT, EXTERNAL_DIRECTORY, InvalidRule, READ, REPEATED_CALL, Rule, SHELL};
use crate::paths::{Paths, display_slash, expand_home};

const PLAN_SHELL_ALLOWED: [&str; 10] = [
    "git status *",
    "git diff *",
    "git log *",
    "git show *",
    "git branch",
    "ls *",
    "dir *",
    "pwd",
    "Get-ChildItem *",
    "Get-Location",
];

/// An ordered list of rules.
///
/// In settings and front matter it is written as `"deny"` (everything),
/// `{"shell": "ask"}`, `{"shell": {"*": "ask", "git *": "allow"}}` or a list of
/// rule objects; a leading `~/` in a pattern is the home directory. A subject
/// no rule matches is asked about. Two base sets are provided: one for an agent
/// that may change things, and a read-only one for planning.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Ruleset(Vec<Rule>);

impl From<Vec<Rule>> for Ruleset {
    fn from(rules: Vec<Rule>) -> Self {
        Self(rules)
    }
}

impl FromIterator<Rule> for Ruleset {
    fn from_iter<I: IntoIterator<Item = Rule>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}

impl<'a> IntoIterator for &'a Ruleset {
    type Item = &'a Rule;
    type IntoIter = std::slice::Iter<'a, Rule>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl Ruleset {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn rules(&self) -> &[Rule] {
        &self.0
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Rule> {
        self.0.iter()
    }

    pub const fn len(&self) -> usize {
        self.0.len()
    }

    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn push(&mut self, rule: Rule) {
        self.0.push(rule);
    }

    pub fn extend(&mut self, other: &Self) {
        self.0.extend(other.0.iter().cloned());
    }

    pub fn layered(layers: &[&Self]) -> Self {
        layers
            .iter()
            .flat_map(|layer| layer.0.iter().cloned())
            .collect()
    }

    pub fn find(&self, permission: &str, pattern: &str) -> Option<&Rule> {
        self.0
            .iter()
            .rev()
            .find(|rule| rule.matches(permission, pattern))
    }

    pub fn evaluate(&self, permission: &str, pattern: &str) -> Action {
        self.find(permission, pattern)
            .map_or(Action::Ask, |rule| rule.action)
    }

    pub fn evaluate_all<'a>(
        &self,
        permission: &str,
        patterns: impl IntoIterator<Item = &'a str>,
    ) -> Action {
        let mut verdict = Action::Allow;
        for pattern in patterns {
            match self.evaluate(permission, pattern) {
                Action::Deny => return Action::Deny,
                Action::Ask => verdict = Action::Ask,
                Action::Allow => {},
            }
        }
        verdict
    }

    pub fn denies_all(&self, permission: &str) -> bool {
        self.0
            .iter()
            .rev()
            .find(|rule| wildcard(&rule.permission, permission))
            .is_some_and(|rule| rule.pattern == "*" && rule.action == Action::Deny)
    }

    // JSON: a rule list as written in a settings file or front matter block.
    pub fn from_value(value: &Value, home: &Path) -> Result<Self, InvalidRule> {
        let mut rules = Self::new();
        match value {
            Value::Null => {},
            Value::String(action) => rules.push(Rule::new("*", "*", parse_action("*", action)?)),
            Value::Object(map) => rules.push_map(map, home)?,
            Value::Array(items) => rules.push_list(items, home)?,
            _ => return Err(InvalidRule::Root),
        }
        Ok(rules)
    }

    pub fn build_defaults(paths: &Paths) -> Self {
        Self::from(vec![
            Rule::new("*", "*", Action::Allow),
            Rule::new(READ, "*.env", Action::Ask),
            Rule::new(READ, "*.env.*", Action::Ask),
            Rule::new(READ, "*.env.example", Action::Allow),
            Rule::new(EXTERNAL_DIRECTORY, "*", Action::Ask),
            Rule::new(EXTERNAL_DIRECTORY, inside(&paths.tmp), Action::Allow),
            Rule::new(REPEATED_CALL, "*", Action::Ask),
        ])
    }

    pub fn plan_defaults(paths: &Paths) -> Self {
        let mut rules = Self::build_defaults(paths);
        rules.push(Rule::new(EDIT, "*", Action::Deny));
        rules.push(Rule::new(
            EDIT,
            inside(&paths.state.join("plans")),
            Action::Allow,
        ));
        rules.push(Rule::new(SHELL, "*", Action::Ask));
        for command in PLAN_SHELL_ALLOWED {
            rules.push(Rule::new(SHELL, command, Action::Allow));
        }
        rules
    }

    // JSON: the object form of a rule list, permission to action or to patterns.
    fn push_map(&mut self, map: &Map<String, Value>, home: &Path) -> Result<(), InvalidRule> {
        for (permission, entry) in map {
            match entry {
                Value::String(action) => {
                    let action = parse_action(permission, action)?;
                    self.push(Rule::new(permission.as_str(), "*", action));
                },
                Value::Object(patterns) => {
                    for (pattern, action) in patterns {
                        let key = format!("{permission}.{pattern}");
                        let Value::String(action) = action else {
                            return Err(InvalidRule::Shape { key });
                        };
                        let action = parse_action(&key, action)?;
                        let pattern = expand_pattern(pattern, home);
                        self.push(Rule::new(permission.as_str(), pattern, action));
                    }
                },
                _ => {
                    return Err(InvalidRule::Shape {
                        key: permission.clone(),
                    });
                },
            }
        }
        Ok(())
    }

    // JSON: the list form of a rule list, one rule object per item.
    fn push_list(&mut self, items: &[Value], home: &Path) -> Result<(), InvalidRule> {
        for (index, item) in items.iter().enumerate() {
            let mut rule: Rule = serde_json::from_value(item.clone())
                .map_err(|source| InvalidRule::Entry { index, source })?;
            rule.pattern = expand_pattern(&rule.pattern, home);
            self.push(rule);
        }
        Ok(())
    }
}

fn parse_action(key: &str, value: &str) -> Result<Action, InvalidRule> {
    Action::parse(value).ok_or_else(|| InvalidRule::Action {
        key: key.to_owned(),
        value: value.to_owned(),
    })
}

fn expand_pattern(pattern: &str, home: &Path) -> String {
    if pattern == "~" || pattern.starts_with("~/") || pattern.starts_with("~\\") {
        display_slash(&expand_home(pattern, home))
    } else {
        pattern.to_owned()
    }
}
