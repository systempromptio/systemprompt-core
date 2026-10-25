#!/usr/bin/env bash
# Boot a released server against a database the tree under test migrated.
#
#   scripts/schema-ladder-boot.sh <rung-tag> <rung-cli> <database-url>
#
# Run from the checkout under test, after schema-ladder.sh has upgraded
# <database-url> with this tree. <rung-cli> is that release's `systemprompt`
# binary. It plays the pod a rolling update leaves running: it serves with
# `--skip-migrate` while the schema is ahead of it. The expected outcome is
# read from the migrations this tree adds over the rung:
#
#   * none marked `@compat: breaking` — the old server serves, `/readyz`
#     answers 200 (with `"schema": "ahead"` once the rung can tell), and one
#     gateway request through a seeded API key writes its quota row;
#   * one marked breaking, rung has the compat ledger — the old server must
#     refuse to start, naming the contracted schema;
#   * one marked breaking, rung predates the ledger — it cannot refuse, so it
#     serves; the quota write may fail, and that is the break the marker
#     declares.
#
# A quota write that fails with no breaking marker is the gate's real catch:
# a contract migration (0.70.0's ai/042) shipped as if it were an expansion.
#
# Needs jq, curl, openssl and cargo (to build crates/tests/mock-inference).
set -euo pipefail

RUNG="${1:?usage: schema-ladder-boot.sh <rung-tag> <rung-cli> <database-url>}"
RUNG_CLI="$(realpath "${2:?usage: schema-ladder-boot.sh <rung-tag> <rung-cli> <database-url>}")"
DB_URL="${3:?usage: schema-ladder-boot.sh <rung-tag> <rung-cli> <database-url>}"
ROOT="$(git rev-parse --show-toplevel)"
WORK="$(mktemp -d)"
# An explicit LADDER_*_PORT is honoured (and refused when taken); otherwise the
# kernel picks free ports, so a sibling run or a stray listener cannot clash.
free_port() {
    python3 -c 'import socket; s = socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1])'
}
API_PORT="${LADDER_API_PORT:-$(free_port)}"
MOCK_PORT="${LADDER_MOCK_PORT:-$(free_port)}"
while [ "$MOCK_PORT" = "$API_PORT" ]; do MOCK_PORT="$(free_port)"; done
BASE="http://127.0.0.1:$API_PORT"
SERVER_PID=""
MOCK_PID=""

for port in "$API_PORT" "$MOCK_PORT"; do
    if (exec 3<>"/dev/tcp/127.0.0.1/$port") 2>/dev/null; then
        echo "::error::port $port is already in use; set LADDER_API_PORT / LADDER_MOCK_PORT" >&2
        exit 1
    fi
done

kill_tree() {
    local child
    for child in $(pgrep -P "$1" 2>/dev/null); do
        kill_tree "$child"
    done
    kill "$1" >/dev/null 2>&1 || true
}

cleanup() {
    for pid in "$SERVER_PID" "$MOCK_PID"; do
        [ -z "$pid" ] || kill_tree "$pid"
    done
    wait >/dev/null 2>&1 || true
    rm -rf "$WORK"
}
trap cleanup EXIT

step() { printf '\n==> %s\n' "$*"; }
fail() {
    echo "::error::$*"
    [ ! -f "$WORK/server.log" ] || { echo "--- $RUNG server log (tail)"; tail -n 80 "$WORK/server.log"; }
    exit 1
}

# Mirrors systemprompt_extension::compat::parse: the directive counts only in
# the leading block of blank and `--` lines.
is_breaking() {
    awk '{ sub(/^[ \t]+/, ""); sub(/[ \t\r]+$/, "") }
         $0 != "" && $0 !~ /^--/ { exit }
         index($0, "-- @compat:") == 1 { v = substr($0, 12); gsub(/^[ \t]+/, "", v); if (v == "breaking") found = 1 }
         END { exit !found }' "$1"
}

git -C "$ROOT" ls-tree -r --name-only "$RUNG" | LC_ALL=C sort > "$WORK/rung-files.txt"
git -C "$ROOT" ls-files '*/schema/migrations/*.sql' | LC_ALL=C sort \
    | LC_ALL=C comm -23 - "$WORK/rung-files.txt" > "$WORK/newer.txt"
BREAKING=()
while read -r file; do
    if is_breaking "$ROOT/$file"; then BREAKING+=("$file"); fi
done < "$WORK/newer.txt"

