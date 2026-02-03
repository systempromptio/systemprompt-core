# systemprompt.io OS - Lean Justfile
# Use CLI directly with global flags: --json, --verbose, --debug, --no-color

# Show all commands
default:
    @just --list

# =============================================================================
# BUILD & TEST
# =============================================================================

# Build workspace
build:
    cargo build --workspace

# Build workspace offline (uses cached .sqlx metadata, no database required)
build-offline:
    SQLX_OFFLINE=true cargo build --workspace

# Build CLI only
cli:
    cargo build --bin systemprompt

# Build CLI offline (uses cached .sqlx metadata, no database required)
cli-offline:
    SQLX_OFFLINE=true cargo build --bin systemprompt

# Prepare sqlx offline cache (requires running database)
sqlx-prepare:
    cargo sqlx prepare --workspace

# Prepare per-crate SQLx caches for publishing (requires running database)
sqlx-prepare-publish:
    #!/usr/bin/env bash
    set -e
    echo "Generating per-crate .sqlx directories for crates.io publishing..."
    echo ""
    for crate in crates/infra/database crates/infra/logging crates/domain/analytics \
                 crates/domain/agent crates/domain/oauth crates/domain/users \
                 crates/domain/content crates/domain/files crates/domain/ai \
                 crates/domain/mcp crates/app/scheduler crates/app/sync \
                 crates/entry/cli crates/entry/api; do
        echo "  Preparing $crate..."
        (cd "$crate" && cargo sqlx prepare)
    done
    echo ""
    echo "Done! Commit the .sqlx directories before publishing:"
    echo "  git add crates/*/.sqlx"
    echo "  git commit -m 'chore: update SQLx cache for release'"

# Verify packages can be built offline (pre-publish check)
sqlx-verify-offline:
    #!/usr/bin/env bash
    set -e
    echo "Verifying offline compilation for all SQLx crates..."
    echo ""
    for crate in systemprompt-database systemprompt-logging systemprompt-analytics \
                 systemprompt-agent systemprompt-oauth systemprompt-users \
                 systemprompt-content systemprompt-files systemprompt-ai \
                 systemprompt-mcp systemprompt-scheduler systemprompt-sync \
                 systemprompt-cli systemprompt-api; do
        echo "  Checking $crate..."
        SQLX_OFFLINE=true cargo package -p "$crate" --allow-dirty 2>&1 | tail -1
    done
    echo ""
    echo "All crates verified for offline compilation!"

# Check without building
check:
    cargo check --workspace

# Check offline (uses cached .sqlx metadata, no database required)
check-offline:
    SQLX_OFFLINE=true cargo check --workspace

# Format code
fmt:
    cargo fmt --all

# Check formatting without making changes
format-check:
    cargo fmt --all -- --check

# Run clippy linter with strict settings
lint:
    cargo clippy --workspace -- -D warnings

# Run custom style validators
validate:
    ./tests/validator/validate.sh

# Run all style checks (format + lint + validate)
style-check:
    #!/usr/bin/env bash
    set -e
    echo "🎨 Running style checks..."
    echo ""
    echo "1️⃣  Checking code formatting..."
    cargo fmt --all -- --check
    echo ""
    echo "2️⃣  Running clippy linter..."
    cargo clippy --workspace -- -D warnings
    echo ""
    echo "3️⃣  Running custom validators..."
    ./tests/validator/validate.sh
    echo ""
    echo "✅ All style checks passed!"

# Clean build artifacts
clean:
    cargo clean

# =============================================================================
# SERVICES
# =============================================================================

# Start API server (checks if already running)
api:
    #!/usr/bin/env bash
    # Check if server is already running on port 8080
    if lsof -ti :8080 >/dev/null 2>&1; then
        echo "✅ Server already running on port 8080"
        echo ""
        echo "💡 To restart with latest code: just api-rebuild"
        exit 0
    fi

    echo "🚀 Starting API server..."
    ./target/debug/systemprompt serve api --foreground

