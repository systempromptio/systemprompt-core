# systemprompt.io OS - Lean Justfile
# Use CLI directly with global flags: --json, --verbose, --debug, --no-color

# Show all commands
default:
    @just --list

# =============================================================================
# BUILD & TEST
# =============================================================================

# Lint: enforce typed identifiers (no raw String/&str for known ID field names)
lint-raw-ids:
    ./scripts/lint-raw-ids.sh

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

# Build the Bridge helper + sync agent (credential helper, plugin/MCP sync)
build-bridge TARGET="":
    #!/usr/bin/env bash
    set -e
    if [ -n "{{TARGET}}" ]; then
        cargo build --manifest-path bin/bridge/Cargo.toml --release --target {{TARGET}}
    else
        cargo build --manifest-path bin/bridge/Cargo.toml --release
    fi

# Build systemprompt-bridge for all supported release targets
build-bridge-all:
    just build-bridge aarch64-apple-darwin
    just build-bridge x86_64-apple-darwin
    just build-bridge x86_64-pc-windows-msvc
    just build-bridge x86_64-unknown-linux-gnu

# Wrap the bridge binary in a macOS .app bundle (Info.plist + AppIcon.icns)
bundle-bridge-mac TARGET="":
    #!/usr/bin/env bash
    set -e
    if [ -n "{{TARGET}}" ]; then
        just build-bridge {{TARGET}}
        bin/bridge/scripts/make-mac-app.sh --target {{TARGET}}
    else
        just build-bridge
        bin/bridge/scripts/make-mac-app.sh
    fi

# Prepare sqlx offline cache (requires running database)
sqlx-prepare:
    cargo sqlx prepare --workspace

# Prepare per-crate SQLx caches for publishing (requires running database).
#
# Each published crate ships its own `.sqlx/` so crates.io can build it offline.
# `entry/api` is intentionally absent: it issues no SQL via `query!` macros, so
# it needs no cache at all (confirmed by `sqlx-verify-offline`).
#
# Note on cache contents: single-crate `cargo sqlx prepare` runs `cargo check`,
# which re-expands path-dependency `query!` macros, so each crate's `.sqlx/`
# also captures a subset of its dependencies' queries. This overlap is inherent
# to preparing individually-published workspace crates (workspace-mode prepare
# would dedupe into one root cache, but crates.io builds each crate standalone).
# The extra entries are inert — a crate only looks up hashes for macros it
# actually compiles. Only regenerate when SQL changed, and review `git diff`:
# pure churn with no SQL change is the prepare's non-determinism, not a real
# delta, and need not be committed. `sqlx-verify-offline` is the correctness gate.
sqlx-prepare-publish:
    #!/usr/bin/env bash
    set -e
    echo "Generating per-crate .sqlx directories for crates.io publishing..."
    echo ""
    for crate in crates/infra/database crates/infra/events crates/infra/logging crates/infra/security \
                 crates/domain/analytics crates/domain/agent crates/domain/oauth \
                 crates/domain/users crates/domain/content crates/domain/files \
                 crates/domain/ai crates/domain/mcp crates/app/scheduler \
                 crates/app/sync crates/entry/cli; do
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
    for crate in systemprompt-database systemprompt-events systemprompt-logging systemprompt-security \
                 systemprompt-analytics systemprompt-agent systemprompt-oauth systemprompt-users \
                 systemprompt-content systemprompt-files systemprompt-ai \
                 systemprompt-mcp systemprompt-scheduler systemprompt-sync \
                 systemprompt-cli systemprompt-api; do
        echo "  Checking $crate..."
        SQLX_OFFLINE=true cargo package -p "$crate" --allow-dirty 2>&1 | tail -1
    done
    echo ""
    echo "All crates verified for offline compilation!"

# Cut a new patch/minor/major release: bump → sync → tag → push → publish.
# The release script itself is gitignored (release credentials and machine-specific
# paths live there); this recipe is the discoverable entry point used by the docs.
release BUMP="patch":
    @[ "{{BUMP}}" = "patch" ] || [ "{{BUMP}}" = "minor" ] || [ "{{BUMP}}" = "major" ] || \
        { echo "usage: just release [patch|minor|major]"; exit 2; }
    @[ -x scripts/release.sh ] || { echo "scripts/release.sh missing — see instructions/information/crates-publishing.md"; exit 1; }
    ./scripts/release.sh {{BUMP}}

# Reject imperative SQL in declarative schema files
lint-schema:
    ./scripts/lint-schema.sh crates

# Reject inline SQL and hand-built migrations in extension.rs files
lint-extensions:
    ./scripts/lint-extensions.sh crates

# Check without building
check: lint-schema lint-extensions lint-test-value
    cargo check --workspace

