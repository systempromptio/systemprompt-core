mod analytics;
mod evaluations;
mod jobs;

pub use analytics::AnalyticsRepository;
pub use evaluations::EvaluationRepository;
pub use jobs::JobRepository;

use chrono::{DateTime, NaiveDate, Utc};
use systemprompt_core_database::DbPool;
use systemprompt_models::ConversationEvaluation;

use crate::models::{JobStatus, ScheduledJob};

#[derive(Debug, Clone)]
pub struct SchedulerRepository {
    jobs: JobRepository,
    evaluations: EvaluationRepository,
    analytics: AnalyticsRepository,
}

impl SchedulerRepository {
    pub fn new(db: &DbPool) -> anyhow::Result<Self> {
        Ok(Self {
            jobs: JobRepository::new(db)?,
            evaluations: EvaluationRepository::new(db)?,
            analytics: AnalyticsRepository::new(db)?,
        })
    }

    pub async fn upsert_job(
        &self,
        job_name: &str,
        schedule: &str,
        enabled: bool,
    ) -> anyhow::Result<()> {
        self.jobs.upsert_job(job_name, schedule, enabled).await
    }

    pub async fn find_job(&self, job_name: &str) -> anyhow::Result<Option<ScheduledJob>> {
        self.jobs.find_job(job_name).await
    }

    pub async fn list_enabled_jobs(&self) -> anyhow::Result<Vec<ScheduledJob>> {
        self.jobs.list_enabled_jobs().await
    }

    pub async fn update_job_execution(
        &self,
        job_name: &str,
        status: JobStatus,
        error: Option<&str>,
        next_run: Option<DateTime<Utc>>,
    ) -> anyhow::Result<()> {
        self.jobs
            .update_job_execution(job_name, status, error, next_run)
            .await
    }

    pub async fn increment_run_count(&self, job_name: &str) -> anyhow::Result<()> {
        self.jobs.increment_run_count(job_name).await
    }

    pub async fn create_evaluation(&self, eval: &ConversationEvaluation) -> anyhow::Result<()> {
        self.evaluations.create_evaluation(eval).await
    }

    pub async fn get_evaluation_by_context(
        &self,
        context_id: &str,
    ) -> anyhow::Result<Option<ConversationEvaluation>> {
        self.evaluations.get_evaluation_by_context(context_id).await
    }

    pub async fn get_unevaluated_conversations(
        &self,
        limit: i64,
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        self.evaluations.get_unevaluated_conversations(limit).await
    }

    pub async fn cleanup_empty_contexts(&self, hours_old: i64) -> anyhow::Result<u64> {
        self.evaluations.cleanup_empty_contexts(hours_old).await
    }

    pub async fn get_evaluation_metrics(
        &self,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        self.analytics
            .get_evaluation_metrics(start_date, end_date)
            .await
    }

    pub async fn get_low_scoring_conversations(
        &self,
        score_threshold: f64,
        limit: i64,
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        self.analytics
            .get_low_scoring_conversations(score_threshold, limit)
            .await
    }

    pub async fn get_top_issues_encountered(
        &self,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        self.analytics
            .get_top_issues_encountered(start_date, end_date)
            .await
    }

    pub async fn get_goal_achievement_stats(
        &self,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        self.analytics
            .get_goal_achievement_stats(start_date, end_date)
            .await
    }

    pub async fn get_detailed_evaluations(
        &self,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
        limit: i64,
        offset: i64,
    ) -> anyhow::Result<Vec<ConversationEvaluation>> {
        self.analytics
            .get_detailed_evaluations(start_date, end_date, limit, offset)
            .await
    }

    pub async fn get_evaluation_quality_distribution(
        &self,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        self.analytics
            .get_evaluation_quality_distribution(start_date, end_date)
            .await
    }
}
