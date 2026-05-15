
DO $$
BEGIN
    IF to_regclass('cowork_exchange_codes') IS NOT NULL
       AND to_regclass('bridge_exchange_codes') IS NULL THEN
        ALTER TABLE cowork_exchange_codes RENAME TO bridge_exchange_codes;
    END IF;
    IF to_regclass('idx_cowork_exchange_codes_user') IS NOT NULL THEN
        ALTER INDEX idx_cowork_exchange_codes_user RENAME TO idx_bridge_exchange_codes_user;
    END IF;
    IF to_regclass('idx_cowork_exchange_codes_active') IS NOT NULL THEN
        ALTER INDEX idx_cowork_exchange_codes_active RENAME TO idx_bridge_exchange_codes_active;
    END IF;
END $$;