# Check offline (uses cached .sqlx metadata, no database required)
check-offline:
    SQLX_OFFLINE=true cargo check --workspace

# Format code (nightly: rustfmt.toml uses unstable options).
# Covers the separate `crates/tests` workspace too — `--all` stops at the
# manifest it is invoked from, so the test workspace must be formatted explicitly.
fmt:
    cargo +nightly fmt --all
    cd crates/tests && cargo +nightly fmt --all

# Check formatting without making changes (main + test workspace).
format-check:
    cargo +nightly fmt --all -- --check
    cd crates/tests && cargo +nightly fmt --all -- --check

# Run clippy linter with strict settings (main workspace).
# The separate `crates/tests` workspace is clippied by `just style-check` (it
# needs a live database for its `query!` fixtures, which CI's lint job lacks);
# CI compiles it in the dedicated Test job instead.
lint:
    cargo clippy --workspace --all-targets --all-features -- -D warnings

# Reject unverified sqlx::query calls outside the allowlist
lint-sqlx:
    ./scripts/check-sqlx.sh

# Reject inline `map_err(|e| ApiError::ctor(...))` at HTTP call sites.
# HTTP status mapping belongs in an entry-local error type's From impls;
# call sites propagate with bare `?` so the variant decides the status.
lint-http-errors:
    ./scripts/check-http-errors.sh

# Reject `let _ = <expr>.unwrap()/.expect()` in the test workspace.
# A discarded fallible result runs the code for its panic side effect but
# asserts nothing — bind the result and assert, or annotate a deliberate
# side-effect call with `// lint-ok: no-assert <reason>`.
lint-test-value:
    ./scripts/check-test-value.sh

# Reject UserId::admin() outside the sanctioned bootstrap call sites.
# The sentinel is reserved for the actor model, the bootstrap CLI, the
# scheduler default config, the MCP server registry, and the LogActor
# platform-event constructor. Any other call site bypasses the actor
# typing and silently attributes work to the platform owner.
lint-no-untyped-admin:
    #!/usr/bin/env bash
    set -euo pipefail
    hits=$(grep -rn 'UserId::admin()' crates/ --include='*.rs' \
        | grep -v 'crates/tests/' \
        | grep -v 'crates/shared/identifiers/src/actor.rs' \
        | grep -v 'crates/shared/identifiers/src/bootstrap.rs' \
        | grep -v 'crates/shared/identifiers/src/user.rs' \
        | grep -v 'crates/entry/cli/src/commands/admin/bootstrap.rs' \
        | grep -v 'crates/entry/cli/src/commands/infrastructure/jobs/run.rs' \
        | grep -v 'crates/shared/models/src/services/scheduler.rs' \
        | grep -v 'crates/domain/mcp/src/services/registry/manager.rs' \
        | grep -v 'crates/infra/logging/src/models/log_entry.rs' \
        || true)
    if [ -n "$hits" ]; then
        echo "lint-no-untyped-admin: untyped UserId::admin() outside the sanctioned call sites:"
        echo "$hits"
        exit 1
    fi

# Every Cargo workspace in the repo. `bin/bridge` and `crates/tests*` are excluded
# from the root workspace, so a bare root-level scan silently skips them — which is
# how a 7.5-high advisory sat unnoticed in the bridge lockfile. Keep this list in
# sync with the tracked Cargo.lock files (`git ls-files '*Cargo.lock'`).
workspaces := ". bin/bridge crates/tests crates/tests/bench crates/tests/fuzz crates/tests/loadtest crates/tests/mock-inference"

# Run cargo-deny across every workspace: licenses, advisories, bans, sources.
# All workspaces share the root deny.toml so the ignore rationales live in one file.
deny:
    #!/usr/bin/env bash
    set -euo pipefail
    for w in {{ workspaces }}; do
        echo "==> cargo deny: $w"
        (cd "$w" && cargo deny check --config "$(git rev-parse --show-toplevel)/deny.toml")
    done

# Run cargo-audit against the RustSec advisory DB, across every workspace
audit:
    #!/usr/bin/env bash
    set -euo pipefail
    for w in {{ workspaces }}; do
        echo "==> cargo audit: $w"
        (cd "$w" && cargo audit)
    done

# Detect unused dependencies across every workspace
machete:
    #!/usr/bin/env bash
    set -euo pipefail
    for w in {{ workspaces }}; do
        echo "==> cargo machete: $w"
        (cd "$w" && cargo machete)
    done

# Build every feature powerset (catches facade-flag drift)
hack:
    cargo hack --workspace --feature-powerset --depth 2 check

# Flag source files exceeding 300 lines (excludes target/, tests/, and `//!` doc heads)
file-size:
    @find crates -name '*.rs' -not -path '*/target/*' -not -path '*/tests/*' | xargs -r awk '!/^\/\/!/ {n[FILENAME]++} END {for (f in n) if (n[f]>300) print n[f], f}'

