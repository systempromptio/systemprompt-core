-- The migration checksum algorithm changed to xxh64; every stored checksum
-- was computed by the previous algorithm and would read as drift. Clearing
-- them lets the runner stamp the current checksum once on its next pass.
ALTER TABLE extension_migrations ALTER COLUMN checksum DROP NOT NULL;
UPDATE extension_migrations SET checksum = NULL;
