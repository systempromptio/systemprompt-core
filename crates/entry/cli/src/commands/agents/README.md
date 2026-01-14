# Agents CLI Commands

This document provides complete documentation for AI agents to use the agents CLI commands. All commands support non-interactive mode for automation.

---

## Prerequisites

```bash
export SYSTEMPROMPT_PROFILE=/var/www/html/tyingshoelaces/.systemprompt/profiles/local/profile.yaml
cd /var/www/html/systemprompt-core
cargo build --package systemprompt-cli

alias sp="./target/debug/systemprompt --non-interactive"
```

---

## Command Reference

| Command | Description | Artifact Type | Requires Services |
|---------|-------------|---------------|-------------------|
| `agents list` | List configured agents | `Table` | No |
| `agents show <name>` | Display agent configuration | `Card` | No |
| `agents validate [name]` | Check agent configs for errors | `Table` | No |
| `agents create` | Create new agent | `Text` | No |
| `agents edit <name>` | Edit agent configuration | `Text` | No |
| `agents delete <name>` | Delete an agent | `Text` | No |
| `agents status [name]` | Show agent process status | `Table` | No |
| `agents logs [name]` | View agent logs | `Text`/`List` | No |
| `agents registry` | Get running agents from gateway | `Table` | Yes |
| `agents message <agent>` | Send A2A message to agent | `Card` | Yes |
| `agents task <agent>` | Get task details and response | `Card` | Yes |

---

## Configuration Commands (No Running Services Required)

### agents list

List all configured agents from the services configuration.

```bash
sp agents list
sp --json agents list
sp agents list --enabled
sp agents list --disabled
```

**Flags:**
| Flag | Description |
|------|-------------|
| `--enabled` | Show only enabled agents |
| `--disabled` | Show only disabled agents |

**Output Structure:**
```json
{
  "agents": [
    {
      "name": "primary",
      "display_name": "Primary Agent",
      "port": 8001,
      "enabled": true,
      "is_primary": true,
      "is_default": false
    }
  ]
}
```

**Artifact Type:** `Table`
**Columns:** `name`, `display_name`, `port`, `enabled`, `is_primary`, `is_default`

---

### agents show

Display detailed configuration for a specific agent.

```bash
sp agents show <agent-name>
sp --json agents show primary
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<name>` | Yes | Agent name to show |

**Output Structure:**
```json
{
  "name": "primary",
  "display_name": "Primary Agent",
  "description": "Main conversational agent",
  "port": 8001,
  "endpoint": "/api/v1/agents/primary",
  "enabled": true,
  "provider": "anthropic",
  "model": "claude-3-5-sonnet-20241022",
  "mcp_servers": ["filesystem", "database"],
  "skills_count": 3
}
```

**Artifact Type:** `Card`

---

### agents validate

Check agent configurations for errors and warnings.

```bash
sp agents validate
sp --json agents validate
sp agents validate <agent-name>
```

**Output Structure:**
```json
{
  "valid": true,
  "agents_checked": 3,
  "issues": [
    {
      "agent": "test-agent",
      "severity": "warning",
      "message": "Description is empty"
    }
  ]
}
```

**Severity Levels:**
- `error` - Configuration is invalid, agent will not work
- `warning` - Configuration issue but agent may work

**Artifact Type:** `Table`

---

### agents create

Create a new agent configuration.

```bash
sp agents create \
  --name "my-agent" \
  --port 8099 \
  --display-name "My Agent" \
  --description "A custom agent" \
  --provider anthropic \
  --model claude-3-5-sonnet-20241022 \
  --enabled
```

