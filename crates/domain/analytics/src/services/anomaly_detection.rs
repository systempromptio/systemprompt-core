use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct AnomalyThresholdConfig {
    pub warning_threshold: f64,
    pub critical_threshold: f64,
}

impl Copy for AnomalyThresholdConfig {}

#[derive(Debug, Clone)]
pub struct AnomalyEvent {
    pub timestamp: DateTime<Utc>,
    pub value: f64,
}

impl Copy for AnomalyEvent {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnomalyLevel {
    Normal,
    Warning,
    Critical,
}

#[derive(Debug, Clone)]
pub struct AnomalyCheckResult {
    pub metric_name: String,
    pub current_value: f64,
    pub level: AnomalyLevel,
    pub message: Option<String>,
}

#[derive(Clone, Debug)]
pub struct AnomalyDetectionService {
    thresholds: Arc<RwLock<HashMap<String, AnomalyThresholdConfig>>>,
    recent_events: Arc<RwLock<HashMap<String, Vec<AnomalyEvent>>>>,
}

impl AnomalyDetectionService {
    pub fn new() -> Self {
        Self {
            thresholds: Arc::new(RwLock::new(Self::default_thresholds())),
            recent_events: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn default_thresholds() -> HashMap<String, AnomalyThresholdConfig> {
        let mut thresholds = HashMap::new();

        thresholds.insert(
            "requests_per_minute".into(),
            AnomalyThresholdConfig {
                warning_threshold: 15.0,
                critical_threshold: 30.0,
            },
        );

        thresholds.insert(
            "session_count_per_fingerprint".into(),
            AnomalyThresholdConfig {
                warning_threshold: 5.0,
                critical_threshold: 10.0,
            },
        );

        thresholds.insert(
            "error_rate".into(),
            AnomalyThresholdConfig {
                warning_threshold: 0.1,
                critical_threshold: 0.25,
            },
        );

        thresholds
    }

    pub async fn check_anomaly(&self, metric_name: &str, value: f64) -> AnomalyCheckResult {
        let level = {
            let thresholds = self.thresholds.read().await;
            thresholds
                .get(metric_name)
                .map_or(AnomalyLevel::Normal, |t| {
                    if value >= t.critical_threshold {
                        AnomalyLevel::Critical
                    } else if value >= t.warning_threshold {
                        AnomalyLevel::Warning
                    } else {
                        AnomalyLevel::Normal
                    }
                })
        };

        let message = match level {
            AnomalyLevel::Critical => Some(format!(
                "CRITICAL: {metric_name} = {value:.2} exceeds critical threshold"
            )),
            AnomalyLevel::Warning => Some(format!(
                "WARNING: {metric_name} = {value:.2} exceeds warning threshold"
            )),
            AnomalyLevel::Normal => None,
        };

        AnomalyCheckResult {
            metric_name: metric_name.to_string(),
            current_value: value,
            level,
            message,
        }
    }

    pub async fn record_event(&self, metric_name: &str, value: f64) {
        let now = Utc::now();
        let cutoff = now - Duration::hours(1);
        let key = metric_name.to_string();
        let event = AnomalyEvent {
            timestamp: now,
            value,
        };

        let mut events = self.recent_events.write().await;
        events.entry(key).or_default().push(event);
        if let Some(metric_events) = events.get_mut(metric_name) {
            metric_events.retain(|e| e.timestamp > cutoff);
        }
    }

    pub async fn check_trend_anomaly(
        &self,
        metric_name: &str,
        window_minutes: i64,
    ) -> Option<AnomalyCheckResult> {
        let metric_events = {
            let events = self.recent_events.read().await;
            events.get(metric_name).cloned()?
        };

        if metric_events.len() < 2 {
            return None;
        }

        let cutoff = Utc::now() - Duration::minutes(window_minutes);
        let recent: Vec<_> = metric_events
            .iter()
            .filter(|e| e.timestamp > cutoff)
            .collect();

        if recent.is_empty() {
            return None;
        }

        let avg: f64 = recent.iter().map(|e| e.value).sum::<f64>() / recent.len() as f64;
        let latest = recent.last()?.value;
        let spike_ratio = if avg > 0.0 { latest / avg } else { 1.0 };

        if spike_ratio > 3.0 {
            Some(AnomalyCheckResult {
                metric_name: metric_name.to_string(),
                current_value: latest,
                level: AnomalyLevel::Critical,
                message: Some(format!(
                    "Spike: {metric_name} jumped {spike_ratio:.1}x above average"
                )),
            })
        } else if spike_ratio > 2.0 {
            Some(AnomalyCheckResult {
                metric_name: metric_name.to_string(),
                current_value: latest,
                level: AnomalyLevel::Warning,
                message: Some(format!(
                    "Elevated: {metric_name} is {spike_ratio:.1}x above average"
                )),
            })
        } else {
            None
        }
    }

    pub async fn update_threshold(&self, metric_name: &str, warning: f64, critical: f64) {
        let mut thresholds = self.thresholds.write().await;
        thresholds.insert(
            metric_name.to_string(),
            AnomalyThresholdConfig {
                warning_threshold: warning,
                critical_threshold: critical,
            },
        );
    }

    pub async fn get_recent_events(&self, metric_name: &str) -> Vec<AnomalyEvent> {
        let events = self.recent_events.read().await;
        events.get(metric_name).cloned().unwrap_or_else(Vec::new)
    }
}

impl Default for AnomalyDetectionService {
    fn default() -> Self {
        Self::new()
    }
}
