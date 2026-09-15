use super::*;
use systemprompt_evaluation::campaigns::report::{CampaignReport, build};
use systemprompt_evaluation::experiments::records::ExecutionStatus;
use systemprompt_evaluation::models::AccountingStatus;
use systemprompt_evaluation::repository::experiments::{
    EvaluationRepositories, ReservationAdmission,
};
use systemprompt_identifiers::EvalCampaignId;

struct ReportFixture {
    pool: PgPool,
    input: Fixture,
    repositories: EvaluationRepositories,
    campaign: EvalCampaignId,
    experiment: EvalExperimentId,
}
impl ReportFixture {
    async fn create(repetitions: u32, independent: bool) -> Self {
        let pool = runs_pool()
            .await
            .expect("report contracts require PostgreSQL");
        let input = fixture(&pool).await;
        let repositories = crate::seams::repositories_with_admission(
            &pool,
            crate::fixture_admission::fixture_admission(),
        );
        let campaign = repositories
            .campaigns
            .create(
                &input.owner,
                &input.owner,
                "retained-report",
                &policy(&input),
            )
            .await
            .unwrap();
        let mut cases = Vec::new();
        for partition in [Partition::Development, Partition::Holdout] {
            for index in 0..10 {
                let key = format!("{partition:?}-{index}");
                let mut content = match case_content(&format!("Independent retained {key} task")) {
                    ResourceContent::Case(content) => content,
                    _ => unreachable!(),
                };
                content.partition = partition;
                cases.push(
                    repositories
                        .revisions
                        .create(&input.owner, &key, &ResourceContent::Case(content))
                        .await
                        .unwrap(),
                );
            }
        }
        let dataset = ResourceContent::Dataset(cases.clone());
        let dataset_id = repositories
            .revisions
            .create(&input.owner, "report-dataset", &dataset)
            .await
            .unwrap();
        let mut spec = input.spec(cases, input.rubric.clone(), repetitions);
        spec.dataset = Some(dataset_id);
        spec.frozen.as_mut().unwrap().dataset_digest = content_digest(&dataset).unwrap();
        spec.claim_independent_improvement = independent;
        let experiment = input
            .experiments
            .create_for_campaign(
                &input.owner,
                &input.owner,
                &CampaignExperiment {
                    campaign_id: campaign.clone(),
                    idempotency_key: "report-run".to_owned(),
                    spec,
                },
            )
            .await
            .unwrap();
        sqlx::query("UPDATE eval_experiments SET status='completed' WHERE id=$1")
            .bind(experiment.as_str())
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE eval_executions SET status='completed',finished_at=NOW() WHERE experiment_id=$1").bind(experiment.as_str()).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO eval_execution_measurements(execution_id,quality_milli,latency_ms,input_tokens,output_tokens,tool_calls,attempted_cost_microdollars,accounting_status,verified_success) SELECT id,CASE WHEN variant_index=0 THEN 4200 ELSE 4500 END,CASE WHEN variant_index=0 THEN 200 ELSE 100 END,CASE WHEN variant_index=0 THEN 200 ELSE 100 END,10,0,CASE WHEN variant_index=0 THEN 200 ELSE 100 END,'complete',true FROM eval_executions WHERE experiment_id=$1").bind(experiment.as_str()).execute(&pool).await.unwrap();
        Self {
            pool,
            input,
            repositories,
            campaign,
            experiment,
        }
    }
    async fn report(&self) -> CampaignReport {
        build(
            &self.repositories,
            &self.repositories.revisions,
            &self.input.owner,
            &self.campaign,
            &self.experiment,
        )
        .await
        .unwrap()
    }
}

#[tokio::test]
async fn retained_report_collapses_repetitions_and_requires_both_partitions() {
    let f = ReportFixture::create(2, true).await;
    let report = f.report().await;
    assert!(report.eligible_for_publication, "{:?}", report.limitations);
    assert_eq!((report.development.pairs, report.holdout.pairs), (10, 10));
    assert_eq!(report.baseline_bundle_digest, "a".repeat(64));
    assert_eq!(report.candidate_bundle_digest, "d".repeat(64));
    assert_eq!(report.development.mean_improvement, Some(300.0));
    assert_eq!(report.holdout.lower_confidence_bound, Some(300.0));
    sqlx::query("UPDATE eval_execution_measurements m SET quality_milli=4000 FROM eval_executions x JOIN eval_resource_revisions r ON r.id=x.case_revision_id WHERE m.execution_id=x.id AND x.experiment_id=$1 AND x.variant_index=1 AND r.content->'content'->>'partition'='holdout'").bind(f.experiment.as_str()).execute(&f.pool).await.unwrap();
    let report = f.report().await;
    assert!(report.development.eligible);
    assert!(!report.holdout.eligible);
    assert!(!report.eligible_for_publication);
    assert!(
        report
            .holdout
            .reasons
            .iter()
            .any(|reason| reason.contains("Quality non-regression"))
    );
}