**Required Flags (non-interactive):**
| Flag | Required | Description |
|------|----------|-------------|
| `--name` | Yes | Agent identifier (3-50 chars, lowercase alphanumeric + hyphens) |
| `--port` | Yes | Port number (>= 1024) |

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--display-name` | Same as name | Human-readable name |
| `--description` | Empty | Agent description |
| `--provider` | `anthropic` | AI provider |
| `--model` | `claude-3-5-sonnet-20241022` | AI model |
| `--enabled` | `false` | Enable agent after creation |

**Validation Rules:**
- Name: 3-50 characters, lowercase alphanumeric with hyphens only
- Port: Must be >= 1024 (non-privileged), non-zero

**Output Structure:**
```json
{
  "name": "my-agent",
  "message": "Agent 'my-agent' created successfully at /path/to/agent.yaml"
}
```

**Artifact Type:** `Text`

---

### agents edit

Edit an existing agent configuration.

```bash
sp agents edit <agent-name> --enable
sp agents edit <agent-name> --disable
sp agents edit <agent-name> --port 8098
sp agents edit <agent-name> --provider openai --model gpt-4
sp agents edit <agent-name> --set card.description="New description"
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<name>` | Yes | Agent name to edit |
| At least one change | Yes | Must specify at least one modification |

**Modification Flags:**
| Flag | Description |
|------|-------------|
| `--enable` | Enable the agent |
| `--disable` | Disable the agent |
| `--port <port>` | Change port number |
| `--provider <provider>` | Change AI provider |
| `--model <model>` | Change AI model |
| `--set <key=value>` | Set arbitrary config value |

**Supported --set Keys:**
- `card.displayName` or `card.display_name`
- `card.description`
- `card.version`
- `endpoint`
- `is_primary` (boolean)
- `default` (boolean)
- `dev_only` (boolean)

**Output Structure:**
```json
{
  "name": "my-agent",
  "message": "Agent 'my-agent' updated successfully with 2 change(s)",
  "changes": [
    "enabled: true",
    "port: 8098"
  ]
}
```

**Artifact Type:** `Text`

---

### agents delete

Delete an agent configuration.

```bash
sp agents delete <agent-name> --yes
sp agents delete --all --yes
```

**Required Flags (non-interactive):**
| Flag | Required | Description |
|------|----------|-------------|
| `<name>` | Yes* | Agent name (*unless using --all) |
| `--yes` / `-y` | Yes | Skip confirmation (REQUIRED in non-interactive mode) |

**Optional Flags:**
| Flag | Description |
|------|-------------|
| `--all` | Delete all agents |

**Output Structure:**
```json
{
  "deleted": ["my-agent"],
  "message": "Agent 'my-agent' deleted successfully"
}
```

**Artifact Type:** `Text`

---

### agents status

Show agent process status (running state, PID, port).

```bash
sp agents status
sp --json agents status
sp agents status <agent-name>
```

**Output Structure:**
```json
{
  "agents": [
    {
      "name": "primary",
      "enabled": true,
      "is_running": true,
      "pid": 12345,
      "port": 8001
    }
  ]
}
```

**Artifact Type:** `Table`
**Columns:** `name`, `enabled`, `is_running`, `pid`, `port`

---

### agents logs

View agent logs from database or disk.

```bash
sp agents logs <agent-name>
sp agents logs <agent-name> --lines 100
sp agents logs <agent-name> --disk
sp agents logs <agent-name> --follow
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `<agent>` | - | Agent name (lists available files if not specified) |
| `--lines`, `-n` | 50 | Number of lines to show |
| `--disk` | false | Force reading from disk files |
| `--follow`, `-f` | false | Follow log output continuously |
| `--logs-dir` | `/var/www/html/tyingshoelaces/logs` | Custom logs directory |

**Output Structure (with agent):**
```json
{
  "agent": "primary",
  "source": "database",
  "logs": [
    "2024-01-15 10:30:00 INFO [agent] Starting...",
    "2024-01-15 10:30:01 INFO [agent] Ready"
  ],
  "log_files": []
}
```

**Output Structure (list available):**
```json
{
  "agent": null,
  "source": "disk",
  "logs": [],
  "log_files": [
    "agent-primary.log",
    "agent-secondary.log"
  ]
}
```

**Artifact Type:** `Text` (with agent) or `List` (available files)

---

## A2A Protocol Commands (Requires Running Services)

These commands interact with running agent services via the A2A protocol.

### agents registry

Discover running agents from the gateway registry endpoint.

```bash
sp agents registry
sp --json agents registry
sp agents registry --url http://localhost:8080
sp agents registry --running
sp agents registry --verbose
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--url` | `http://localhost:8080` | Gateway URL |
| `--running` | false | Show only running agents |
| `--verbose` | false | Include full agent details |

**Registry Endpoint:** `GET {gateway}/api/v1/agents/registry`

**Output Structure:**
```json
{
  "gateway_url": "http://localhost:8080",
  "agents_count": 2,
  "agents": [
    {
      "name": "primary",
      "description": "Primary conversational agent",
      "url": "http://localhost:8080/api/v1/agents/primary",
      "version": "1.0.0",
      "status": "running",
      "streaming": true,
      "skills_count": 5,
      "skills": []
    }
  ]
}
```

**Artifact Type:** `Table`
**Columns:** `name`, `url`, `status`, `version`, `streaming`, `skills_count`

---

### agents message

Send a message to a running agent via the A2A protocol. Returns a task ID for tracking.

