-- The OTLP exporter ships live from the tracing pipeline; its replay watermark table is gone.
DROP TABLE IF EXISTS otlp_export_state;
