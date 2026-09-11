//! Unit tests for health check types
//!
//! Tests cover:
//! - ModuleHealth construction and operations
//! - ModuleHealth AddAssign implementation
//! - HealthSummary default and aggregation methods
//! - HealthChecker builder pattern

use systemprompt_api::{HealthSummary, ModuleHealth};

#[test]
fn test_module_health_default() {
    let health = ModuleHealth::default();
    assert_eq!(health.healthy, 0);
    assert_eq!(health.crashed, 0);
}

#[test]
fn test_module_health_custom_values() {
    let health = ModuleHealth {
        healthy: 5,
        crashed: 2,
    };
    assert_eq!(health.healthy, 5);
    assert_eq!(health.crashed, 2);
}

#[test]
fn test_module_health_all_healthy() {
    let health = ModuleHealth {
        healthy: 10,
        crashed: 0,
    };
    assert_eq!(health.healthy, 10);
    assert_eq!(health.crashed, 0);
}

#[test]
fn test_module_health_all_crashed() {
    let health = ModuleHealth {
        healthy: 0,
        crashed: 5,
    };
    assert_eq!(health.healthy, 0);
    assert_eq!(health.crashed, 5);
}


#[test]
fn test_module_health_add_assign() {
    let mut health = ModuleHealth {
        healthy: 2,
        crashed: 1,
    };
    health += ModuleHealth {
        healthy: 3,
        crashed: 2,
    };
    assert_eq!(health.healthy, 5);
    assert_eq!(health.crashed, 3);
}

#[test]
fn test_module_health_add_assign_with_defaults() {
    let mut health = ModuleHealth::default();
    health += ModuleHealth {
        healthy: 5,
        crashed: 0,
    };
    assert_eq!(health.healthy, 5);
    assert_eq!(health.crashed, 0);
}

#[test]
fn test_module_health_add_assign_multiple() {
    let mut health = ModuleHealth::default();

    for _ in 0..5 {
        health += ModuleHealth {
            healthy: 1,
            crashed: 0,
        };
    }

    for _ in 0..2 {
        health += ModuleHealth {
            healthy: 0,
            crashed: 1,
        };
    }

    assert_eq!(health.healthy, 5);
    assert_eq!(health.crashed, 2);
}

#[test]
fn test_module_health_add_assign_zeros() {
    let mut health = ModuleHealth {
        healthy: 10,
        crashed: 5,
    };
    health += ModuleHealth::default();
    assert_eq!(health.healthy, 10);
    assert_eq!(health.crashed, 5);
}

#[test]
fn test_health_summary_default() {
    let summary = HealthSummary::default();
    assert!(summary.modules.is_empty());
}

#[test]
fn test_health_summary_total_healthy_empty() {
    let summary = HealthSummary::default();
    assert_eq!(summary.total_healthy(), 0);
}

#[test]
fn test_health_summary_total_crashed_empty() {
    let summary = HealthSummary::default();
    assert_eq!(summary.total_crashed(), 0);
}

#[test]
fn test_health_summary_is_all_healthy_empty() {
    let summary = HealthSummary::default();
    assert!(summary.is_all_healthy());
}

#[test]
fn test_health_summary_with_modules() {
    let mut summary = HealthSummary::default();
    summary.modules.insert(
        "api".to_string(),
        ModuleHealth {
            healthy: 3,
            crashed: 0,
        },
    );
    summary.modules.insert(
        "mcp".to_string(),
        ModuleHealth {
            healthy: 2,
            crashed: 0,
        },
    );

    assert_eq!(summary.total_healthy(), 5);
    assert_eq!(summary.total_crashed(), 0);
    assert!(summary.is_all_healthy());
}

#[test]
fn test_health_summary_with_crashed_modules() {
    let mut summary = HealthSummary::default();
    summary.modules.insert(
        "api".to_string(),
        ModuleHealth {
            healthy: 2,
            crashed: 1,
        },
    );
    summary.modules.insert(
        "mcp".to_string(),
        ModuleHealth {
            healthy: 1,
            crashed: 2,
        },
    );

    assert_eq!(summary.total_healthy(), 3);
    assert_eq!(summary.total_crashed(), 3);
    assert!(!summary.is_all_healthy());
}

#[test]
fn test_health_summary_mixed_health() {
    let mut summary = HealthSummary::default();
    summary.modules.insert(
        "healthy-module".to_string(),
        ModuleHealth {
            healthy: 5,
            crashed: 0,
        },
    );
    summary.modules.insert(
        "partially-healthy".to_string(),
        ModuleHealth {
            healthy: 3,
            crashed: 2,
        },
    );
    summary.modules.insert(
        "crashed-module".to_string(),
        ModuleHealth {
            healthy: 0,
            crashed: 3,
        },
    );

    assert_eq!(summary.total_healthy(), 8);
    assert_eq!(summary.total_crashed(), 5);
    assert!(!summary.is_all_healthy());
}


#[test]
fn test_module_health_large_values() {
    let health = ModuleHealth {
        healthy: u32::MAX,
        crashed: u32::MAX / 2,
    };
    assert_eq!(health.healthy, u32::MAX);
    assert_eq!(health.crashed, u32::MAX / 2);
}

#[test]
fn test_health_summary_many_modules() {
    let mut summary = HealthSummary::default();

    for i in 0..100 {
        summary.modules.insert(
            format!("module-{}", i),
            ModuleHealth {
                healthy: 1,
                crashed: 0,
            },
        );
    }

    assert_eq!(summary.modules.len(), 100);
    assert_eq!(summary.total_healthy(), 100);
    assert_eq!(summary.total_crashed(), 0);
    assert!(summary.is_all_healthy());
}

#[test]
fn test_health_summary_overwrite_module() {
    let mut summary = HealthSummary::default();

    summary.modules.insert(
        "api".to_string(),
        ModuleHealth {
            healthy: 5,
            crashed: 0,
        },
    );

    summary.modules.insert(
        "api".to_string(),
        ModuleHealth {
            healthy: 3,
            crashed: 2,
        },
    );

    assert_eq!(summary.modules.len(), 1);
    assert_eq!(summary.total_healthy(), 3);
    assert_eq!(summary.total_crashed(), 2);
}