#[tokio::test]
async fn retained_report_never_infers_missing_tokens_quality_or_latency() {
    let f = ReportFixture::create(2, true).await;
    for column in [
        "input_tokens",
        "output_tokens",
        "quality_milli",
        "latency_ms",
    ] {
        let statement = format!(
            "UPDATE eval_execution_measurements SET {column}=NULL WHERE execution_id=(SELECT id FROM eval_executions WHERE experiment_id=$1 AND variant_index=1 ORDER BY id LIMIT 1)"
        );
        sqlx::query(sqlx::AssertSqlSafe(statement))
            .bind(f.experiment.as_str())
            .execute(&f.pool)
            .await
            .unwrap();
        let report = f.report().await;
        assert!(
            !report.eligible_for_publication,
            "missing {column} cannot be inferred"
        );
        assert!(
            report
                .limitations
                .iter()
                .any(|reason| reason.contains("incomplete outcome evidence"))
        );
        sqlx::query("UPDATE eval_execution_measurements m SET input_tokens=100,output_tokens=10,quality_milli=4500,latency_ms=100 FROM eval_executions x WHERE x.id=m.execution_id AND x.experiment_id=$1 AND x.variant_index=1").bind(f.experiment.as_str()).execute(&f.pool).await.unwrap();
    }
    sqlx::query("UPDATE eval_execution_measurements m SET accounting_status='partial' FROM eval_executions x WHERE x.id=m.execution_id AND x.experiment_id=$1 AND x.variant_index=1").bind(f.experiment.as_str()).execute(&f.pool).await.unwrap();
    let comparison = f
        .repositories
        .lifecycle
        .comparison(&f.input.owner, &f.experiment)
        .await
        .unwrap();
    assert!(!comparison.variants.is_empty());
    for row in &comparison.variants {
        assert_eq!(row.status, ExecutionStatus::Completed);
        let expected = if row.variant == 0 {
            AccountingStatus::Complete
        } else {
            AccountingStatus::Partial
        };
        assert_eq!(
            row.measurement.as_ref().unwrap().accounting_status,
            expected
        );
    }
    let report = f.report().await;
    assert!(!report.eligible_for_publication);
    assert!(
        report
            .development
            .reasons
            .iter()
            .any(|reason| reason == "Incomplete accounting")
    );
    assert!(
        report
            .holdout
            .reasons
            .iter()
            .any(|reason| reason == "Incomplete accounting")
    );
}

#[tokio::test]
async fn retained_report_rejects_incomplete_execution_pairs_and_unsettled_budget() {
    let f = ReportFixture::create(2, true).await;
    sqlx::query("UPDATE eval_executions SET status='blocked' WHERE id=(SELECT id FROM eval_executions WHERE experiment_id=$1 AND variant_index=1 ORDER BY id LIMIT 1)").bind(f.experiment.as_str()).execute(&f.pool).await.unwrap();
    let report = f.report().await;
    assert!(!report.eligible_for_publication);
    assert!(
        report
            .limitations
            .iter()
            .any(|reason| reason.contains("incomplete outcome evidence"))
    );
    sqlx::query("UPDATE eval_executions SET status='completed' WHERE experiment_id=$1")
        .bind(f.experiment.as_str())
        .execute(&f.pool)
        .await
        .unwrap();
    assert!(f.report().await.eligible_for_publication);
    assert!(matches!(
        f.repositories
            .budgets
            .reserve(
                &f.input.owner,
                &f.input.budget,
                "unsettled-report-request",
                17
            )
            .await
            .unwrap(),
        ReservationAdmission::Admitted(_)
    ));
    let report = f.report().await;
    assert!(!report.eligible_for_publication);
    assert!(
        report
            .limitations
            .iter()
            .any(|reason| reason == "Accounting has unsettled reservations or is frozen")
    );
}

#[tokio::test]
async fn retained_report_cannot_publish_development_only_or_running_experiments() {
    let f = ReportFixture::create(1, false).await;
    let report = f.report().await;
    assert!(report.development.eligible && report.holdout.eligible);
    assert!(!report.eligible_for_publication);
    assert!(
        report
            .limitations
            .iter()
            .any(|reason| reason == "A fresh independent holdout was not reserved")
    );
    sqlx::query("UPDATE eval_experiments SET status='running' WHERE id=$1")
        .bind(f.experiment.as_str())
        .execute(&f.pool)
        .await
        .unwrap();
    assert!(
        f.report()
            .await
            .limitations
            .iter()
            .any(|reason| reason == "Experiment is not completed")
    );
    let foreign = new_owner(&f.pool).await;
    assert!(
        build(
            &f.repositories,
            &f.repositories.revisions,
            &foreign,
            &f.campaign,
            &f.experiment
        )
        .await
        .is_err()
    );
    assert!(
        build(
            &f.repositories,
            &f.repositories.revisions,
            &f.input.owner,
            &f.campaign,
            &EvalExperimentId::generate()
        )
        .await
        .is_err()
    );
}

#[tokio::test]
async fn absent_execution_rows_cannot_be_hidden_by_another_successful_repetition() {
    let f = ReportFixture::create(2, true).await;
    assert!(f.report().await.eligible_for_publication);
    let case: String = sqlx::query_scalar(
        "SELECT case_revision_id FROM eval_executions WHERE experiment_id=$1 ORDER BY id LIMIT 1",
    )
    .bind(f.experiment.as_str())
    .fetch_one(&f.pool)
    .await
    .unwrap();
    sqlx::query("DELETE FROM eval_execution_measurements WHERE execution_id IN(SELECT id FROM eval_executions WHERE experiment_id=$1 AND case_revision_id=$2 AND repetition=1)").bind(f.experiment.as_str()).bind(&case).execute(&f.pool).await.unwrap();
    sqlx::query("DELETE FROM eval_executions WHERE experiment_id=$1 AND case_revision_id=$2 AND repetition=1").bind(f.experiment.as_str()).bind(&case).execute(&f.pool).await.unwrap();
    let report = f.report().await;
    assert_eq!((report.development.pairs, report.holdout.pairs), (10, 10));
    assert!(
        !report.eligible_for_publication,
        "both missing rows must remain an incomplete repetition even when another pair is successful"
    );
    assert!(
        report
            .limitations
            .iter()
            .any(|reason| reason.contains("incomplete baseline/candidate pair"))
    );
}