# Verify every production file has a doc head + BSL-1.1 license reference
check-headers:
    ./scripts/check-file-headers.sh

# Run custom style validators
validate:
    ./tests/validator/validate.sh
    ./scripts/check-sqlx.sh

# Run all style checks (format + lint + validate)
style-check:
    #!/usr/bin/env bash
    set -e
    echo "🎨 Running style checks..."
    echo ""
    echo "1️⃣  Checking code formatting..."
    cargo +nightly fmt --all -- --check
    echo ""
    echo "2️⃣  Running clippy linter..."
    cargo clippy --workspace --all-targets --all-features -- -D warnings
    echo ""
    echo "3️⃣  Running custom validators..."
    ./tests/validator/validate.sh
    echo ""
    echo "4️⃣  Checking sqlx::query allowlist..."
    ./scripts/check-sqlx.sh
    echo ""
    echo "5️⃣  Checking HTTP error propagation..."
    ./scripts/check-http-errors.sh
    echo ""
    echo "6️⃣  Checking the test workspace (fmt + clippy + compile)..."
    (cd crates/tests && cargo +nightly fmt --all -- --check)
    (cd crates/tests && cargo clippy --workspace --all-targets --all-features -- -D warnings)
    (cd crates/tests && cargo test --workspace --no-run)
    echo ""
    echo "✅ All style checks passed!"

# Run unit tests (separate test workspace, no database required)
unit-test *ARGS:
    cargo test --manifest-path crates/tests/Cargo.toml --workspace {{ARGS}}

# Check unit test compilation without running
unit-check:
    cargo check --manifest-path crates/tests/Cargo.toml --workspace --tests

# Run unit tests for a specific crate (e.g., just unit-test-crate systemprompt-agent-tests)
unit-test-crate CRATE *ARGS:
    cargo test --manifest-path crates/tests/Cargo.toml -p {{CRATE}} {{ARGS}}

# Run property-based tests (proptest)
property-test *ARGS:
    cargo test --manifest-path crates/tests/Cargo.toml -p systemprompt-property-tests {{ARGS}}

# Run protocol contract tests
contract-test *ARGS:
    cargo test --manifest-path crates/tests/Cargo.toml -p systemprompt-contract-tests {{ARGS}}

# Run concurrency tests
concurrency-test *ARGS:
    cargo test --manifest-path crates/tests/Cargo.toml -p systemprompt-concurrency-tests {{ARGS}}

# Mutation-test one production crate against its test-workspace suite
# (e.g. just mutants crates/infra/security systemprompt-security-tests).
# Hours per crate; mutates the tree in-place (auto-reverted) — run it in a
# spare checkout, and export DATABASE_URL at a fresh migrated DB first.
mutants DIR TESTPKG *ARGS:
    cd {{DIR}} && cargo mutants --in-place --baseline=skip \
        --test-tool=nextest \
        --test-package {{TESTPKG}} \
        --cargo-test-arg --manifest-path={{justfile_directory()}}/crates/tests/Cargo.toml \
        --timeout 300 {{ARGS}}

# Run criterion benchmarks
bench *ARGS:
    cargo bench --manifest-path crates/tests/bench/Cargo.toml {{ARGS}}

# Run a specific fuzz target (e.g., just fuzz fuzz_jsonrpc_parse 60)
fuzz TARGET DURATION="60":
    cargo fuzz run --fuzz-dir crates/tests/fuzz {{TARGET}} -- -max_total_time={{DURATION}}

# Run load tests (requires running server: cd ../systemprompt-web && just start)
loadtest SCENARIO="all" PROFILE="ci" *ARGS:
    cargo run --manifest-path crates/tests/loadtest/Cargo.toml -- --scenario {{SCENARIO}} --profile {{PROFILE}} {{ARGS}}

# Run the mock internal inference server (stands in for the customer's endpoint)
mock-inference *ARGS:
    cargo run --manifest-path crates/tests/mock-inference/Cargo.toml -- {{ARGS}}

# Run load tests against the air-gapped profile (strict thresholds)
loadtest-airgap *ARGS:
    cargo run --manifest-path crates/tests/loadtest/Cargo.toml -- --profile airgap {{ARGS}}

# Run the staged-ramp load test (100->250->500->1000 users)
loadtest-scaled *ARGS:
    cargo run --manifest-path crates/tests/loadtest/Cargo.toml -- --profile scaled {{ARGS}}

# Run the soak load test (~20 users sustained ~1h)
loadtest-soak *ARGS:
    cargo run --manifest-path crates/tests/loadtest/Cargo.toml -- --profile soak {{ARGS}}