if ! git -C "$ROOT" grep -q skip_migrate "$RUNG" -- crates/entry/cli/src/commands/infrastructure/services; then
    echo "::notice::$RUNG has no \`serve --skip-migrate\`: its pods migrate on boot and were never part of the ahead-schema contract; old-binary boot skipped."
    exit 0
fi
HAS_LEDGER=false
if git -C "$ROOT" grep -q breaks_older "$RUNG" -- crates/infra/database/src; then HAS_LEDGER=true; fi

echo "    $(wc -l < "$WORK/newer.txt") migration(s) newer than $RUNG; ${#BREAKING[@]} marked breaking${BREAKING[*]:+: ${BREAKING[*]}}"
echo "    $RUNG reads the compat ledger: $HAS_LEDGER"

step "write a minimal $RUNG server profile"
mkdir -p "$WORK"/{system/web,services/config,services/ai,services/content,services/web/templates,services/web/assets,services/skills/echo_skill,services/agents,services/plugins,services/hooks,services/marketplaces,bin,storage,profile}
openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -out "$WORK/system/signing_key.pem" 2>/dev/null

cat > "$WORK/services/config/config.yaml" <<'YAML'
includes:
  - ../ai/providers.yaml
  - ../ai/gateway.yaml
agents:
  ladderagent:
    name: ladderagent
    port: 4777
    endpoint: /api/v1/agents/ladderagent/
    enabled: false
    dev_only: false
    is_primary: false
    default: false
    tags: []
    card:
      protocolVersion: 0.3.0
      name: ladderagent
      displayName: Ladder Agent
      description: Fixture agent for the schema ladder
      version: 1.0.0
      preferredTransport: JSONRPC
      capabilities:
        streaming: true
        pushNotifications: false
      defaultInputModes:
      - text/plain
      defaultOutputModes:
      - text/plain
    metadata:
      systemPrompt: You are a fixture agent.
      mcpServers:
        source: instance
        include: []
      skills:
        source: instance
      provider: anthropic
      model: claude-sonnet-4-5
      toolModelOverrides: {}
    oauth:
      required: false
      scopes: []
      audience: a2a
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
ai:
  default_provider: anthropic
  providers:
    anthropic:
      enabled: true
      default_model: claude-sonnet-4-5
YAML
cat > "$WORK/services/ai/providers.yaml" <<YAML
providers:
  - name: anthropic
    wire: anthropic
    surface: anthropic
    endpoint: http://127.0.0.1:$MOCK_PORT
    api_key_secret: anthropic
    models:
      - id: claude-sonnet-4-5
        pricing:
          input_per_million: 3.0
          output_per_million: 15.0
          cache_read_per_million: 0.0
YAML
# Why: route ids are synthesize_route_id(model_pattern, provider); boot
# refuses an authored id that does not match it.
cat > "$WORK/services/ai/gateway.yaml" <<'YAML'
gateway:
  enabled: true
  routes:
  - id: claude-star-4203d1
    model_pattern: claude-*
    provider: anthropic
  default_provider: anthropic
  auth_scheme: bearer
  inference_path_prefix: /v1
YAML
cat > "$WORK/services/ai/config.yaml" <<'YAML'
ai:
  default_provider: anthropic
  providers:
    anthropic:
      enabled: true
      default_model: claude-sonnet-4-5
YAML
printf 'id: echo_skill\nname: Echo Skill\ndescription: Fixture skill\n' > "$WORK/services/skills/echo_skill/config.yaml"
printf '# Echo Skill\n\nEcho the input back.\n' > "$WORK/services/skills/echo_skill/index.md"
echo '{}' > "$WORK/services/content/config.yaml"
echo '{}' > "$WORK/services/web/metadata.yaml"
cat > "$WORK/services/web/config.yaml" <<YAML
paths:
  templates: $WORK/services/web/templates
  assets: $WORK/services/web/assets
branding:
  name: ladder-brand
  title: "Ladder Brand"
  description: Fixture brand for the schema ladder
  copyright: 2026 fixture
  themeColor: '#ff0000'
  display_sitename: true
  twitter_handle: '@fixture'
  logo:
    primary:
      svg: /images/logo.svg
  favicon: /images/favicon.ico
fonts:
  body:
    family: OpenSans
    fallback: sans-serif
YAML
cat > "$WORK/profile/profile.yaml" <<YAML
name: ladder
display_name: Schema Ladder Old Binary
target: local
site:
  name: laddersite
  github_link: null
