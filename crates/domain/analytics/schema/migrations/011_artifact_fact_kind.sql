-- Admit the `artifact` fact kind: one typed tool result per fact, joined to
-- the invocation and request it belongs to. Producers began emitting it when
-- tool results became first-class artifacts; without this the change queue
-- refuses them at the CHECK.

ALTER TABLE analytics_fact_changes DROP CONSTRAINT IF EXISTS analytics_fact_changes_fact_kind_check;
ALTER TABLE analytics_fact_changes ADD CONSTRAINT analytics_fact_changes_fact_kind_check
    CHECK (fact_kind IN ('invocation','request','assessment','resource_association','artifact'));