# Run the spike load test (baseline -> ~800 burst -> recovery)
loadtest-spike *ARGS:
    cargo run --manifest-path crates/tests/loadtest/Cargo.toml -- --profile spike {{ARGS}}

# Run a load test fanned out across replica base URLs (comma-separated)
loadtest-distributed NODES *ARGS:
    cargo run --manifest-path crates/tests/loadtest/Cargo.toml -- --nodes {{NODES}} {{ARGS}}

# Generate line-coverage summary for the workspace.
#
# Mirrors .github/workflows/coverage.yml so local + CI numbers stay
# comparable. Runs entirely in dedicated target dirs under
# coverage-report/ (override with COVERAGE_TARGET_DIR) so concurrent
# sessions sharing this checkout can neither clobber the instrumented
# artifacts nor be broken by this run — never builds in the shared
# crates/tests/target or ./target, and never mutates shared files.
# Works around three local-only sabotage points the CI runner
# doesn't have:
#
#   1. sccache via [build] rustc-wrapper in ~/.cargo/config.toml
#      (returns uninstrumented cached rlibs) — neutralised by
#      CARGO_BUILD_RUSTC_WRAPPER="" on the cargo invocation.
#   2. The mold linker pinned by target.<triple>.rustflags — mold
#      drops the profile-runtime constructors and instrumented
#      binaries skip the atexit registration, silently producing
#      zero profraw files. Setting the RUSTFLAGS env replaces
#      target rustflags entirely (cargo's flag-resolution order),
#      so the default linker is used, with --jobs 4 to cap RAM use
#      on the 2GB+ instrumented binaries.
#   3. Historically, a Cranelift [unstable]/[profile.dev] section in
#      the cargo configs (silently strips -C instrument-coverage).
#      Those sections are gone; if a coverage run ever produces
#      profraws but a 0% report again, check they haven't returned.
coverage:
    #!/usr/bin/env bash
    set -euo pipefail
    ROOT="$(pwd)"
    PROFDIR="$ROOT/coverage-report/profraw"
    TDIR="${COVERAGE_TARGET_DIR:-$ROOT/coverage-report/target}"
    MAINTDIR="$TDIR-main"
    rm -rf "$PROFDIR" "$ROOT/coverage-report/tests.profdata"
    mkdir -p "$PROFDIR"

    cd crates/tests

    echo "==> Running instrumented test suite (target dir: $TDIR)"
    # Setting RUSTFLAGS env replaces target.<triple>.rustflags entirely
    # (cargo's flag-resolution order), which is exactly what we want:
    # the parent config's `-C link-arg=-fuse-ld=mold` is dropped and
    # the default linker handles the link. We can NOT use cargo-llvm-cov
    # here — it MERGES target rustflags into RUSTFLAGS internally, so
    # mold gets re-injected and silently strips the profile-runtime
    # constructors, producing zero profraw files at runtime.
    #
    # CARGO_BUILD_RUSTC_WRAPPER="" disables sccache (otherwise it
    # returns cached uninstrumented rlibs and the test binaries link
    # __llvm_profile_runtime but record no counters).
    #
    # --jobs 4 caps concurrent linker invocations: instrumented test
    # binaries can exceed 2GB and the default ld OOM-kills under
    # 32-way parallelism even on 23GB RAM.
    # Build the `systemprompt` binary from the main workspace under the same
    # instrumentation flags so subprocess tests can invoke it and contribute
    # coverage. The crates/tests workspace doesn't include entry/cli, so its
    # `--bins` flag would not otherwise produce the binary.
    echo "==> Building instrumented systemprompt binary from main workspace"
    (cd "$ROOT" && CARGO_BUILD_RUSTC_WRAPPER="" RUSTC_WRAPPER="" \
        CARGO_TARGET_DIR="$MAINTDIR" \
        LLVM_PROFILE_FILE="$PROFDIR/%m%c.profraw" \
        RUSTFLAGS="-C instrument-coverage -C llvm-args=--runtime-counter-relocation" \
        cargo build -p systemprompt-cli --bin systemprompt --jobs 4)
    export SYSTEMPROMPT_BIN="$MAINTDIR/debug/systemprompt"

    # DATABASE_URL is required by subprocess_full.rs and other tests that
    # invoke the systemprompt binary through full SecretsBootstrap; without
    # it those tests early-return and produce no coverage.
    #
    # %m%c (continuous mode, no %p): with per-PID files, PID reuse across the
    # ~18k nextest processes silently overwrites earlier profraws — tests
    # covered only by a single low-frequency process read as uncovered. One
    # mmap-shared file per module signature makes counter updates atomic and
    # mirrors coverage.yml.
    : "${DATABASE_URL:=postgres://systemprompt_admin:3e00fcdac26b5b731829e8737515db8f@localhost:5432/systemprompt-web}"
    CARGO_BUILD_RUSTC_WRAPPER="" \
        RUSTC_WRAPPER="" \
        CARGO_TARGET_DIR="$TDIR" \
        LLVM_PROFILE_FILE="$PROFDIR/%m%c.profraw" \
        RUSTFLAGS="-C instrument-coverage -C llvm-args=--runtime-counter-relocation" \
        SYSTEMPROMPT_BIN="$SYSTEMPROMPT_BIN" \
        DATABASE_URL="$DATABASE_URL" \
        cargo nextest run --workspace --lib --bins --tests --build-jobs 4 --no-fail-fast \
        || echo "warning: test failures/timeouts above — continuing to coverage report"

    PROFRAW_COUNT=$(find "$PROFDIR" -name "*.profraw" | wc -l)
    echo "==> Generated $PROFRAW_COUNT profraw files"

    LLVM_PROFDATA=$(rustc --print sysroot)/lib/rustlib/x86_64-unknown-linux-gnu/bin/llvm-profdata
    LLVM_COV=$(rustc --print sysroot)/lib/rustlib/x86_64-unknown-linux-gnu/bin/llvm-cov

    echo "==> Merging profile data"
    find "$PROFDIR" -name '*.profraw' > "$ROOT/coverage-report/profraw-list.txt"
    "$LLVM_PROFDATA" merge -sparse -f "$ROOT/coverage-report/profraw-list.txt" -o "$ROOT/coverage-report/tests.profdata"

    # Test binaries land in deps/; the `systemprompt` cli bin lands one level
    # up in debug/ (cargo writes named binaries there, not under deps/).
    BINS=$(find "$TDIR/debug/deps" -maxdepth 1 -executable -type f \
        \( -name 'systemprompt_*' -o -name 'systemprompt-*' \) ! -name '*.d' -printf '%T@ %p\n' \
        | sort -rn \
        | awk '{ base=$2; sub(".*/", "", base); sub(/-[0-9a-f]+$/, "", base); if (!seen[base]++) print $2 }')
    SP_BIN="$MAINTDIR/debug/systemprompt"
    [ -x "$SP_BIN" ] && BINS="$BINS $SP_BIN"
    OBJ_ARGS=""
    for b in $BINS; do OBJ_ARGS="$OBJ_ARGS --object $b"; done

    echo "==> Coverage report"
    "$LLVM_COV" report \
        --instr-profile="$ROOT/coverage-report/tests.profdata" \
        $OBJ_ARGS \
        --ignore-filename-regex="(\.cargo|rustc|crates/tests|bin/bridge|$HOME/\.cargo)" \
        --summary-only

    "$LLVM_COV" export \
        --instr-profile="$ROOT/coverage-report/tests.profdata" \
        $OBJ_ARGS \
        --ignore-filename-regex="(\.cargo|rustc|crates/tests|bin/bridge|$HOME/\.cargo)" \
        --format=lcov \
        > "$ROOT/coverage-report/lcov.info"

    echo ""
    echo "lcov.info: coverage-report/lcov.info"
    echo "For HTML report: just coverage-html"