# Rebuild and restart entire system (API + agents + MCP)
api-rebuild:
    #!/usr/bin/env bash
    set -e

    echo "🔨 Building..."
    cargo build --bin systemprompt

    echo "🧹 Cleaning up services..."
    ./target/debug/systemprompt cleanup-services

    echo "✅ Starting fresh API server..."
    ./target/debug/systemprompt serve api --foreground

# Convenient alias for api-rebuild
restart:
    just api-rebuild

# Build and start API server with TEST database (for integration tests)
api-test-rebuild:
    #!/usr/bin/env bash
    set -e

    echo "🔨 Building..."
    cargo build --bin systemprompt

    echo "🧹 Cleaning up services..."
    ./target/debug/systemprompt cleanup-services

    echo "✅ Starting fresh API server with TEST database..."
    export DATABASE_URL="database/test.db"
    ./target/debug/systemprompt serve api --foreground

# Reload agents with latest binary (keeps API server running)
agents-reload:
    #!/usr/bin/env bash
    set +e  # Don't exit on errors

    echo "🔨 Building latest binary..."
    cargo build --bin systemprompt

    echo "🧹 Stopping old agent processes..."

    # Kill agent processes on known ports
    for port in 9000 9001 9002 9003; do
        lsof -ti :$port 2>/dev/null | xargs -r kill -9 2>/dev/null || true
    done

    # Kill all agent processes by name
    pkill -9 -f "systemprompt admin agents run" 2>/dev/null || true
    pkill -9 -f "systemprompt-admin" 2>/dev/null || true
    pkill -9 -f "systemprompt-introduction" 2>/dev/null || true
    pkill -9 -f "systemprompt-helper" 2>/dev/null || true

    echo "⏳ Waiting for processes to terminate..."
    sleep 2

    echo "🚀 Starting agents with new binary via API reconciliation..."

    # Trigger API to restart all enabled agents
    ./target/debug/systemprompt admin agents restart --all 2>/dev/null || echo "Note: Agents will auto-start with API"

    echo "✅ Agents reloaded with latest binary"
    echo ""
    echo "💡 Check status: just agents"

# Nuclear option: kill everything and reset (API, agents, MCP servers, database)
api-nuke:
    #!/usr/bin/env bash
    set +e  # Don't exit on errors

    echo "🔨 Building..."
    cargo build --bin systemprompt

    echo "💥 NUKING ALL PROCESSES..."

    # Kill all processes on service ports (API, agents, MCP servers)
    for port in 8080 9000 9001 9002 9003 5000 5001 5002 5003 5004 5005; do
        lsof -ti :$port 2>/dev/null | xargs -r kill -9 2>/dev/null || true
    done

    # Kill any remaining systemprompt service processes by name
    pkill -9 -f "systemprompt serve api" 2>/dev/null || true
    pkill -9 -f "systemprompt admin agents run" 2>/dev/null || true
    pkill -9 -f "systemprompt-admin" 2>/dev/null || true
    pkill -9 -f "systemprompt-introduction" 2>/dev/null || true
    pkill -9 -f "systemprompt-helper" 2>/dev/null || true

    # Clean up any zombie processes
    pkill -9 -f "systemprompt" 2>/dev/null || true

    # Give processes time to fully terminate
    sleep 1

    # Clean up services database (remove crashed/orphaned entries)
    ./target/debug/systemprompt infra db execute "DELETE FROM services" 2>/dev/null || true

    echo "✅ Nuclear cleanup complete, starting fresh API server..."

    # Start fresh
    ./target/debug/systemprompt serve api --foreground


# =============================================================================
# TESTING
# =============================================================================

# Initialize test database (REQUIRED before running tests)
test-setup:
    #!/usr/bin/env bash
    set -e
    echo "🧪 Initializing test database..."
    echo ""
    tests/integration/scripts/setup-test-db.sh

