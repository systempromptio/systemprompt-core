-- Migration: Remove image_optimization_status column
-- Image optimization is now filesystem-based - WebP existence is the source of truth

ALTER TABLE markdown_content DROP COLUMN IF EXISTS image_optimization_status;