# Render coverage as a browsable HTML tree (requires `just coverage` first).
coverage-html:
    #!/usr/bin/env bash
    set -euo pipefail
    ROOT="$(pwd)"
    if [ ! -f "$ROOT/coverage-report/tests.profdata" ]; then
        echo "Run 'just coverage' first to generate profdata"
        exit 1
    fi
    LLVM_COV=$(rustc --print sysroot)/lib/rustlib/x86_64-unknown-linux-gnu/bin/llvm-cov
    TDIR="${COVERAGE_TARGET_DIR:-$ROOT/coverage-report/target}"
    MAINTDIR="$TDIR-main"
    # Test binaries land in deps/; the `systemprompt` cli bin lands one level
    # up in debug/ (cargo writes named binaries there, not under deps/).
    BINS=$(find "$TDIR/debug/deps" -maxdepth 1 -executable -type f \
        \( -name 'systemprompt_*' -o -name 'systemprompt-*' \) ! -name '*.d' -printf '%T@ %p\n' \
        | sort -rn \
        | awk '{ base=$2; sub(".*/", "", base); sub(/-[0-9a-f]+$/, "", base); if (!seen[base]++) print $2 }')
    SP_BIN="$MAINTDIR/debug/systemprompt"
    [ -x "$SP_BIN" ] && BINS="$BINS $SP_BIN"
    OBJ_ARGS=""
    for b in $BINS; do OBJ_ARGS="$OBJ_ARGS --object $b"; done
    mkdir -p "$ROOT/coverage-report/html"
    "$LLVM_COV" show \
        --instr-profile="$ROOT/coverage-report/tests.profdata" \
        $OBJ_ARGS \
        --ignore-filename-regex="(\.cargo|rustc|crates/tests|bin/bridge|$HOME/\.cargo)" \
        --format=html \
        --output-dir="$ROOT/coverage-report/html"
    echo "Coverage report: coverage-report/html/index.html"