# Run integration tests with test database (AUTOMATED)
test-run:
    #!/usr/bin/env bash
    set -e
    echo "🧪 Running integration tests..."
    echo ""
    echo "⚠️  MAKE SURE API IS RUNNING IN ANOTHER TERMINAL:"
    echo "   In another terminal, run: just api-test"
    echo ""
    echo "Press Enter to continue or Ctrl+C to abort..."
    read
    cd tests/integration
    export DATABASE_URL="database/test.db"
    npm test

# Start API server with test database (for integration tests)
api-test:
    #!/usr/bin/env bash
    echo "🧪 Starting API server with TEST database..."
    echo "📝 Database: database/test.db"
    echo ""
    export DATABASE_URL="database/test.db"
    ./target/debug/systemprompt serve api --foreground

# Run full test workflow: setup DB → start API → run tests
test-full:
    #!/usr/bin/env bash
    set -e
    echo "🧪 FULL TEST WORKFLOW"
    echo "═══════════════════════════════════════════════════════════"
    echo ""

    echo "Step 1️⃣  Initializing test database..."
    tests/integration/scripts/setup-test-db.sh
    echo ""

    echo "Step 2️⃣  Building project..."
    cargo build --bin systemprompt
    echo ""

    echo "⚠️  Step 3️⃣  Starting API in background with test database..."
    export DATABASE_URL="database/test.db"
    ./target/debug/systemprompt serve api --foreground &
    API_PID=$!

    # Give API time to start
    echo "   Waiting for API to start..."
    sleep 3

    # Check if API is running
    if ! lsof -ti :8080 >/dev/null 2>&1; then
        echo "❌ API failed to start!"
        exit 1
    fi
    echo "✅ API started (PID: $API_PID)"
    echo ""

    # Run tests
    echo "Step 4️⃣  Running tests..."
    cd tests/integration
    export DATABASE_URL="database/test.db"
    if npm test; then
        TEST_EXIT=0
    else
        TEST_EXIT=$?
    fi

    # Cleanup
    echo ""
    echo "🧹 Cleaning up..."
    kill $API_PID 2>/dev/null || true
    wait $API_PID 2>/dev/null || true
    sleep 1

    echo "═══════════════════════════════════════════════════════════"
    if [ $TEST_EXIT -eq 0 ]; then
        echo "✅ All tests passed!"
        exit 0
    else
        echo "❌ Tests failed!"
        exit $TEST_EXIT
    fi

# Clean test database (remove test data)
test-clean:
    #!/usr/bin/env bash
    echo "🧹 Cleaning test database..."
    echo ""
    export DATABASE_URL="database/test.db"
    ./target/debug/systemprompt infra db execute "DELETE FROM task_artifacts WHERE artifact_id LIKE 'test-%' OR created_by LIKE 'test-%'"
    ./target/debug/systemprompt infra db execute "DELETE FROM user_contexts WHERE context_id LIKE 'test-%'"
    ./target/debug/systemprompt infra db execute "DELETE FROM user_sessions WHERE session_id LIKE 'test-%'"
    ./target/debug/systemprompt infra db execute "DELETE FROM ai_requests WHERE session_id LIKE 'test-%'"
    echo "✅ Test data cleaned!"

# Reset test database completely (use with caution!)
test-reset:
    #!/usr/bin/env bash
    echo "🧹 Resetting test database completely..."
    echo "⚠️  This will delete ALL data from test database!"
    echo "Press Enter to continue or Ctrl+C to abort..."
    read
    rm -f database/test.db
    just test-setup

# Show test database info
test-info:
    #!/usr/bin/env bash
    export DATABASE_URL="database/test.db"
    echo "📊 Test Database Information:"
    echo ""
    ./target/debug/systemprompt infra db info --verbose || echo "Database not initialized. Run: just test-setup"

