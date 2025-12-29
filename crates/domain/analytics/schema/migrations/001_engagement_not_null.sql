-- Migration: Add NOT NULL constraints to engagement_events columns
-- These columns have DEFAULT 0, so they should never be NULL

-- Step 1: Update any existing NULL values to 0
UPDATE engagement_events SET focus_time_ms = 0 WHERE focus_time_ms IS NULL;
UPDATE engagement_events SET blur_count = 0 WHERE blur_count IS NULL;
UPDATE engagement_events SET tab_switches = 0 WHERE tab_switches IS NULL;
UPDATE engagement_events SET visible_time_ms = 0 WHERE visible_time_ms IS NULL;
UPDATE engagement_events SET hidden_time_ms = 0 WHERE hidden_time_ms IS NULL;

-- Step 2: Add NOT NULL constraints
ALTER TABLE engagement_events
    ALTER COLUMN focus_time_ms SET NOT NULL,
    ALTER COLUMN blur_count SET NOT NULL,
    ALTER COLUMN tab_switches SET NOT NULL,
    ALTER COLUMN visible_time_ms SET NOT NULL,
    ALTER COLUMN hidden_time_ms SET NOT NULL;
