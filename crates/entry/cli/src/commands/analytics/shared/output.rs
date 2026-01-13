use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TrendPoint {
    pub timestamp: String,
    pub value: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TrendData {
    pub points: Vec<TrendPoint>,
    pub period: String,
    pub metric: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StatsSummary {
    pub total: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change_percent: Option<f64>,
    pub period: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BreakdownItem {
    pub name: String,
    pub count: i64,
    pub percentage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BreakdownData {
    pub items: Vec<BreakdownItem>,
    pub total: i64,
    pub label: String,
}

impl BreakdownData {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            items: Vec::new(),
            total: 0,
            label: label.into(),
        }
    }

    pub fn add(&mut self, name: impl Into<String>, count: i64) {
        self.total += count;
        self.items.push(BreakdownItem {
            name: name.into(),
            count,
            percentage: 0.0,
        });
    }

    pub fn finalize(&mut self) {
        if self.total > 0 {
            self.items.iter_mut().for_each(|item| {
                item.percentage = (item.count as f64 / self.total as f64) * 100.0;
            });
        }
        self.items.sort_by(|a, b| b.count.cmp(&a.count));
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MetricCard {
    pub label: String,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secondary: Option<String>,
}

impl MetricCard {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            change: None,
            secondary: None,
        }
    }

    pub fn with_change(mut self, change: impl Into<String>) -> Self {
        self.change = Some(change.into());
        self
    }

    pub fn with_secondary(mut self, secondary: impl Into<String>) -> Self {
        self.secondary = Some(secondary.into());
        self
    }
}

pub fn format_number(n: i64) -> String {
    let s = n.abs().to_string();
    let chars: Vec<char> = s.chars().collect();
    let formatted: String = chars
        .iter()
        .rev()
        .enumerate()
        .flat_map(|(i, c)| {
            if i > 0 && i % 3 == 0 {
                vec![',', *c]
            } else {
                vec![*c]
            }
        })
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    if n < 0 {
        format!("-{}", formatted)
    } else {
        formatted
    }
}

pub fn format_cost(cents: i64) -> String {
    let dollars = cents as f64 / 100.0;
    match dollars {
        d if d < 0.01 && cents > 0 => format!("${:.4}", d),
        d if d < 100.0 => format!("${:.2}", d),
        _ => format!("${:.0}", dollars),
    }
}

pub fn format_percent(value: f64) -> String {
    match value.abs() {
        v if v < 0.1 => format!("{:.2}%", value),
        v if v < 10.0 => format!("{:.1}%", value),
        _ => format!("{:.0}%", value),
    }
}

pub fn format_change(current: i64, previous: i64) -> Option<String> {
    (previous != 0).then(|| {
        let change = ((current - previous) as f64 / previous as f64) * 100.0;
        let sign = if change >= 0.0 { "+" } else { "" };
        format!("{}{:.1}%", sign, change)
    })
}

pub fn format_tokens(tokens: i64) -> String {
    match tokens {
        t if t < 1000 => format!("{}", t),
        t if t < 1_000_000 => format!("{:.1}K", t as f64 / 1000.0),
        _ => format!("{:.1}M", tokens as f64 / 1_000_000.0),
    }
}