# Clean coverage artifacts
coverage-clean:
    rm -rf coverage-report/

# Clean build artifacts
clean:
    cargo clean

# =============================================================================
# SERVICES
# =============================================================================

# Start API server (checks if already running)
[unix]
api:
    #!/usr/bin/env bash
    if lsof -ti :8080 >/dev/null 2>&1; then
        echo "✅ Server already running on port 8080"
        echo ""
        echo "💡 To restart with latest code: just api-rebuild"
        exit 0
    fi
    echo "🚀 Starting API server..."
    ./target/debug/systemprompt serve api --foreground

[windows]
api:
    #!powershell
    $port = netstat -ano | Select-String ":8080.*LISTENING"
    if ($port) {
        Write-Host "✅ Server already running on port 8080"
        Write-Host ""
        Write-Host "💡 To restart with latest code: just api-rebuild"
        exit 0
    }
    Write-Host "🚀 Starting API server..."
    .\target\debug\systemprompt.exe serve api --foreground

# Rebuild and restart entire system (API + agents + MCP)
[unix]
api-rebuild:
    #!/usr/bin/env bash
    set -e
    echo "🔨 Building..."
    cargo build --bin systemprompt
    echo "🧹 Cleaning up services..."
    ./target/debug/systemprompt cleanup-services
    echo "✅ Starting fresh API server..."
    ./target/debug/systemprompt serve api --foreground

[windows]
api-rebuild:
    #!powershell
    $ErrorActionPreference = "Stop"
    Write-Host "🔨 Building..."
    cargo build --bin systemprompt
    Write-Host "🧹 Cleaning up services..."
    .\target\debug\systemprompt.exe cleanup-services
    Write-Host "✅ Starting fresh API server..."
    .\target\debug\systemprompt.exe serve api --foreground

# Convenient alias for api-rebuild
restart:
    just api-rebuild

# Build and start API server with TEST database (for integration tests)
[unix]
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

[windows]
api-test-rebuild:
    #!powershell
    $ErrorActionPreference = "Stop"
    Write-Host "🔨 Building..."
    cargo build --bin systemprompt
    Write-Host "🧹 Cleaning up services..."
    .\target\debug\systemprompt.exe cleanup-services
    Write-Host "✅ Starting fresh API server with TEST database..."
    $env:DATABASE_URL = "database/test.db"
    .\target\debug\systemprompt.exe serve api --foreground

# Reload agents with latest binary (keeps API server running)
[unix]
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

[windows]
agents-reload:
    #!powershell
    Write-Host "🔨 Building latest binary..."
    cargo build --bin systemprompt
    Write-Host "🧹 Stopping old agent processes..."
    # Kill agent processes on known ports
    foreach ($port in 9000, 9001, 9002, 9003) {
        $pids = netstat -ano | Select-String ":$port.*LISTENING" | ForEach-Object { ($_ -split '\s+')[-1] } | Sort-Object -Unique
        foreach ($pid in $pids) {
            if ($pid -and $pid -ne "0") { taskkill /PID $pid /F 2>$null }
        }
    }
    # Kill agent processes by name pattern
    taskkill /IM "systemprompt*" /F 2>$null
    Write-Host "⏳ Waiting for processes to terminate..."
    Start-Sleep -Seconds 2
    Write-Host "🚀 Starting agents with new binary via API reconciliation..."
    & .\target\debug\systemprompt.exe admin agents restart --all 2>$null
    if (-not $?) { Write-Host "Note: Agents will auto-start with API" }
    Write-Host "✅ Agents reloaded with latest binary"
    Write-Host ""
    Write-Host "💡 Check status: just agents"

# Nuclear option: kill everything and reset (API, agents, MCP servers, database)
[unix]
api-nuke:
    #!/usr/bin/env bash
    set +e
    echo "🔨 Building..."
    cargo build --bin systemprompt
    echo "💥 NUKING ALL PROCESSES..."
    for port in 8080 9000 9001 9002 9003 5000 5001 5002 5003 5004 5005; do
        lsof -ti :$port 2>/dev/null | xargs -r kill -9 2>/dev/null || true
    done
    pkill -9 -f "systemprompt serve api" 2>/dev/null || true
    pkill -9 -f "systemprompt admin agents run" 2>/dev/null || true
    pkill -9 -f "systemprompt-admin" 2>/dev/null || true
    pkill -9 -f "systemprompt-introduction" 2>/dev/null || true
    pkill -9 -f "systemprompt-helper" 2>/dev/null || true
    pkill -9 -f "systemprompt" 2>/dev/null || true
    sleep 1
    ./target/debug/systemprompt infra db execute "DELETE FROM services" 2>/dev/null || true
    echo "✅ Nuclear cleanup complete, starting fresh API server..."
    ./target/debug/systemprompt serve api --foreground