database:
  type: postgres
  external_db_access: false
server:
  host: 127.0.0.1
  port: $API_PORT
  api_server_url: $BASE
  api_internal_url: $BASE
  api_external_url: $BASE
  use_https: false
  cors_allowed_origins:
    - http://127.0.0.1
  content_negotiation:
    enabled: false
    markdown_suffix: .md
  security_headers:
    enabled: true
    hsts: max-age=63072000; includeSubDomains; preload
    frame_options: DENY
    content_type_options: nosniff
    referrer_policy: strict-origin-when-cross-origin
    permissions_policy: camera=()
    content_security_policy: null
  instance_id: schema-ladder
  max_concurrent_streams: 256
  trusted_proxies: []
paths:
  system: $WORK/system
  services: $WORK/services
  bin: $WORK/bin
  web_path: $WORK/system/web
  storage: $WORK/storage
  geoip_database: null
security:
  jwt_issuer: https://issuer.test
  jwt_access_token_expiration: 3600
  jwt_refresh_token_expiration: 86400
  jwt_audiences:
    - api
  allowed_resource_audiences:
    - hook
  allow_registration: true
  signing_key_path: signing_key.pem
rate_limits:
  disabled: true
  oauth_public_per_second: 10
  oauth_auth_per_second: 10
  contexts_per_second: 100
  tasks_per_second: 50
  artifacts_per_second: 50
  agent_registry_per_second: 50
  agents_per_second: 20
  mcp_registry_per_second: 50
  mcp_per_second: 200
  stream_per_second: 100
  content_per_second: 50
  burst_multiplier: 3
system_admin:
  username: testadmin
runtime:
  environment: development
  log_level: normal
  output_format: text
  no_color: true
  non_interactive: true
extensions:
  disabled: []
governance:
  authz:
    hook:
      mode: unrestricted
      timeout_ms: 500
      acknowledgement: "I understand this disables all authorization"
YAML

rung() {
    env -u RUST_LOG -u SYSTEMPROMPT_PROFILE \
        DATABASE_URL="$DB_URL" \
        OAUTH_AT_REST_PEPPER=ladder_oauth_at_rest_pepper_for_the_schema_ladder_zz \
        MANIFEST_SIGNING_SECRET_SEED=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA= \
        SYSTEMPROMPT_CUSTOM_SECRETS=encryption_master_key,anthropic \
        encryption_master_key=3333333333333333333333333333333333333333333333333333333333333333 \
        anthropic=sk-ant-schema-ladder \
        ANTHROPIC_API_KEY=sk-ant-schema-ladder \
        SYSTEMPROMPT_SUBPROCESS=1 \
        timeout "${RUNG_TIMEOUT:-120}" "$RUNG_CLI" --non-interactive --no-color \
        --profile "$WORK/profile/profile.yaml" "$@"
}

serve() {
    RUNG_TIMEOUT=600 rung infra services serve --foreground --skip-migrate > "$WORK/server.log" 2>&1 &
    SERVER_PID=$!
}

if [ "${#BREAKING[@]}" -gt 0 ] && [ "$HAS_LEDGER" = true ]; then
    step "$RUNG must refuse a schema this tree contracted"
    status=0
    RUNG_TIMEOUT=120 rung infra services serve --foreground --skip-migrate > "$WORK/server.log" 2>&1 || status=$?
    [ "$status" -ne 0 ] || fail "$RUNG served a database carrying @compat: breaking migrations it predates."
    [ "$status" -ne 124 ] || fail "$RUNG neither served nor refused within 120s against a contracted schema."
    grep -qF 'marked `@compat: breaking`' "$WORK/server.log" \
        || fail "$RUNG exited $status but not with the contracted-schema refusal."
    echo "    refused (exit $status): $(grep -F 'marked `@compat: breaking`' "$WORK/server.log" | head -n 1)"
    exit 0
fi

step "build the mock inference upstream"
cargo build --locked --manifest-path "$ROOT/crates/tests/mock-inference/Cargo.toml" \
    --target-dir "${LADDER_MOCK_TARGET:-$WORK/mock-target}"
"${LADDER_MOCK_TARGET:-$WORK/mock-target}/debug/mock-inference" --port "$MOCK_PORT" > "$WORK/mock.log" 2>&1 &
MOCK_PID=$!

step "seed the system admin and an API key with a request quota"
rung admin bootstrap --email testadmin@example.com > "$WORK/bootstrap.log" 2>&1 \
    || { cat "$WORK/bootstrap.log"; fail "$RUNG could not bootstrap its admin against the migrated database."; }
