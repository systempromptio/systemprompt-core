-- Unconsumed AI-image analytics view and its supporting expression indexes
-- removed together with their schema file; the files table remains the
-- query surface.
DROP VIEW IF EXISTS v_ai_image_generation_stats CASCADE;
DROP INDEX IF EXISTS idx_files_ai_generation_provider;
DROP INDEX IF EXISTS idx_files_ai_generation_model;