[windows]
api-nuke:
    #!powershell
    Write-Host "🔨 Building..."
    cargo build --bin systemprompt
    Write-Host "💥 NUKING ALL PROCESSES..."
    # Kill processes on service ports
    foreach ($port in 8080, 9000, 9001, 9002, 9003, 5000, 5001, 5002, 5003, 5004, 5005) {
        $pids = netstat -ano | Select-String ":$port.*LISTENING" | ForEach-Object { ($_ -split '\s+')[-1] } | Sort-Object -Unique
        foreach ($pid in $pids) {
            if ($pid -and $pid -ne "0") { taskkill /PID $pid /F 2>$null }
        }
    }
    # Kill all systemprompt processes
    taskkill /IM "systemprompt*" /F 2>$null
    Start-Sleep -Seconds 1
    # Clean up services database
    & .\target\debug\systemprompt.exe infra db execute "DELETE FROM services" 2>$null
    Write-Host "✅ Nuclear cleanup complete, starting fresh API server..."
    .\target\debug\systemprompt.exe serve api --foreground

# =============================================================================
# TESTING
# =============================================================================

# Run the Rust test workspace (crates/tests) end to end against a fresh database.
# Mirrors the CI `test` job: drop+recreate the target DB, apply every extension
# schema with the migrate tool built OFFLINE (the schema does not exist yet, so
# live query verification of its core-crate deps would fail), then run the suite
# LIVE against the migrated schema. Override the target with TEST_DATABASE_URL;
# the default points at a dedicated `systemprompt_test` DB on the local server.
test-rust *args:
    #!/usr/bin/env bash
    set -euo pipefail
    db="${TEST_DATABASE_URL:-postgres://postgres:postgres@localhost:5432/systemprompt_test}"
    base="${db%/*}"
    name="${db##*/}"
    echo "▶ resetting test database: ${name}"
    psql "${base}/postgres" -v ON_ERROR_STOP=1 -c "DROP DATABASE IF EXISTS \"${name}\" WITH (FORCE);" >/dev/null
    psql "${base}/postgres" -v ON_ERROR_STOP=1 -c "CREATE DATABASE \"${name}\";" >/dev/null
    echo "▶ applying extension schemas (offline build)"
    SQLX_OFFLINE=true DATABASE_URL="${db}" \
        cargo run --manifest-path crates/tests/Cargo.toml -p systemprompt-test-migrate --release
    echo "▶ running Rust test workspace (live against migrated schema)"
    SQLX_OFFLINE=false DATABASE_URL="${db}" \
        cargo test --manifest-path crates/tests/Cargo.toml --workspace --lib {{args}}

# Install the prebuilt cargo-nextest binary (no compile) into CARGO_HOME/bin.
# Required by `just test-shard` / `just test-all-shards`.
install-nextest:
    #!/usr/bin/env bash
    set -euo pipefail
    bin="${CARGO_HOME:-$HOME/.cargo}/bin"
    echo "▶ installing cargo-nextest into ${bin}"
    curl -LsSf https://get.nexte.st/latest/linux | tar zxf - -C "${bin}"
    "${bin}/cargo-nextest" nextest --version

# Run one CI shard locally against a fresh, freshly-migrated database.
# Mirrors the CI `test` job exactly: the shard group→crate mapping and the
# nextest invocation come from scripts/test-shard.sh (shared with CI). Each run
# drops+recreates the target DB so cross-run pollution can't occur. Override the
# DB with TEST_DATABASE_URL; the default is a disposable `systemprompt_test`.
# Groups: shared infra domain app-runtime app-scheduler app-sync app-generator entry-api entry-cli bridge integration edge
test-shard GROUP *args:
    #!/usr/bin/env bash
    set -euo pipefail
    command -v cargo-nextest >/dev/null 2>&1 || {
        echo "cargo-nextest not found — run 'just install-nextest' first" >&2
        exit 1
    }
    db="${TEST_DATABASE_URL:-postgres://postgres:postgres@localhost:5432/systemprompt_test}"
    base="${db%/*}"
    name="${db##*/}"
    echo "▶ resetting test database: ${name}"
    psql "${base}/postgres" -v ON_ERROR_STOP=1 -c "DROP DATABASE IF EXISTS \"${name}\" WITH (FORCE);" >/dev/null
    psql "${base}/postgres" -v ON_ERROR_STOP=1 -c "CREATE DATABASE \"${name}\";" >/dev/null
    echo "▶ applying extension schemas (offline build)"
    SQLX_OFFLINE=true DATABASE_URL="${db}" \
        cargo run --manifest-path crates/tests/Cargo.toml -p systemprompt-test-migrate --release
    echo "▶ running shard {{GROUP}} (live against migrated schema)"
    SQLX_OFFLINE=false DATABASE_URL="${db}" \
        bash scripts/test-shard.sh {{GROUP}} {{args}}