# Stream logs from test database (newest logs at bottom, chronological order)
test-logs:
    #!/usr/bin/env bash
    export DATABASE_URL="database/test.db"
    echo "📋 Streaming logs from test database (chronological order, newest at bottom)..."
    echo "════════════════════════════════════════════════════════════"
    echo ""
    ./target/debug/systemprompt infra db query "SELECT timestamp, level, module, message, context_id, trace_id FROM logs ORDER BY timestamp ASC LIMIT 1000" --format table || echo "No logs found in test database"

# Stream only errors and warnings from test database
test-logs-errors:
    #!/usr/bin/env bash
    export DATABASE_URL="database/test.db"
    echo "📋 Streaming ERROR and WARN logs from test database..."
    echo "════════════════════════════════════════════════════════════"
    echo ""
    ./target/debug/systemprompt infra db query "SELECT timestamp, level, module, message, context_id, trace_id FROM logs WHERE level IN ('ERROR', 'WARN') ORDER BY timestamp ASC LIMIT 1000" --format table || echo "No errors/warnings found in test database"

# Stream debug logs from test database
test-logs-debug:
    #!/usr/bin/env bash
    export DATABASE_URL="database/test.db"
    echo "📋 Streaming DEBUG logs from test database..."
    echo "════════════════════════════════════════════════════════════"
    echo ""
    ./target/debug/systemprompt infra db query "SELECT timestamp, level, module, message, context_id, trace_id FROM logs WHERE level = 'DEBUG' ORDER BY timestamp ASC LIMIT 1000" --format table || echo "No debug logs found in test database"

# =============================================================================
# OPERATIONS
# =============================================================================

# List agents (use --json, --verbose flags as needed)
agents:
    ./target/debug/systemprompt admin agents list

# Agent orchestrator operations (alias for agents command)
a2a *ARGS:
    ./target/debug/systemprompt admin agents {{ARGS}}

# MCP server operations
mcp *ARGS:
    ./target/debug/systemprompt plugins mcp {{ARGS}}

# Tenant management (create, list, show, edit, delete)
tenant *ARGS:
    ./target/debug/systemprompt cloud tenant {{ARGS}}

# Profile management (create, list, show, edit, delete)
profile *ARGS:
    ./target/debug/systemprompt cloud profile {{ARGS}}

# Database operations (pass subcommand, e.g., 'just db migrate' or 'just db tables')
# IMPORTANT: For queries with commas/spaces, use 'just query "SQL"' instead of 'just db query "SQL"'
# Use --test flag to operate on test database: just db migrate --test
db *ARGS:
    #!/usr/bin/env bash
    set -- {{ARGS}}  # Convert justfile args to bash positional params

    # Check if trying to use 'db query' with complex SQL
    if [[ "$1" == "query" ]] && [[ "$#" -gt 2 ]]; then
        echo "⚠️  ERROR: Use 'just query \"SQL\"' for queries with commas/spaces"
        echo "   Current: just db query {{ARGS}}"
        echo "   Correct: just query \"YOUR_SQL_HERE\""
        exit 1
    fi

    if [[ " $* " =~ " --test " ]]; then
        # Remove --test flag from args and set DATABASE_URL for test db
        ARGS_WITHOUT_TEST=()
        for arg in "$@"; do
            [[ "$arg" != "--test" ]] && ARGS_WITHOUT_TEST+=("$arg")
        done
        echo "📝 Using test database: database/test.db"
        DATABASE_URL="database/test.db" ./target/debug/systemprompt infra db "${ARGS_WITHOUT_TEST[@]}"
    else
        ./target/debug/systemprompt infra db "$@"
    fi

# Execute database query (supports table, json, or csv format)
query SQL FORMAT="table":
    #!/usr/bin/env bash
    if [[ "{{FORMAT}}" == "json" ]]; then
        ./target/debug/systemprompt infra db query "{{SQL}}" --format json
    elif [[ "{{FORMAT}}" == "csv" ]]; then
        ./target/debug/systemprompt infra db query "{{SQL}}" --format csv
    else
        ./target/debug/systemprompt infra db query "{{SQL}}"
    fi

