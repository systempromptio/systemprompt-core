CREATE TABLE IF NOT EXISTS anomaly_thresholds (
    metric_name VARCHAR(100) PRIMARY KEY,
    warning_threshold REAL NOT NULL,
    critical_threshold REAL NOT NULL,
    description TEXT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO anomaly_thresholds (metric_name, warning_threshold, critical_threshold, description)
VALUES
    ('requests_per_minute', 15, 30, 'Request velocity per minute'),
    ('session_count_per_fingerprint', 5, 10, 'Sessions per fingerprint'),
    ('avg_response_time_deviation', 2.0, 5.0, 'Standard deviations from mean'),
    ('error_rate', 0.1, 0.25, 'Error rate threshold'),
    ('unique_ips_per_fingerprint', 3, 10, 'IP addresses per fingerprint')
ON CONFLICT (metric_name) DO NOTHING;
