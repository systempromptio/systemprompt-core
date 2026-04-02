use systemprompt_agent::services::agent_orchestration::reconciler::ConsistencyReport;

#[test]
fn test_consistency_report_new() {
    let report = ConsistencyReport::new();

    assert!(report.consistent_running.is_empty());
    assert!(report.inconsistent_running.is_empty());
    assert!(report.failed.is_empty());
    assert!(report.orphaned_processes.is_empty());
}

#[test]
fn test_consistency_report_default() {
    let report = ConsistencyReport::default();

    assert!(report.consistent_running.is_empty());
    assert!(report.inconsistent_running.is_empty());
    assert!(report.failed.is_empty());
    assert!(report.orphaned_processes.is_empty());
}

#[test]
fn test_consistency_report_no_inconsistencies_when_empty() {
    let report = ConsistencyReport::new();
    assert!(!report.has_inconsistencies());
}

#[test]
fn test_consistency_report_no_inconsistencies_with_consistent_running() {
    let mut report = ConsistencyReport::new();
    report.consistent_running.push("agent-1".to_string());
    report.consistent_running.push("agent-2".to_string());

    assert!(!report.has_inconsistencies());
}

#[test]
fn test_consistency_report_has_inconsistencies_with_inconsistent_running() {
    let mut report = ConsistencyReport::new();
    report.inconsistent_running.push(("agent-1".to_string(), 1234));

    assert!(report.has_inconsistencies());
}

#[test]
fn test_consistency_report_has_inconsistencies_with_orphaned_processes() {
    let mut report = ConsistencyReport::new();
    report.orphaned_processes.push(("agent-1".to_string(), 5678));

    assert!(report.has_inconsistencies());
}

#[test]
fn test_consistency_report_has_inconsistencies_with_both() {
    let mut report = ConsistencyReport::new();
    report.inconsistent_running.push(("agent-1".to_string(), 1234));
    report.orphaned_processes.push(("agent-2".to_string(), 5678));

    assert!(report.has_inconsistencies());
}

#[test]
fn test_consistency_report_no_inconsistencies_with_failed_only() {
    let mut report = ConsistencyReport::new();
    report.failed.push("agent-1".to_string());

    assert!(!report.has_inconsistencies());
}

#[test]
fn test_consistency_report_total_agents_empty() {
    let report = ConsistencyReport::new();
    assert_eq!(report.total_agents(), 0);
}

#[test]
fn test_consistency_report_total_agents_mixed() {
    let mut report = ConsistencyReport::new();
    report.consistent_running.push("a".to_string());
    report.consistent_running.push("b".to_string());
    report.inconsistent_running.push(("c".to_string(), 100));
    report.failed.push("d".to_string());
    report.failed.push("e".to_string());

    assert_eq!(report.total_agents(), 5);
}

#[test]
fn test_consistency_report_total_agents_excludes_orphaned() {
    let mut report = ConsistencyReport::new();
    report.consistent_running.push("a".to_string());
    report.orphaned_processes.push(("orphan".to_string(), 999));

    assert_eq!(report.total_agents(), 1);
}

#[test]
fn test_consistency_report_total_agents_only_consistent() {
    let mut report = ConsistencyReport::new();
    report.consistent_running.push("a".to_string());
    report.consistent_running.push("b".to_string());
    report.consistent_running.push("c".to_string());

    assert_eq!(report.total_agents(), 3);
}

#[test]
fn test_consistency_report_total_agents_only_inconsistent() {
    let mut report = ConsistencyReport::new();
    report.inconsistent_running.push(("a".to_string(), 1));
    report.inconsistent_running.push(("b".to_string(), 2));

    assert_eq!(report.total_agents(), 2);
}

#[test]
fn test_consistency_report_debug() {
    let report = ConsistencyReport::new();
    let debug_str = format!("{:?}", report);

    assert!(debug_str.contains("ConsistencyReport"));
}

#[test]
fn test_consistency_report_log_summary_no_inconsistencies() {
    let report = ConsistencyReport::new();
    report.log_summary();

    assert!(!report.has_inconsistencies());
}

#[test]
fn test_consistency_report_log_summary_with_inconsistencies() {
    let mut report = ConsistencyReport::new();
    report.inconsistent_running.push(("agent-1".to_string(), 42));
    report.log_summary();

    assert!(report.has_inconsistencies());
}

#[test]
fn test_reconcile_starting_services_returns_zero() {
    let result =
        systemprompt_agent::services::agent_orchestration::reconciler::AgentReconciler::reconcile_starting_services();
    assert_eq!(result, 0);
}