# Run every CI shard sequentially, each against its own fresh database.
# Bounded compile + run memory per shard (no OOM); same definitions as CI.
test-all-shards:
    #!/usr/bin/env bash
    set -euo pipefail
    for g in $(scripts/test-shard.sh --list); do
        echo "═══ shard: ${g} ═══"
        just test-shard "${g}"
    done

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
[unix]
api-test:
    #!/usr/bin/env bash
    echo "🧪 Starting API server with TEST database..."
    echo "📝 Database: database/test.db"
    echo ""
    export DATABASE_URL="database/test.db"
    ./target/debug/systemprompt serve api --foreground

[windows]
api-test:
    #!powershell
    Write-Host "🧪 Starting API server with TEST database..."
    Write-Host "📝 Database: database/test.db"
    Write-Host ""
    $env:DATABASE_URL = "database/test.db"
    .\target\debug\systemprompt.exe serve api --foreground

# Run full test workflow: setup DB → start API → run tests
[unix]
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

[windows]
test-full:
    #!powershell
    $ErrorActionPreference = "Stop"
    Write-Host "🧪 FULL TEST WORKFLOW"
    Write-Host "═══════════════════════════════════════════════════════════"
    Write-Host ""
    Write-Host "Step 1️⃣  Building project..."
    cargo build --bin systemprompt
    Write-Host ""
    Write-Host "⚠️  Step 2️⃣  Starting API in background with test database..."
    $env:DATABASE_URL = "database/test.db"
    $apiJob = Start-Job -ScriptBlock { & .\target\debug\systemprompt.exe serve api --foreground }
    Write-Host "   Waiting for API to start..."
    Start-Sleep -Seconds 3
    $port = netstat -ano | Select-String ":8080.*LISTENING"
    if (-not $port) {
        Write-Host "❌ API failed to start!"
        Stop-Job $apiJob -ErrorAction SilentlyContinue
        exit 1
    }
    Write-Host "✅ API started"
    Write-Host ""
    Write-Host "Step 3️⃣  Running tests..."
    Push-Location tests/integration
    $env:DATABASE_URL = "database/test.db"
    $testResult = 0
    try { npm test } catch { $testResult = 1 }
    Pop-Location
    Write-Host ""
    Write-Host "🧹 Cleaning up..."
    Stop-Job $apiJob -ErrorAction SilentlyContinue
    Remove-Job $apiJob -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 1
    Write-Host "═══════════════════════════════════════════════════════════"
    if ($testResult -eq 0) {
        Write-Host "✅ All tests passed!"
        exit 0
    } else {
        Write-Host "❌ Tests failed!"
        exit 1
    }

# Clean test database (remove test data)
[unix]
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

[windows]
test-clean:
    #!powershell
    Write-Host "🧹 Cleaning test database..."
    Write-Host ""
    $env:DATABASE_URL = "database/test.db"
    & .\target\debug\systemprompt.exe infra db execute "DELETE FROM task_artifacts WHERE artifact_id LIKE 'test-%' OR created_by LIKE 'test-%'"
    & .\target\debug\systemprompt.exe infra db execute "DELETE FROM user_contexts WHERE context_id LIKE 'test-%'"
    & .\target\debug\systemprompt.exe infra db execute "DELETE FROM user_sessions WHERE session_id LIKE 'test-%'"
    & .\target\debug\systemprompt.exe infra db execute "DELETE FROM ai_requests WHERE session_id LIKE 'test-%'"
    Write-Host "✅ Test data cleaned!"

# Reset test database completely (use with caution!)
[unix]
test-reset:
    #!/usr/bin/env bash
    echo "🧹 Resetting test database completely..."
    echo "⚠️  This will delete ALL data from test database!"
    echo "Press Enter to continue or Ctrl+C to abort..."
    read
    rm -f database/test.db
    just test-setup

[windows]
test-reset:
    #!powershell
    Write-Host "🧹 Resetting test database completely..."
    Write-Host "⚠️  This will delete ALL data from test database!"
    Write-Host "Press Enter to continue or Ctrl+C to abort..."
    Read-Host
    Remove-Item -Path database/test.db -ErrorAction SilentlyContinue
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
