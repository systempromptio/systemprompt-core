-- @cost: rows=0 measured=20ms triggers=live
-- services.status is the ServiceStatus vocabulary. NOT VALID adds the
-- constraint under a brief lock without scanning; VALIDATE then scans under
-- SHARE UPDATE EXCLUSIVE, so reads and writes continue while it runs.
ALTER TABLE services
    ADD CONSTRAINT services_status_check
    CHECK (status IN ('starting', 'running', 'stopping', 'stopped', 'error')) NOT VALID;
ALTER TABLE services VALIDATE CONSTRAINT services_status_check;
