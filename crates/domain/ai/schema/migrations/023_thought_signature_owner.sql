-- Unowned ephemeral signatures cannot be attributed safely. Invalidate this
-- one-hour cache before making authenticated ownership mandatory.
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns
                   WHERE table_schema = current_schema()
                     AND table_name = 'ai_gateway_thought_signatures'
                     AND column_name = 'user_id') THEN
        DELETE FROM ai_gateway_thought_signatures;
        ALTER TABLE ai_gateway_thought_signatures
            ADD COLUMN user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE;
        ALTER TABLE ai_gateway_thought_signatures
            DROP CONSTRAINT ai_gateway_thought_signatures_pkey;
        ALTER TABLE ai_gateway_thought_signatures
            ADD PRIMARY KEY (user_id, conversation_id, tool_use_id);
    END IF;
END $$;
