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
