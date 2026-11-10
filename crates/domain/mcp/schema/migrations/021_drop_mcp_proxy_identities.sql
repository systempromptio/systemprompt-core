-- @compat: breaking
-- The MCP proxy authenticates every request at the edge and binds a session to
-- its server and principal in mcp_sessions, so no caller identity or bearer is
-- stored per session.
DROP TABLE IF EXISTS mcp_proxy_identities;
