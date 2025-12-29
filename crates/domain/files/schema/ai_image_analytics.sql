CREATE OR REPLACE VIEW v_ai_image_generation_stats AS
SELECT
    metadata->'type_specific'->'generation'->>'provider' AS provider,
    metadata->'type_specific'->'generation'->>'model' AS model,
    metadata->'type_specific'->'generation'->>'resolution' AS resolution,
    metadata->'type_specific'->'generation'->>'aspect_ratio' AS aspect_ratio,
    COUNT(*) AS total_images,
    AVG((metadata->'type_specific'->'generation'->>'generation_time_ms')::INTEGER) AS avg_generation_time_ms,
    SUM(size_bytes) AS total_storage_bytes,
    SUM((metadata->'type_specific'->'generation'->>'cost_estimate')::DECIMAL) AS total_cost,
    DATE(created_at) AS generation_date
FROM files
WHERE ai_content = true
  AND deleted_at IS NULL
  AND metadata->'type_specific'->'generation' IS NOT NULL
GROUP BY
    metadata->'type_specific'->'generation'->>'provider',
    metadata->'type_specific'->'generation'->>'model',
    metadata->'type_specific'->'generation'->>'resolution',
    metadata->'type_specific'->'generation'->>'aspect_ratio',
    DATE(created_at);

CREATE INDEX IF NOT EXISTS idx_files_ai_generation_provider
    ON files ((metadata->'type_specific'->'generation'->>'provider'))
    WHERE ai_content = true AND deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_files_ai_generation_model
    ON files ((metadata->'type_specific'->'generation'->>'model'))
    WHERE ai_content = true AND deleted_at IS NULL;