# Sync skills to Claude Code
skills:
    ./target/debug/systemprompt core skills

# Stream logs (use --level, --module flags to filter)
logs *ARGS:
    ./target/debug/systemprompt infra logs --stream {{ARGS}}

# Stream logs (alias for logs)
log *ARGS:
    ./target/debug/systemprompt infra logs --stream {{ARGS}}

# Trace a request flow by trace_id (shows execution steps, logs, artifacts)
trace TRACE_ID:
    #!/usr/bin/env bash
    echo "============================================================"
    echo "TRACE: {{TRACE_ID}}"
    echo "============================================================"
    echo ""

    # Get task info first
    echo "📋 TASK INFO"
    echo "------------------------------------------------------------"
    ./target/debug/systemprompt infra db query "SELECT task_id, context_id, agent_name, status, execution_time_ms, created_at FROM agent_tasks WHERE trace_id = '{{TRACE_ID}}'" || echo "No task found"
    echo ""

    # Execution steps with lifecycle transitions
    echo "🔄 EXECUTION STEPS"
    echo "------------------------------------------------------------"
    ./target/debug/systemprompt infra db query "SELECT s.step_type, s.title, s.subtitle, s.status, s.duration_ms, s.tool_name, s.started_at FROM task_execution_steps s JOIN agent_tasks t ON s.task_id = t.task_id WHERE t.trace_id = '{{TRACE_ID}}' ORDER BY s.started_at" || echo "No execution steps found"
    echo ""

    # Logs (INFO and above, skip DEBUG)
    echo "📝 LOGS (INFO+)"
    echo "------------------------------------------------------------"
    ./target/debug/systemprompt infra db query "SELECT timestamp, level, module, message FROM logs WHERE trace_id = '{{TRACE_ID}}' AND level != 'DEBUG' ORDER BY timestamp" || echo "No logs found"
    echo ""

    # Artifacts
    echo "📦 ARTIFACTS"
    echo "------------------------------------------------------------"
    ./target/debug/systemprompt infra db query "SELECT ta.artifact_id, ta.name, ta.artifact_type, ta.skill_name, ta.created_at FROM task_artifacts ta JOIN agent_tasks t ON ta.task_id = t.task_id WHERE t.trace_id = '{{TRACE_ID}}' ORDER BY ta.created_at" || echo "No artifacts found"
    echo ""
    echo "============================================================"

# Generate admin token
token:
    ./target/debug/systemprompt login admin

# Generate admin token (alias for token)
admin-token:
    ./target/debug/systemprompt login admin

# Assign admin role to a user (by username or email)
assign-admin USER:
    ./target/debug/systemprompt infra db assign-admin {{USER}}

# =============================================================================
# POSTGRESQL (Docker Database)
# =============================================================================

# Start PostgreSQL test/development container (port 5433)
postgres-start:
    #!/usr/bin/env bash
    if docker ps -a --format '{{{{.Names}}' | grep -q '^systemprompt-postgres-test$'; then
        if docker ps --format '{{{{.Names}}' | grep -q '^systemprompt-postgres-test$'; then
            echo "✅ PostgreSQL test container already running on port 5433"
        else
            echo "🚀 Starting existing PostgreSQL test container..."
            docker start systemprompt-postgres-test
            sleep 2
            echo "✅ PostgreSQL test container started on port 5433"
        fi
    else
        echo "🚀 Creating and starting PostgreSQL test container..."
        docker compose up -d postgres-test
        sleep 3
        echo "✅ PostgreSQL test container started on port 5433"
    fi
    echo ""
    echo "📝 Connection: postgresql://systemprompt_test:systemprompt_test_password@127.0.0.1:5433/systemprompt_test"

