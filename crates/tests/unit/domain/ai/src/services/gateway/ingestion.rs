use systemprompt_ai::{GatewayPolicyIngestOptions, GatewayPolicyIngestReport};

mod ingest_options_tests {
    use super::*;

    #[test]
    fn default_options_are_false() {
        let opts = GatewayPolicyIngestOptions::default();
        assert!(!opts.override_existing);
        assert!(!opts.delete_orphans);
    }

    #[test]
    fn options_can_be_set() {
        let opts = GatewayPolicyIngestOptions {
            override_existing: true,
            delete_orphans: false,
        };
        assert!(opts.override_existing);
        assert!(!opts.delete_orphans);
    }

    #[test]
    fn options_full_override() {
        let opts = GatewayPolicyIngestOptions {
            override_existing: true,
            delete_orphans: true,
        };
        assert!(opts.override_existing);
        assert!(opts.delete_orphans);
    }

    #[test]
    fn options_debug_prints() {
        let opts = GatewayPolicyIngestOptions {
            override_existing: false,
            delete_orphans: true,
        };
        let debug = format!("{opts:?}");
        assert!(debug.contains("override_existing"));
        assert!(debug.contains("delete_orphans"));
    }

    #[test]
    fn options_copy_semantics() {
        let opts = GatewayPolicyIngestOptions {
            override_existing: true,
            delete_orphans: false,
        };
        let opts2 = opts;
        assert!(opts2.override_existing);
    }
}

mod ingest_report_tests {
    use super::*;

    #[test]
    fn default_report_is_zero() {
        let report = GatewayPolicyIngestReport::default();
        assert_eq!(report.inserted, 0);
        assert_eq!(report.updated, 0);
        assert_eq!(report.skipped, 0);
        assert_eq!(report.deleted, 0);
    }

    #[test]
    fn report_fields_set_independently() {
        let report = GatewayPolicyIngestReport {
            inserted: 3,
            updated: 2,
            skipped: 1,
            deleted: 0,
        };
        assert_eq!(report.inserted, 3);
        assert_eq!(report.updated, 2);
        assert_eq!(report.skipped, 1);
        assert_eq!(report.deleted, 0);
    }

    #[test]
    fn report_debug_prints() {
        let report = GatewayPolicyIngestReport {
            inserted: 1,
            updated: 0,
            skipped: 4,
            deleted: 2,
        };
        let debug = format!("{report:?}");
        assert!(debug.contains("inserted"));
        assert!(debug.contains("skipped"));
        assert!(debug.contains("deleted"));
    }

    #[test]
    fn report_copy_semantics() {
        let report = GatewayPolicyIngestReport {
            inserted: 5,
            updated: 0,
            skipped: 0,
            deleted: 0,
        };
        let report2 = report;
        assert_eq!(report2.inserted, 5);
    }

    #[test]
    fn total_operations() {
        let report = GatewayPolicyIngestReport {
            inserted: 2,
            updated: 3,
            skipped: 1,
            deleted: 4,
        };
        let total = report.inserted + report.updated + report.skipped + report.deleted;
        assert_eq!(total, 10);
    }
}