```bash
sp agents message <agent-name> -m "Hello, how can you help me?"
sp agents message primary -m "What tools do you have?" --blocking
sp agents message primary -m "Search for files" --stream
sp agents message primary -m "Continue task" --context-id <ctx-id> --task-id <task-id>
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<agent>` | Yes | Agent name to message |
| `-m`, `--message` | Yes | Message text to send |

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--url` | `http://localhost:8080` | Gateway URL |
| `--context-id` | Auto-generated UUID | Context ID for conversation |
| `--task-id` | None | Task ID to continue existing task |
| `--stream` | false | Use streaming mode |
| `--blocking` | false | Wait for task completion |
| `--timeout` | 30 | Timeout in seconds for blocking mode |

**A2A Protocol Details:**

The command sends a JSON-RPC 2.0 request to the agent endpoint:

```json
{
  "jsonrpc": "2.0",
  "method": "message/send",
  "params": {
    "message": {
      "role": "user",
      "parts": [{ "kind": "text", "text": "Your message here" }],
      "messageId": "<uuid>",
      "contextId": "<uuid>",
      "kind": "message"
    }
  },
  "id": "<request-uuid>"
}
```

**Output Structure:**
```json
{
  "agent": "primary",
  "task": {
    "task_id": "task_abc123",
    "context_id": "ctx_xyz789",
    "state": "completed",
    "timestamp": "2024-01-15T10:30:00Z"
  },
  "message_sent": "Hello, how can you help me?",
  "artifacts_count": 1
}
```

**Task States:**
| State | Description |
|-------|-------------|
| `pending` | Task awaiting processing |
| `submitted` | Task submitted, awaiting execution |
| `working` | Agent actively processing |
| `completed` | Task successfully completed |
| `failed` | Task failed with error |
| `canceled` | Task was canceled |
| `rejected` | Task rejected by agent |
| `input-required` | Waiting for user input |
| `auth-required` | Authentication needed |

**Artifact Type:** `Card`

---

### agents task

Get task details including conversation history and agent response.

```bash
sp agents task <agent-name> --task-id <task-id> --token "$TOKEN"
sp --json agents task primary --task-id task_abc123 --token "$TOKEN"
sp agents task admin --task-id "$TASK_ID" --history-length 10 --token "$TOKEN"
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<agent>` | Yes | Agent name that processed the task |
| `--task-id` | Yes | Task ID from message response |
| `--token` | Yes | Bearer token for authentication |

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--history-length` | All | Number of history messages to retrieve |
| `--url` | `http://localhost:8080` | Gateway URL |
| `--timeout` | 30 | Timeout in seconds |

**A2A Spec Compliance:** Per A2A spec Section 7.3, `tasks/get` uses `TaskQueryParams` which only requires `id` (task UUID). The context is resolved from task storage by the server.

**Output Structure:**
```json
{
  "task_id": "task_abc123",
  "context_id": "ctx_xyz789",
  "state": "completed",
  "timestamp": "2024-01-15T10:30:00Z",
  "history": [
    { "role": "User", "text": "What is 2+2?" },
    { "role": "Agent", "text": "2 + 2 equals 4." }
  ],
  "artifacts": []
}
```

**Artifact Type:** `Card`

---

## Complete CRUD Flow Example

This flow demonstrates the full lifecycle of agent management:

```bash
# Phase 1: List existing agents
sp --json agents list

# Phase 2: Validate configuration
sp --json agents validate

# Phase 3: Create new agent
sp agents create \
  --name "test-agent" \
  --port 8199 \
  --display-name "Test Agent" \
  --description "Created for testing" \
  --provider anthropic \
  --model claude-3-5-sonnet-20241022

# Phase 4: Verify creation
sp --json agents show test-agent

# Phase 5: Edit agent
sp agents edit test-agent --enable --port 8198

# Phase 6: Validate after edit
sp --json agents validate test-agent

# Phase 7: Check status
sp --json agents status test-agent

# Phase 8: View logs (after agent runs)
sp agents logs test-agent --lines 20

# Phase 9: Delete agent
sp agents delete test-agent --yes

# Phase 10: Verify deletion
sp --json agents list
```

---

## A2A Communication Flow

This flow demonstrates interacting with running agents with authentication:

```bash
# Step 1: Start services (in tyingshoelaces repo)
cd /var/www/html/tyingshoelaces && just start

# Step 2: Get authentication token
TOKEN=$(sp system login --email your-admin@email.com --token-only)
# Verify token was captured
echo "Token length: ${#TOKEN}"

# Step 3: Discover available agents
sp --json agents registry --running

# Step 4: Send initial message to agent (auto-creates context)
RESPONSE=$(sp --json agents message admin -m "What is 2+2?" --token "$TOKEN" --blocking)
echo "$RESPONSE"
# Extract task_id and context_id from response (note: JSON output wraps data)
TASK_ID=$(echo "$RESPONSE" | jq -r '.data.task.task_id')
CONTEXT_ID=$(echo "$RESPONSE" | jq -r '.data.task.context_id')

# Step 5: Get task details with agent response (no --context-id needed per A2A spec)
sp agents task admin --task-id "$TASK_ID" --token "$TOKEN"
# Returns structured history and artifacts in JSON output

# Step 6: Continue conversation (use context_id from previous response)
sp --json agents message admin \
  -m "Now multiply that by 10" \
  --context-id "$CONTEXT_ID" \
  --token "$TOKEN" \
  --blocking

# Step 7: Get full conversation history
sp agents task admin --task-id "$TASK_ID" --token "$TOKEN"
```

### Authentication

A2A protocol commands require authentication. Use `system login` to get a token:

```bash
# Get token interactively
sp system login --email admin@example.com

# Get token for scripting (outputs only the token)
TOKEN=$(sp system login --email admin@example.com --token-only)

# Use token with message command
sp agents message admin -m "Hello" --token "$TOKEN"

# Or set as environment variable
export SYSTEMPROMPT_TOKEN="$TOKEN"
sp agents message admin -m "Hello"  # Uses env var automatically
```

The token is a JWT with 24-hour default expiration. Use `--duration-hours` to customize.

---

## Output Type Summary

| Command | Return Type | Artifact Type | Metadata |
|---------|-------------|---------------|----------|
| `list` | `AgentListOutput` | `Table` | columns |
| `show` | `AgentDetailOutput` | `Card` | title |
| `validate` | `ValidationOutput` | `Table` | - |
| `create` | `AgentCreateOutput` | `Text` | title |
| `edit` | `AgentEditOutput` | `Text` | title |
| `delete` | `AgentDeleteOutput` | `Text` | title |
| `status` | `AgentStatusOutput` | `Table` | columns |
| `logs` | `AgentLogsOutput` | `Text`/`List` | title |
| `registry` | `RegistryOutput` | `Table` | columns |
| `message` | `MessageOutput` | `Card` | title |
| `task` | `TaskGetOutput` | `Card` | title |

---

## Error Handling

### Missing Required Flags

```bash
sp agents show
# Error: --name is required in non-interactive mode

sp agents delete test-agent
# Error: --yes is required to delete agents in non-interactive mode

sp agents create --name test
# Error: --port is required in non-interactive mode

sp agents message primary
# Error: Message text is required. Use -m or --message
```

### Validation Errors

```bash
sp agents create --name "Test Agent" --port 8099
# Error: Agent name must be lowercase alphanumeric with hyphens only

sp agents create --name "ab" --port 8099
# Error: Agent name must be between 3 and 50 characters

sp agents create --name "test-agent" --port 80
# Error: Port must be >= 1024 (non-privileged)
```

### Not Found Errors

```bash
sp agents show nonexistent
# Error: Agent 'nonexistent' not found

sp agents message nonexistent -m "test"
# Error: Failed to send message to agent at http://localhost:8080/api/v1/agents/nonexistent
```

### Service Connection Errors

```bash
sp agents registry
# Error: Failed to connect to gateway at http://localhost:8080/api/v1/agents/registry

sp agents message primary -m "test"
# Error: Failed to send message to agent at http://localhost:8080/api/v1/agents/primary
```

---

## JSON Output

All commands support `--json` flag for structured output:

```bash
# Verify JSON is valid
sp --json agents list | jq .

# Extract specific fields (JSON output wraps data in .data)
sp --json agents list | jq '.data.agents[].name'
sp --json agents show primary | jq '.data.port'
sp --json agents validate | jq '.data.valid'
sp --json agents status | jq '.data.agents[] | select(.is_running == true)'
sp --json agents registry | jq '.data.agents[] | select(.status == "running")'
sp --json agents message primary -m "test" --token "$TOKEN" | jq '.data.task.task_id'
```

---

## Compliance Checklist

- [x] All `execute` functions accept `config: &CliConfig`
- [x] All commands return `CommandResult<T>` with proper artifact type
- [x] `delete` command requires `--yes` / `-y` flag
- [x] All output types derive `Serialize`, `Deserialize`, `JsonSchema`
- [x] No `println!` / `eprintln!` - uses `CliService`
- [x] No `unwrap()` / `expect()` - uses `?` with `.context()`
- [x] `resolve_input` pattern used for interactive/non-interactive selection
- [x] JSON output supported via `--json` flag
- [x] Proper error messages for missing required flags