# Start PostgreSQL production container (port 5432)
postgres-start-prod:
    #!/usr/bin/env bash
    if docker ps -a --format '{{{{.Names}}' | grep -q '^systemprompt-postgres-prod$'; then
        if docker ps --format '{{{{.Names}}' | grep -q '^systemprompt-postgres-prod$'; then
            echo "✅ PostgreSQL prod container already running on port 5432"
        else
            echo "🚀 Starting existing PostgreSQL prod container..."
            docker start systemprompt-postgres-prod
            sleep 2
            echo "✅ PostgreSQL prod container started on port 5432"
        fi
    else
        echo "🚀 Creating and starting PostgreSQL prod container..."
        docker compose up -d postgres-prod
        sleep 3
        echo "✅ PostgreSQL prod container started on port 5432"
    fi
    echo ""
    echo "📝 Connection: postgresql://systemprompt:systemprompt_prod_password@127.0.0.1:5432/systemprompt_production"

# Stop all PostgreSQL containers
postgres-stop:
    #!/usr/bin/env bash
    echo "🛑 Stopping PostgreSQL containers..."
    docker stop systemprompt-postgres-test 2>/dev/null || echo "Test container not running"
    docker stop systemprompt-postgres-prod 2>/dev/null || echo "Prod container not running"
    echo "✅ PostgreSQL containers stopped"

# View PostgreSQL logs
postgres-logs SERVICE="test":
    #!/usr/bin/env bash
    if [ "{{SERVICE}}" = "prod" ]; then
        docker logs -f systemprompt-postgres-prod
    else
        docker logs -f systemprompt-postgres-test
    fi

# Connect to PostgreSQL CLI (test database)
postgres-psql:
    #!/usr/bin/env bash
    echo "🔗 Connecting to PostgreSQL test database..."
    PGPASSWORD=systemprompt_test_password psql -h 127.0.0.1 -p 5433 -U systemprompt_test -d systemprompt_test

# Connect to PostgreSQL CLI (production database)
postgres-psql-prod:
    #!/usr/bin/env bash
    echo "🔗 Connecting to PostgreSQL production database..."
    PGPASSWORD=systemprompt_prod_password psql -h 127.0.0.1 -p 5432 -U systemprompt -d systemprompt_production

# Check PostgreSQL container status
postgres-status:
    #!/usr/bin/env bash
    echo "📊 PostgreSQL Container Status:"
    echo ""
    docker ps -a --filter "name=systemprompt-postgres" --format "table {{{{.Names}}\t{{{{.Status}}\t{{{{.Ports}}"

# Remove PostgreSQL containers and volumes (DESTRUCTIVE!)
postgres-nuke:
    #!/usr/bin/env bash
    echo "⚠️  This will DELETE all PostgreSQL containers and data volumes!"
    echo "Press Enter to continue or Ctrl+C to abort..."
    read
    docker stop systemprompt-postgres-test systemprompt-postgres-prod 2>/dev/null || true
    docker rm systemprompt-postgres-test systemprompt-postgres-prod 2>/dev/null || true
    docker volume rm systemprompt-os-rust-2_postgres_test_data systemprompt-os-rust-2_postgres_prod_data 2>/dev/null || true
    echo "✅ PostgreSQL containers and volumes removed"

# =============================================================================
# REMOTE POSTGRESQL (Deployed on GCP)
# =============================================================================

# Connect to remote PostgreSQL via psql
db-connect:
    #!/usr/bin/env bash
    if [ ! -f ".env.remote" ]; then
        echo "❌ .env.remote not found"
        echo "Create .env.remote with DATABASE_URL from systemprompt-db deployment"
        exit 1
    fi
    source .env.remote
    psql "$DATABASE_URL"

# Run migrations on remote PostgreSQL
migrate:
    #!/usr/bin/env bash
    if [ ! -f "../.env.remote" ]; then
        echo "❌ .env.remote not found"
        echo "Create ../.env.remote with DATABASE_URL from systemprompt-db deployment"
        exit 1
    fi
    source ../.env.remote
    echo "Running migrations on remote database..."
    ./target/debug/systemprompt infra db migrate

