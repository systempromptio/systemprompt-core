CREATE TABLE IF NOT EXISTS anomaly_thresholds (
    metric_name VARCHAR(100) PRIMARY KEY,
    warning_threshold REAL NOT NULL,
    critical_threshold REAL NOT NULL,
    description TEXT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