USER_ID="$(psql "$DB_URL" -XAtq -v ON_ERROR_STOP=1 -c "SELECT id FROM users WHERE name = 'testadmin'")"
[ -n "$USER_ID" ] || fail "no testadmin row after bootstrap"
rung --json admin users api-key issue --user "$USER_ID" --name schema-ladder \
    --max-requests 1000 --window-seconds 3600 > "$WORK/key.json" 2> "$WORK/key.err" \
    || { cat "$WORK/key.err"; fail "$RUNG could not issue an API key against the migrated database."; }
KEY="$(jq -r '[.. | objects | (.secret? // (select(.heading? == "secret") | .content)) | strings] | first // empty' "$WORK/key.json")"
[ -n "$KEY" ] || { cat "$WORK/key.json"; fail "api-key issue printed no secret"; }

step "serve $RUNG against the schema this tree left"
serve
deadline=$((SECONDS + 120))
until curl -fsS "$BASE/readyz" -o "$WORK/readyz.json" 2>/dev/null; do
    kill -0 "$SERVER_PID" 2>/dev/null || fail "$RUNG exited before readiness against the migrated database."
    [ "$SECONDS" -lt "$deadline" ] || fail "$RUNG was not ready within 120s."
    sleep 1
done
echo "    /readyz: $(cat "$WORK/readyz.json")"
jq -e '.status == "ready"' "$WORK/readyz.json" >/dev/null || fail "/readyz is 200 but not ready"
jq -e --arg v "${RUNG#v}" '.version == $v' "$WORK/readyz.json" >/dev/null \
    || fail "/readyz answered from a server that is not $RUNG"
if [ "$HAS_LEDGER" = true ] && [ -s "$WORK/newer.txt" ]; then
    jq -e '.schema == "ahead"' "$WORK/readyz.json" >/dev/null \
        || fail "$RUNG serves a schema ahead of it without reporting \"schema\": \"ahead\"."
fi

step "one gateway request through the seeded key must write its quota row"
SESSION="$(curl -fsS -X POST "$BASE/api/public/gateway/sessions" -H "Authorization: Bearer $KEY" | jq -r .session_id)"
if [ -z "$SESSION" ] || [ "$SESSION" = null ]; then
    fail "$RUNG would not mint a gateway session for the seeded key."
fi
before="$(psql "$DB_URL" -XAtq -c 'SELECT count(*) FROM ai_quota_buckets')"
code="$(curl -sS -o "$WORK/messages.json" -w '%{http_code}' -X POST "$BASE/v1/messages" \
    -H "Authorization: Bearer $KEY" -H "x-session-id: $SESSION" \
    -H 'anthropic-version: 2023-06-01' -H 'content-type: application/json' \
    -d '{"model":"claude-sonnet-4-5","max_tokens":16,"messages":[{"role":"user","content":"ping"}]}')"
after="$(psql "$DB_URL" -XAtq -c 'SELECT count(*) FROM ai_quota_buckets')"
echo "    /v1/messages: HTTP $code; ai_quota_buckets $before -> $after"

if [ "$code" = 200 ] && [ "$after" -gt "$before" ]; then
    if [ "${#BREAKING[@]}" -gt 0 ]; then
        echo "::notice::$RUNG predates the compat ledger and its quota write still works; the breaking marker covers something this probe does not exercise."
    fi
    echo "    $RUNG serves the upgraded schema"
    exit 0
fi
cat "$WORK/messages.json"; echo
# Why: an accepted failure must be the schema contract, not a broken probe; a
# failure the database did not raise (mock, auth, routing) proves nothing.
if ! grep -qiE 'error returned from database|SQLSTATE|ON CONFLICT' "$WORK/server.log"; then
    fail "$RUNG refused the gateway request without a database error; the probe itself is broken"
fi
echo "    database error: $(grep -iE 'error returned from database|SQLSTATE|ON CONFLICT' "$WORK/server.log" | head -n 1 | cut -c1-240)"
if [ "${#BREAKING[@]}" -gt 0 ]; then
    echo "    expected: $RUNG predates the compat ledger, and ${BREAKING[*]} declares the contract it cannot serve"
    exit 0
fi
fail "$RUNG cannot write a quota row against the upgraded schema, and no newer migration is marked \`-- @compat: breaking\`. Mark the contract migration (a new migration if the file is already on next) so rolling updates stop older pods first."