# Create new site database on remote
db-create-site SITENAME:
    #!/usr/bin/env bash
    if [ ! -f ".env.remote" ]; then
        echo "❌ .env.remote not found"
        exit 1
    fi
    source .env.remote
    echo "Creating database for site: {{SITENAME}}"
    psql "$DATABASE_URL" -c "CREATE DATABASE {{SITENAME}} OWNER app;"
    echo "✅ Database {{SITENAME}} created!"

# List all databases
db-list:
    #!/usr/bin/env bash
    if [ ! -f ".env.remote" ]; then
        echo "❌ .env.remote not found"
        exit 1
    fi
    source .env.remote
    psql "$DATABASE_URL" -c "\l"

# Show database connection statistics
db-stats:
    #!/usr/bin/env bash
    if [ ! -f ".env.remote" ]; then
        echo "❌ .env.remote not found"
        exit 1
    fi
    source .env.remote
    psql "$DATABASE_URL" -c "SELECT datname, count(*) FROM pg_stat_activity GROUP BY datname;"

# =============================================================================
# INFRASTRUCTURE (Docker & Config)
# =============================================================================

# Generate .env from YAML configs
config ENV="docker":
    ./infrastructure/scripts/generate-env.sh --environment {{ENV}}

# Validate configuration
config-validate ENV="docker":
    ./infrastructure/scripts/generate-env.sh --environment {{ENV}} --validate

# Build Docker images from source (cli, mcp, or all)
docker-build TARGET="all":
    ./infrastructure/scripts/build.sh {{TARGET}}

# Build Docker images from pre-built release binaries (fast!)
docker-build-prebuilt:
    cd infrastructure/compose && docker compose -f docker-compose.prebuilt.yml build

# Start Docker services (full build)
docker-up:
    cd infrastructure/compose && docker compose up -d

# Start Docker services (pre-built binaries)
docker-up-prebuilt:
    cd infrastructure/compose && docker compose -f docker-compose.prebuilt.yml up -d

# Stop Docker services
docker-down:
    cd infrastructure/compose && docker compose down
    cd infrastructure/compose && docker compose -f docker-compose.prebuilt.yml down

# View Docker logs (full build)
docker-logs:
    cd infrastructure/compose && docker compose logs -f

# View Docker logs (prebuilt)
docker-logs-prebuilt:
    cd infrastructure/compose && docker compose -f docker-compose.prebuilt.yml logs -f

# Restart Docker services
docker-restart:
    cd infrastructure/compose && docker compose restart

# Full Docker setup from source (config + build + start)
docker-setup ENV="docker":
    just config {{ENV}}
    just docker-build all
    just docker-up

# =============================================================================
# BLOG & CONTENT MANAGEMENT
# =============================================================================

# Ingest markdown files into blog system
ingest-markdown path:
    #!/usr/bin/env bash
    set -e
    if [ ! -d "{{ path }}" ]; then
        echo "❌ Directory not found: {{ path }}"
        exit 1
    fi
    echo "📚 Ingesting markdown files from: {{ path }}"
    cargo run --bin ingest -p systemprompt-core-content -- --path "{{ path }}"

# Quick test: create sample content and ingest
ingest-test:
    bash scripts/rag-ingest-test.sh

# =============================================================================
# WORKFLOWS
# =============================================================================

# Build and start API
dev:
    cargo build --bin systemprompt
    ./target/debug/systemprompt serve api --foreground

# Full system status (unified view of all services)
status:
    ./target/debug/systemprompt status

# Restart failed services
restart-failed:
    ./target/debug/systemprompt restart --failed

# Health check
health:
    ./target/debug/systemprompt admin agents health --all

# =============================================================================
# WEB & BLOG
# =============================================================================
# Web builds are now fully static - no API server required!
# Blog posts are bundled at build time from markdown files

