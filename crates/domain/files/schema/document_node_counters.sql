CREATE TABLE IF NOT EXISTS document_node_counters (
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    writer TEXT NOT NULL CHECK (length(writer) > 0),
    next BIGINT NOT NULL CHECK (next > 0),
    PRIMARY KEY (document_id, writer)
);