# Build web frontend (static blog + React app)
web-build:
    #!/usr/bin/env bash
    set -e
    echo "🔨 Building web frontend..."
    echo ""

    # Check API is running (needed for blog pre-rendering)
    echo "🔍 Checking if API server is running..."
    if ! curl -s http://localhost:8080/api/v1/content/blog/json >/dev/null 2>&1; then
        echo "❌ ERROR: API server not running on port 8080"
        echo ""
        echo "💡 FIX: Start API server first:"
        echo "   In another terminal: just api"
        echo "   Then run: just web-build"
        exit 1
    fi
    echo "✅ API server is running"

    # Build
    cd web && npm run build
    echo ""
    echo "✅ Web build complete:"
    echo "   - React app: /dist/index.html"
    echo "   - Static blog: /dist/blog/**/index.html"
    echo "   - Sitemap: /dist/sitemap.xml"

# Run web dev server
web-dev:
    cd web && npm run dev

# Generate sitemap (fully static, no API required)
# For local development - uses localhost URLs
web-sitemap:
    #!/usr/bin/env bash
    set -e
    echo "📄 Generating sitemap for local development..."
    cd web && VITE_API_URL=http://localhost:8080 npm run sitemap:generate
    echo "✅ Sitemap generated with localhost URLs (no API required!)"

# Generate sitemap for production deployment
web-sitemap-prod:
    #!/usr/bin/env bash
    set -e
    echo "📄 Generating sitemap for production..."
    cd web && SITEMAP_BASE_URL=https://tyingshoelaces.com npm run sitemap:generate
    echo "✅ Sitemap generated with production URLs (https://tyingshoelaces.com)"

# Ingest all blog content from config (production-ready)
ingest-content:
    #!/usr/bin/env bash
    set -e
    echo "📚 Ingesting content from config.yml..."
    ./target/debug/ingest --config crates/services/content/config.yml

# Ingest specific content source (blog, web, personal, etc.)
ingest-source SOURCE:
    #!/usr/bin/env bash
    set -e
    echo "📚 Ingesting {{ SOURCE }} content..."
    ./target/debug/ingest --config crates/services/content/config.yml --source {{ SOURCE }}

# Seed database from markdown (one-time operation, never updates existing)
seed-content:
    #!/usr/bin/env bash
    set -e
    echo "🌱 Seeding database from markdown files..."
    ./target/debug/ingest --config crates/services/content/config.yml --seed-only
    echo "✨ Seeding complete. Database is now source of truth."

# Ingest all content sources from config
ingest-all: ingest-content

# Sync database content to markdown files (DB → markdown, overwrites)
sync-content:
    #!/usr/bin/env bash
    set -e
    echo "📤 Syncing database content to markdown files..."
    ./target/debug/export --config crates/services/content/config.yml
    echo "✨ Sync complete. Markdown files updated from database."

# =============================================================================
# TIPS
# =============================================================================
# Use the CLI directly with global flags for variations:
#   systemprompt --json agents list
#   systemprompt --verbose db tables
#   systemprompt --debug mcp status
#   systemprompt agents enable my-agent
#   systemprompt db describe agents
#   systemprompt logs --level ERROR --stream

# =============================================================================
# WEBAUTHN
# =============================================================================

# Generate WebAuthn setup token for admin and open registration page
webauthn-admin EMAIL="admin@localhost":
    #!/usr/bin/env bash
    set -e
    echo "🔐 Generating WebAuthn setup token for {{EMAIL}}..."

    # Find the systemprompt binary
    if [ -f "./target/debug/systemprompt" ]; then
        CLI="./target/debug/systemprompt"
    elif [ -f "../systemprompt-template/target/debug/systemprompt" ]; then
        CLI="../systemprompt-template/target/debug/systemprompt"
    elif command -v systemprompt &> /dev/null; then
        CLI="systemprompt"
    else
        echo "❌ systemprompt binary not found. Run 'just build' first."
        exit 1
    fi

    $CLI admin users webauthn generate-setup-token --email "{{EMAIL}}"
