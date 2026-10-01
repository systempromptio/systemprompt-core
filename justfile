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

# Security-critical lookups read the primary, never a lagging replica.
lint-authoritative-reads:
    ./scripts/lint-authoritative-reads.sh

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

# Serve the bridge GUI's web tree over HTTP so a browser can render it.
#
# The desktop webview is Windows/macOS only and reads its assets over a wry
# custom protocol, so this is the only way to see the GUI on Linux. Assets come
# off disk: edit CSS/JS/HTML and refresh, no rebuild between edits. Drive it
# with ?fixture=<name> — see bin/bridge/web/dev/fixtures and
# bin/bridge/README.md § Developing the GUI.
bridge-preview PORT="4310":
    cargo run --manifest-path bin/bridge/Cargo.toml --features dev-preview \
        --bin systemprompt-bridge -- dev-web --port {{PORT}}

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

# Prepare the workspace sqlx offline cache (development only; requires a
# running database). Deterministic: see scripts/sqlx-prepare.sh.
sqlx-prepare:
    scripts/sqlx-prepare.sh workspace

# Prepare per-crate SQLx caches for publishing (requires a running database).
#
# Each published crate ships its own `.sqlx/` so crates.io can build it offline.
# The crate set is derived from `cargo metadata`; `entry/api` is excluded there
# because it issues no SQL via `query!` macros (confirmed by
# `sqlx-verify-offline`). Every crate is cleaned before it is prepared so the
# result does not depend on target/ state, and a cache that shrinks is rejected
# unless PREPARE_ALLOW_PRUNE=1. `sqlx-verify-offline` is the correctness gate.
sqlx-prepare-publish:
    scripts/sqlx-prepare.sh publish

# Every per-crate .sqlx cache must hold only the queries its own src/ issues
# (ownership by SQL text). `sqlx-prepare-publish` prunes foreign entries as it
# goes; this is the read-only check, no database or rebuild needed.
sqlx-audit-caches:
    scripts/sqlx-audit-caches.sh

# Verify every SQLx crate compiles against its own per-crate .sqlx cache.
#
# Run from each crate directory so the macros resolve that crate's `.sqlx/`
# rather than the workspace root cache — that is the cache crates.io builds
# against. `cargo package` cannot be used here: pre-publish, the workspace's
# own path dependencies do not yet exist at the new version on the index, so
# it fails on resolution for reasons unrelated to the SQLx cache.
sqlx-verify-offline:
    #!/usr/bin/env bash
    set -e
    echo "Verifying offline compilation for all SQLx crates..."
    echo ""
    for crate in crates/infra/database crates/infra/events crates/infra/logging crates/infra/security \
                 crates/domain/analytics crates/domain/agent crates/domain/oauth crates/domain/users \
                 crates/domain/content crates/domain/files crates/domain/ai \
                 crates/domain/mcp crates/app/scheduler \
                 crates/entry/cli crates/entry/api; do
        echo "  Checking $crate..."
        (cd "$crate" && SQLX_OFFLINE=true cargo check --all-features)
    done
    echo ""
    echo "All crates verified for offline compilation!"

# Prepare the release bump commit on `next`: bump → sync pins/snippets → amend.
# Tagging, publishing, and main all happen later, via `just gate` → `just promote`
# → merge → tag → publish (canonical flow: internal/release-flow.md). The script
# itself is gitignored; this recipe is the discoverable entry point.
release BUMP="patch":
    @[ "{{BUMP}}" = "patch" ] || [ "{{BUMP}}" = "minor" ] || [ "{{BUMP}}" = "major" ] || \
        { echo "usage: just release [patch|minor|major]"; exit 2; }
    @[ -x scripts/release.sh ] || { echo "scripts/release.sh missing — see internal/release-flow.md"; exit 1; }
    ./scripts/release.sh {{BUMP}}

# Reject imperative SQL in declarative schema files
lint-schema:
    ./scripts/lint-schema.sh crates

# Reject inline SQL and hand-built migrations in extension.rs files
lint-extensions:
    ./scripts/lint-extensions.sh crates

# Check production comment syntax and Rustdoc placement; review meaning per AGENTS.md.
lint-comments:
    ./scripts/lint-inline-comments.sh

# Reject test-only seams (test features, test_api modules, unreachable_pub) in production crates
lint-test-seams:
    ./scripts/lint-test-seams.sh

# Reject tests that return early on a missing prerequisite without saying so
lint-silent-skips:
    ./scripts/lint-silent-skips.sh

# Reject inline `#[cfg(test)] mod tests` — tests belong in crates/tests/.
# Covers bin/bridge, which no root `--workspace` invocation reaches.
lint-inline-tests:
    ./scripts/lint-inline-tests.sh

# Reject upward crate dependencies and cycles in the layer stack
lint-layers:
    ./scripts/lint-layers.sh

# Reject ad-hoc Repository::new() outside composition roots
lint-repo-construction:
    ./scripts/lint-repo-construction.sh

# Every released version in CHANGELOG.md must carry its v<version> tag
check-release-tag:
    ./scripts/check-release-tag.sh

# Every crate whose sources changed since the last release must say so.
check-crate-changelogs:
    ./scripts/check-crate-changelogs.sh

# Every published version string must match [workspace.package].version
check-version-strings:
    ./scripts/check-version-strings.sh

# Tracked lockfiles must resolve systemprompt crates from the workspace or crates.io
check-lockfile-registry:
    ./scripts/check-lockfile-registry.sh

# Reject `From` impls that only restate the same fields, which say two types are one
lint-field-copy-from:
    python3 ./scripts/lint-field-copy-from.py

# Every source gate, in one list. quality.yml's `source-gates` job runs each
# recipe listed here as its own step (so one red gate cannot hide the rest)
# and asserts that its step list equals this line — a gate added here without
# a CI step fails the job, and a CI step for a gate not listed here fails it
# too. `check-release-tag`, `check-crate-changelogs`, `machete` and
# `bridge-bindings-check` are release-shaped (full history, cargo-machete, a
# bridge build) and stay CI-only by design.
check-gates: check-version-strings check-lockfile-registry lint-field-copy-from lint-env-vars lint-native-test-deps sqlx-audit-caches lint-discarded-results lint-fail-open lint-swallowed-errors lint-tracing-messages lint-async-trait lint-json-value lint-table-ownership lint-silent-skips lint-schema lint-extensions lint-comments lint-inline-tests lint-test-seams lint-test-value lint-raw-ids lint-sqlx lint-http-errors lint-no-untyped-admin check-headers lint-layers lint-repo-construction lint-authoritative-reads lint-bridge-lints-sync lint-bridge-css-tokens lint-bridge-i18n lint-bridge-js-imports lint-bridge-no-window lint-bridge-verdicts lint-bridge-typed-warnings lint-bridge-layers lint-bridge-globals lint-bridge-file-size lint-repo-hygiene
    cargo test --locked --manifest-path scripts/rust-contracts/Cargo.toml

# A public, code-only repository: a tracked prose file outside the sanctioned set
# (README/CHANGELOG, root AGENTS/CLAUDE/SECURITY, documentation/, the scripts/
# allowlists, test fixtures and the vendored A2A spec) is a status/plan/report
# doc that leaked in, as is a status/plan/report/summary/progress/findings-named
# prose file anywhere.
lint-repo-hygiene:
    #!/usr/bin/env bash
    set -euo pipefail
    prose='\.(md|txt|org|rst|adoc|pdf|docx)$'
    sanctioned='^(documentation/|crates/tests/|scripts/[^/]+\.txt$|(AGENTS|CLAUDE|SECURITY)\.md$|crates/domain/agent/docs/a2aspec\.txt$)|(^|/)(README|CHANGELOG)\.md$'
    hits=$( { git ls-files | grep -E "$prose" | grep -vE "$sanctioned" || true
              git ls-files | grep -iE "(status|plan|report|summary|progress|findings)[^/]*${prose}" || true
            } | sort -u)
    if [ -n "$hits" ]; then
        echo "✗ prose files outside the sanctioned set (CLAUDE.md § Repository Hygiene):" >&2
        echo "$hits" | sed 's/^/    /' >&2
        exit 1
    fi
    echo "✓ no stray status/plan/report docs tracked"

# Every source gate, then a workspace check
check: check-gates
    cargo check --workspace --keep-going

# Verify publishable caches without the development cache or warm workspace outputs.
check-offline:
    bash scripts/check-offline.sh

# Format code (nightly: rustfmt.toml uses unstable options).
# Covers the separate `crates/tests` and `bin/bridge` workspaces too — `--all`
# stops at the manifest it is invoked from, so each must be named explicitly.
fmt:
    cargo fmt --all
    cd crates/tests && cargo fmt --all
    cargo fmt --manifest-path bin/bridge/Cargo.toml --all

# Check formatting without making changes (main + test + bridge workspaces).
format-check:
    cargo fmt --all -- --check
    cd crates/tests && cargo fmt --all -- --check
    cargo fmt --manifest-path bin/bridge/Cargo.toml --all -- --check

# Build rustdoc with warnings as errors (main + test workspace).
# `--workspace` stops at the manifest it is invoked from, so the 86 test
# crates need their own pass — without it their `//!` heads go unchecked and
# intra-doc links rot silently.
# Local-only, like `just style-check`: the test workspace ships no `.sqlx`
# cache, so its `query!` fixtures need the live database that
# `crates/tests/.cargo/config.toml` points at. CI's docs job runs offline and
# covers the main workspace alone.
doc-check:
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features
    RUSTDOCFLAGS="-D warnings" cargo doc --manifest-path bin/bridge/Cargo.toml --no-deps
    cd crates/tests && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps

# Run clippy linter with strict settings (main workspace).
# The separate `crates/tests` workspace is clippied by `just style-check` (it
# needs a live database for its `query!` fixtures, which CI's lint job lacks);
# CI compiles it in the dedicated Test job instead.
# `--keep-going` mirrors CI: a compile error in one crate must not hide the
# clippy findings in every crate behind it (0.51.0 lost a gate round that way).
lint: lint-bridge lint-bridge-native-tests
    cargo clippy --workspace --all-targets --all-features --keep-going -- -D warnings

# `bin/bridge` is its own workspace, so the root `--workspace` clippy above
# never sees it.
#
# This lints the HOST target only. On Linux that configures out `src/gui/**`
# entirely (`lib.rs` gates `pub mod gui` on windows/macos), along with winproc,
# the Windows registry store, the keystore backends and the Windows/macOS
# scheduler code — so a green run here says nothing about any of them. CI does
# cover them, natively on both platforms, in quality.yml's `bridge-native`
# matrix; locally, use `just lint-bridge-native`.
lint-bridge:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo clippy --manifest-path bin/bridge/Cargo.toml -p systemprompt-bridge --all-targets --all-features --keep-going -- -D warnings
    if [ "$(uname -s)" = "Linux" ]; then
        echo "note: src/gui/** and the Windows/macOS-only modules were configured out of that run."
        echo "note: run 'just lint-bridge-native' before calling desktop work done."
    fi

# Lint the bridge code the host target configures out. On Linux this is the
# Windows cfg set via a cross-target check (clippy does not link, so no mingw
# toolchain is needed); macOS cannot be linted from Linux at all, because ring
# and objc2-exception-helper build scripts need a real cc — CI's `bridge-native`
# job covers that on a mac runner.
lint-bridge-native: lint-bridge-native-tests
    #!/usr/bin/env bash
    set -euo pipefail
    if [ "$(uname -s)" = "Linux" ]; then
        rustup target add x86_64-pc-windows-gnu
        cargo clippy --manifest-path bin/bridge/Cargo.toml -p systemprompt-bridge \
            --all-targets --all-features --keep-going --target x86_64-pc-windows-gnu -- -D warnings
        echo "note: macOS-only code is unlinted here; quality.yml's bridge-native job covers it."
    else
        cargo clippy --manifest-path bin/bridge/Cargo.toml -p systemprompt-bridge \
            --all-targets --all-features --keep-going -- -D warnings
    fi

# The native quality matrix compiles the bridge test crates with no database
# (SQLX_OFFLINE=true). Nothing on Linux mirrored that until 0.51.0 lost a gate
# round to a DB-backed fixture pulled into `bridge/install`. This is the
# offline build of exactly the crates quality.yml runs
# (scripts/bridge-native-crates.txt); the run itself stays on the native OS.
lint-bridge-native-tests:
    ./scripts/bridge-native-tests.sh --no-run

# Cheap, script-only form of the same failure: none of the native test crates
# may reach systemprompt-test-fixtures or a `sqlx::query*!` user, even
# transitively within crates/tests.
lint-native-test-deps:
    ./scripts/lint-native-test-deps.sh

# The bridge mirrors the root [workspace.lints] tables by hand (standalone
# workspace, no inheritance). Fail when the copies drift.
lint-bridge-lints-sync:
    ./scripts/lint-bridge-lints-sync.sh

# An undefined `var(--sp-*)` in the bridge UI drops the whole declaration
# silently — no console error, no build failure, just a rule that stops
# applying. Fail on it instead.
lint-bridge-css-tokens:
    ./scripts/lint-bridge-css-tokens.sh

# `t()` returns undefined for a key the catalogue does not carry, so an id that
# exists in the tree and not in bridge.ftl renders the English fallback -- and a
# `t()` written without one renders the string "undefined". Both are invisible
# until a user reports it, which is how `status-cloud-reach-label` shipped.
lint-bridge-i18n:
    ./scripts/lint-bridge-i18n.sh

# The web tree is plain ES modules -- no bundler, no type checker -- so a helper
# used without importing it is not a build error, it is a ReferenceError at
# first render that takes the whole pane down. `sp-profile.js` and
# `sp-activity-log.js` both shipped calling `t(...)` with no `import { t }`.
lint-bridge-js-imports:
    ./scripts/lint-bridge-js-imports.sh

# The bridge owns no console (`windows_subsystem = "windows"`), so a console
# child spawned without CREATE_NO_WINDOW gets its own window and the foreground.
# The tray redraw asked `schtasks` on every 30s probe tick, flashing a console at
# the user twice a minute and eating their keystrokes.
lint-bridge-no-window:
    ./scripts/lint-bridge-no-window.sh

# The IPC envelope's TypeScript bindings under bin/bridge/bindings/ are
# generated by ts-rs; a variant renamed in Rust used to leave them stale with
# nothing to say so.
bridge-bindings:
    cd crates/tests && TS_RS_EXPORT_DIR="$(pwd)/../../bin/bridge/bindings" cargo test -q -p systemprompt-bridge-ts-export-tests export_bindings -- --ignored

bridge-bindings-check:
    ./scripts/check-bridge-bindings.sh

# Every state on the GUI wire ships beside a verdict Rust computed. A JS branch
# on a state's *name* is how the Home card called four healthy MCP servers
# broken while Status, deriving it separately, called them fine.
lint-bridge-verdicts:
    ./scripts/lint-bridge-verdicts.sh

# The bridge is one crate, so `lint-layers` (a cargo-graph walk) cannot see
# its module structure; it had integration ⇄ sync, integration ⇄ install and
# a host installer reaching up into `gui`. This declares the module order and
# fails on any `crate::<module>` reference that points upward.
lint-bridge-layers:
    ./scripts/lint-bridge-layers.sh

# The bridge's service state lives on `BridgeContext`, built once and
# injected. A `static X: OnceLock<..>` can hold one value per process — the
# reason one test crate existed per proxy start outcome. Fail on any new one
# outside the script's reasoned allowlist.
lint-bridge-globals:
    ./scripts/lint-bridge-globals.sh

# The web tree has no bundler or type checker, so a file is only ever reviewed
# by reading it. The standard caps JS at 150 lines and CSS at 300; this makes
# the cap a failure instead of a note, so files are split before they sprawl.
lint-bridge-file-size:
    ./scripts/lint-bridge-file-size.sh

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
    # Stay in the repo root and point cargo-deny at each manifest, so the root
    # deny.toml is the config it discovers. `--config` has moved between global
    # and subcommand scope across cargo-deny releases; `--manifest-path` has not.
    for w in {{ workspaces }}; do
        echo "==> cargo deny: $w"
        cargo deny --manifest-path "${w%/}/Cargo.toml" check
    done

# Run cargo-audit across every workspace, on top of `just deny`.
#
# Both read the same RustSec database, but cargo-deny's `advisories` check does not
# surface advisories flagged `informational = "unsound"`, so those were invisible.
# cargo-audit does, and the script runs it with `--deny unsound` so a new one fails.
#
# The reason cargo-audit was dropped before — two ignore lists drifting apart — is
# handled by generating the `--ignore` flags from `deny.toml` on every run. There is
# still exactly one place a suppression is written down, and it is `deny.toml`.

# Run cargo-audit across every workspace (catches `unsound` advisories cargo-deny hides)
audit:
    ./scripts/cargo-audit-all.sh {{ workspaces }}

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

# Reject production source files exceeding 300 lines, `//!` head included
# (excludes target/ and crates/tests/; covers the crates, the facade and the
# bridge). A module head is part of the file a reviewer reads; a 120-line
# head over a 290-line body is a 410-line file.
#
# This used to print and exit 0 — awk returns 0 whether or not it matched — so
# the CI job named after it was green while 49 files were over the limit. It is
# a gate now: it prints the offenders largest-first and fails.
file-size:
    #!/usr/bin/env bash
    set -euo pipefail
    over=$(find crates systemprompt/src bin/bridge/src -name '*.rs' -not -path '*/target/*' -not -path '*/tests/*' \
        | xargs -r awk '{n[FILENAME]++} END {for (f in n) if (n[f]>300) print n[f], f}' \
        | sort -rn)
    if [ -n "$over" ]; then
        echo "$over"
        echo
        echo "file-size: $(echo "$over" | wc -l) file(s) over the 300-line limit." >&2
        exit 1
    fi
    echo "file-size: no source file exceeds 300 lines"

# Verify every production file has a doc head + BSL-1.1 license reference
check-headers:
    ./scripts/check-file-headers.sh

# Run custom style validators
validate:
    ./scripts/check-sqlx.sh

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
    cargo clippy --workspace --all-targets --all-features --keep-going -- -D warnings
    echo ""
    echo "3️⃣  Checking sqlx::query allowlist..."
    ./scripts/check-sqlx.sh
    echo ""
    echo "4️⃣  Checking HTTP error propagation..."
    ./scripts/check-http-errors.sh
    echo ""
    echo "5️⃣  Checking the test workspace (fmt + clippy + compile)..."
    (cd crates/tests && cargo fmt --all -- --check)
    (cd crates/tests && cargo clippy --workspace --all-targets --all-features --keep-going -- -D warnings)
    (cd crates/tests && cargo test --workspace --no-run)
    echo ""
    echo "6️⃣  Building rustdoc (both workspaces)..."
    just doc-check
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

# Real-DNS SSRF cases (security test 02): resolve cloud-metadata hostnames and
# assert the guarded resolver refuses them. Needs outbound DNS; skipped otherwise.
test-ssrf-live:
    SP_SSRF_NET_TESTS=1 cargo nextest run --manifest-path crates/tests/Cargo.toml \
        -p systemprompt-models-tests -E 'test(ssrf)' --no-capture

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
# Uses the workflow's test shards and source exclusions. Instrumented binaries
# share coverage-report/target (override with COVERAGE_TARGET_DIR); the migration
# tool uses crates/tests/target unless COVERAGE_MIGRATE_BIN supplies an executable.
# DATABASE_URL supplies connection settings and a disposable database prefix.
# Each run resets <database>_cov_build and <database>_cov_<shard> databases.
# Tests compile once against the migrated build database, then run from nextest
# metadata against isolated shard databases. Reports include text, JSON and LCOV.
# Instrumentation bypasses compiler wrappers and configured linker rustflags.
# CARGO_BUILD_JOBS defaults to 2 to limit concurrent compiler/linker memory use.
#
# --ignore-filename-regex also excludes sixteen process-entry files (twelve
# CLI, three domain supervisors, one bridge installer) — see the matching
# comment in .github/workflows/coverage.yml and keep the two regexes in sync.
# Each is a supervisor whose body is `spawn a subprocess / long-running
# server and wait`; there is no seam to drive one from a test without
# actually booting the thing it supervises. A file qualifies only if it
# cannot execute in-process at all — merely lacking a test is not enough,
# which is why the interactive dialoguer paths stay in the denominator.
# They are listed explicitly rather than by directory so that ordinary
# testable code added alongside them still counts. Keep this list in
# sync with the report and HTML filters and .github/workflows/coverage.yml:
#
#   commands/infrastructure/services/serve.rs      — blocking API server boot
#   commands/cloud/deploy/pipeline/orchestrator.rs — drives a live Fly deploy
#   commands/cloud/deploy/pipeline/artifacts.rs    — shells out to docker build
#   commands/cloud/tenant/create/cloud.rs          — provisions real cloud tenants
#   commands/admin/setup/docker.rs                 — spawns docker daemon setup
#   commands/admin/setup/docker_database.rs        — spawns a postgres container
#   commands/cloud/backup/client.rs                — streams a live backup socket
#   commands/admin/setup/docker_compose.rs         — renders and runs compose
#   commands/cloud/tenant/docker/container.rs      — drives live docker containers
#   agent/services/agent_orchestration/orchestrator/daemon.rs — supervises agents
#   bin/bridge/src/update/install/linux.rs         — replaces the running binary
coverage:
    #!/usr/bin/env bash
    set -euo pipefail
    ROOT="$(pwd)"
    PROFDIR="$ROOT/coverage-report/profraw"
    TDIR="${COVERAGE_TARGET_DIR:-$ROOT/coverage-report/target}"
    : "${CARGO_BUILD_JOBS:=2}"
    : "${CARGO_PROFILE_DEV_DEBUG:=line-tables-only}"
    : "${CARGO_INCREMENTAL:=0}"
    export CARGO_BUILD_JOBS CARGO_PROFILE_DEV_DEBUG CARGO_INCREMENTAL
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
    # CARGO_BUILD_JOBS caps concurrent linker invocations: instrumented test
    # binaries can exceed 2GB and the default ld OOM-kills under
    # 32-way parallelism even on 23GB RAM.
    # Build the `systemprompt` binary from the main workspace under the same
    # instrumentation flags so subprocess tests can invoke it and contribute
    # coverage. The crates/tests workspace doesn't include entry/cli, so its
    # `--bins` flag would not otherwise produce the binary.
    echo "==> Building instrumented systemprompt binary from main workspace"
    (cd "$ROOT" && CARGO_BUILD_RUSTC_WRAPPER="" RUSTC_WRAPPER="" \
        CARGO_TARGET_DIR="$TDIR" \
        LLVM_PROFILE_FILE="$PROFDIR/%m%c.profraw" \
        RUSTFLAGS="-C instrument-coverage -C llvm-args=--runtime-counter-relocation" \
        cargo build -p systemprompt-cli --bin systemprompt --jobs "$CARGO_BUILD_JOBS")
    export SYSTEMPROMPT_BIN="$TDIR/debug/systemprompt"

    # bin/bridge is its own workspace, so neither build above produces it. The
    # black-box suite spawns it via SP_BRIDGE_BIN and skips silently when the
    # path is absent, which is why bin/bridge/src/cli measured 66%. The spawned
    # process inherits LLVM_PROFILE_FILE, so its counters pool with the rest.
    echo "==> Building instrumented systemprompt-bridge binary"
    (cd "$ROOT" && CARGO_BUILD_RUSTC_WRAPPER="" RUSTC_WRAPPER="" \
        CARGO_TARGET_DIR="$TDIR" \
        LLVM_PROFILE_FILE="$PROFDIR/%m%c.profraw" \
        RUSTFLAGS="-C instrument-coverage -C llvm-args=--runtime-counter-relocation" \
        cargo build --manifest-path bin/bridge/Cargo.toml --bin systemprompt-bridge --jobs "$CARGO_BUILD_JOBS")
    export SP_BRIDGE_BIN="$TDIR/debug/systemprompt-bridge"

    # DATABASE_URL is required by subprocess_full.rs and other tests that
    # invoke the systemprompt binary through full SecretsBootstrap; without
    # it those tests early-return and produce no coverage.
    #
    # It must point at a disposable, freshly-migrated database, exactly as
    # coverage.yml does. Pointed at the shared dev `systemprompt-web` DB — the
    # default until 2026-08-03 — its web-project triggers fail core tests en
    # masse and the run under-reports by ~6 points (the 82.75% of the
    # 2026-07-21 baseline against the same tree that measures 88.91% here).
    #
    # %m%c (continuous mode, no %p): with per-PID files, PID reuse across the
    # ~18k nextest processes silently overwrites earlier profraws — tests
    # covered only by a single low-frequency process read as uncovered. One
    # mmap-shared file per module signature makes counter updates atomic and
    # mirrors coverage.yml.
    : "${DATABASE_URL:=postgres://systemprompt_admin:3e00fcdac26b5b731829e8737515db8f@localhost:5432/systemprompt_coverage}"
    cov_base="${DATABASE_URL%/*}"
    cov_name="${DATABASE_URL##*/}"
    cov_db_prefix="${cov_name}_cov"
    if [ -n "${COVERAGE_MIGRATE_BIN:-}" ]; then
        MIGRATE_BIN="$COVERAGE_MIGRATE_BIN"
        test -x "$MIGRATE_BIN"
    else
        echo "==> Building migration tool"
        (cd "$ROOT/crates/tests" && SQLX_OFFLINE=true \
            CARGO_TARGET_DIR="$ROOT/crates/tests/target" \
            CARGO_BUILD_RUSTC_WRAPPER="" RUSTC_WRAPPER="" \
            cargo build -p systemprompt-test-migrate --release \
            --jobs "$CARGO_BUILD_JOBS")
        MIGRATE_BIN="$ROOT/crates/tests/target/release/systemprompt-test-migrate"
    fi
    COVERAGE_BINARIES_METADATA="$ROOT/coverage-report/binaries.json"
    COVERAGE_CARGO_METADATA="$ROOT/coverage-report/cargo-metadata.json"
    build_db="${cov_db_prefix}_build"
    build_url="${cov_base}/${build_db}"
    echo "==> Preparing metadata build database: $build_db"
    psql "${cov_base}/postgres" -v ON_ERROR_STOP=1 \
        -c "DROP DATABASE IF EXISTS \"${build_db}\" WITH (FORCE);" \
        -c "CREATE DATABASE \"${build_db}\";" >/dev/null
    SQLX_OFFLINE=true DATABASE_URL="$build_url" "$MIGRATE_BIN"
    echo "==> Recording reusable nextest metadata"
    CARGO_BUILD_RUSTC_WRAPPER="" RUSTC_WRAPPER="" \
        CARGO_TARGET_DIR="$TDIR" DATABASE_URL="$build_url" \
        SQLX_OFFLINE=false cargo metadata \
        --manifest-path "$ROOT/crates/tests/Cargo.toml" --format-version 1 \
        > "$COVERAGE_CARGO_METADATA"
    CARGO_BUILD_RUSTC_WRAPPER="" RUSTC_WRAPPER="" \
        CARGO_TARGET_DIR="$TDIR" DATABASE_URL="$build_url" \
        SQLX_OFFLINE=false \
        RUSTFLAGS="-C instrument-coverage -C llvm-args=--runtime-counter-relocation" \
        cargo nextest list --manifest-path "$ROOT/crates/tests/Cargo.toml" \
        --workspace --lib --bins --tests --list-type binaries-only \
        --message-format json > "$COVERAGE_BINARIES_METADATA"
    echo "==> Running each test shard against a fresh disposable database"
    FALLBACK_PROFILE_ARCHIVE=$(mktemp -d "$ROOT/coverage-report/preexisting-fallback.XXXXXX")
    QUARANTINED_PROFILE_COUNT=$(bash "$ROOT/scripts/quarantine-fallback-profraw.sh" \
        "$ROOT/crates/tests" "$FALLBACK_PROFILE_ARCHIVE" \
        "$ROOT/crates/tests/target" "$TDIR")
    echo "==> Quarantined $QUARANTINED_PROFILE_COUNT pre-run fallback profraw files in $FALLBACK_PROFILE_ARCHIVE"
    FALLBACK_PROFILE_MARKER="$ROOT/coverage-report/profile-run-start"
    touch "$FALLBACK_PROFILE_MARKER"
    for group in $(bash "$ROOT/scripts/test-shard.sh" --list); do
        db="${cov_db_prefix}_${group//-/_}"
        shard_url="${cov_base}/${db}"
        echo "==> Resetting coverage database: $db"
        psql "${cov_base}/postgres" -v ON_ERROR_STOP=1 \
            -c "DROP DATABASE IF EXISTS \"${db}\" WITH (FORCE);" \
            -c "CREATE DATABASE \"${db}\";" >/dev/null
        echo "==> Applying extension schemas for $group"
        SQLX_OFFLINE=true DATABASE_URL="$shard_url" "$MIGRATE_BIN"
        set +e
        CARGO_BUILD_RUSTC_WRAPPER="" \
            RUSTC_WRAPPER="" \
            CARGO_TARGET_DIR="$TDIR" \
            LLVM_PROFILE_FILE="$PROFDIR/%m%c.profraw" \
            RUSTFLAGS="-C instrument-coverage -C llvm-args=--runtime-counter-relocation" \
            SYSTEMPROMPT_BIN="$SYSTEMPROMPT_BIN" \
            SP_BRIDGE_BIN="$SP_BRIDGE_BIN" \
            COVERAGE_BINARIES_METADATA="$COVERAGE_BINARIES_METADATA" \
            COVERAGE_CARGO_METADATA="$COVERAGE_CARGO_METADATA" \
            NEXTEST_PROFILE=coverage \
            DATABASE_URL="$shard_url" \
            bash "$ROOT/scripts/test-shard.sh" "$group" \
            --bins --tests --no-fail-fast
        shard_status=$?
        set -e
        if [ "$shard_status" -ne 0 ]; then
            echo "warning: shard $group failed with status $shard_status"
            TEST_STATUS=1
        fi
    done

    FALLBACK_PROFILE_COUNT=$(bash "$ROOT/scripts/collect-fallback-profraw.sh" \
        "$ROOT/crates/tests" "$FALLBACK_PROFILE_MARKER" "$PROFDIR" \
        "$ROOT/crates/tests/target" "$TDIR")
    echo "==> Collected $FALLBACK_PROFILE_COUNT run-scoped fallback profraw files"

    PROFRAW_COUNT=$(find "$PROFDIR" -name "*.profraw" | wc -l)
    echo "==> Generated $PROFRAW_COUNT profraw files"

    LLVM_PROFDATA=$(rustc --print sysroot)/lib/rustlib/x86_64-unknown-linux-gnu/bin/llvm-profdata
    LLVM_COV=$(rustc --print sysroot)/lib/rustlib/x86_64-unknown-linux-gnu/bin/llvm-cov

    echo "==> Merging profile data"
    find "$PROFDIR" -name '*.profraw' > "$ROOT/coverage-report/profraw-list.txt"
    "$LLVM_PROFDATA" merge -sparse -f "$ROOT/coverage-report/profraw-list.txt" -o "$ROOT/coverage-report/tests.profdata"

    BINS=$(jq -r '."rust-binaries"[]."binary-path"' "$ROOT/coverage-report/binaries.json" | sort -u)
    SP_BIN="$TDIR/debug/systemprompt"
    [ -x "$SP_BIN" ] && BINS="$BINS $SP_BIN"
    BRIDGE_BIN="$TDIR/debug/systemprompt-bridge"
    [ -x "$BRIDGE_BIN" ] && BINS="$BINS $BRIDGE_BIN"
    OBJ_ARGS=""
    for b in $BINS; do OBJ_ARGS="$OBJ_ARGS --object $b"; done
    IGNORE_REGEX="(\.cargo|rustc|crates/tests|/debug/build/[^/]+/out/|$HOME/\.cargo|crates/domain/(agent/src/services/(a2a_server/standalone|agent_orchestration/orchestrator/daemon)|mcp/src/services/orchestrator/daemon)\.rs|crates/entry/cli/src/commands/(infrastructure/services/serve|cloud/deploy/pipeline/(orchestrator|artifacts)|cloud/tenant/create/cloud|admin/setup/docker(_database|_compose)?|cloud/tenant/docker/container|cloud/backup/(client|mod)|plugins/run|admin/agents/run)\.rs|bin/bridge/src/update/install/linux\.rs)"

    echo "==> Coverage report"
    "$LLVM_COV" report \
        --instr-profile="$ROOT/coverage-report/tests.profdata" \
        $OBJ_ARGS \
        --ignore-filename-regex="$IGNORE_REGEX" \
        --summary-only \
        | tee "$ROOT/coverage-report/coverage-summary.txt"

    "$LLVM_COV" export \
        --instr-profile="$ROOT/coverage-report/tests.profdata" \
        $OBJ_ARGS \
        --ignore-filename-regex="$IGNORE_REGEX" \
        --format=text --summary-only \
        > "$ROOT/coverage-report/coverage-summary.json"

    "$LLVM_COV" export \
        --instr-profile="$ROOT/coverage-report/tests.profdata" \
        $OBJ_ARGS \
        --ignore-filename-regex="$IGNORE_REGEX" \
        --format=lcov \
        > "$ROOT/coverage-report/lcov.info"

    echo ""
    echo "coverage-summary.txt: coverage-report/coverage-summary.txt"
    echo "coverage-summary.json: coverage-report/coverage-summary.json"
    echo "lcov.info: coverage-report/lcov.info"
    echo "For HTML report: just coverage-html"

    # Why: the report is produced first so the number stays available for
    # diagnosis, but a run whose tests failed measured a partial binary set and
    # must not exit 0 -- that reads as a healthy percentage of everything.
    if [ "${TEST_STATUS:-0}" -ne 0 ]; then
        echo "coverage: tests failed above; this figure describes a partial run" >&2
        exit 1
    fi

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
    BINS=$(jq -r '."rust-binaries"[]."binary-path"' "$ROOT/coverage-report/binaries.json" | sort -u)
    SP_BIN="$TDIR/debug/systemprompt"
    [ -x "$SP_BIN" ] && BINS="$BINS $SP_BIN"
    BRIDGE_BIN="$TDIR/debug/systemprompt-bridge"
    [ -x "$BRIDGE_BIN" ] && BINS="$BINS $BRIDGE_BIN"
    OBJ_ARGS=""
    for b in $BINS; do OBJ_ARGS="$OBJ_ARGS --object $b"; done
    mkdir -p "$ROOT/coverage-report/html"
    "$LLVM_COV" show \
        --instr-profile="$ROOT/coverage-report/tests.profdata" \
        $OBJ_ARGS \
        --ignore-filename-regex="(\.cargo|rustc|crates/tests|/debug/build/[^/]+/out/|$HOME/\.cargo|crates/domain/(agent/src/services/(a2a_server/standalone|agent_orchestration/orchestrator/daemon)|mcp/src/services/orchestrator/daemon)\.rs|crates/entry/cli/src/commands/(infrastructure/services/serve|cloud/deploy/pipeline/(orchestrator|artifacts)|cloud/tenant/create/cloud|admin/setup/docker(_database|_compose)?|cloud/tenant/docker/container|cloud/backup/(client|mod)|plugins/run|admin/agents/run)\.rs|bin/bridge/src/update/install/linux\.rs)" \
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
    db="${TEST_DATABASE_URL:-postgres://systemprompt_admin:3e00fcdac26b5b731829e8737515db8f@localhost:5432/systemprompt_test}"
    base="${db%/*}"
    name="${db##*/}"
    echo "▶ resetting test database: ${name}"
    psql "${base}/postgres" -v ON_ERROR_STOP=1 -c "DROP DATABASE IF EXISTS \"${name}\" WITH (FORCE);" >/dev/null
    psql "${base}/postgres" -v ON_ERROR_STOP=1 -c "CREATE DATABASE \"${name}\";" >/dev/null
    echo "▶ applying extension schemas (offline build)"
    SQLX_OFFLINE=true DATABASE_URL="${db}" \
        cargo run --manifest-path crates/tests/Cargo.toml -p systemprompt-test-migrate
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
# Groups: shared infra domain app-runtime app-scheduler app-generator entry-api entry-cli bridge integration-api integration-cli integration-rest-1 integration-rest-2 edge
test-shard GROUP *args:
    #!/usr/bin/env bash
    set -euo pipefail
    command -v cargo-nextest >/dev/null 2>&1 || {
        echo "cargo-nextest not found — run 'just install-nextest' first" >&2
        exit 1
    }
    db="${TEST_DATABASE_URL:-postgres://systemprompt_admin:3e00fcdac26b5b731829e8737515db8f@localhost:5432/systemprompt_test}"
    base="${db%/*}"
    name="${db##*/}"
    echo "▶ resetting test database: ${name}"
    psql "${base}/postgres" -v ON_ERROR_STOP=1 -c "DROP DATABASE IF EXISTS \"${name}\" WITH (FORCE);" >/dev/null
    psql "${base}/postgres" -v ON_ERROR_STOP=1 -c "CREATE DATABASE \"${name}\";" >/dev/null
    echo "▶ applying extension schemas (offline build)"
    SQLX_OFFLINE=true DATABASE_URL="${db}" \
        cargo run --manifest-path crates/tests/Cargo.toml -p systemprompt-test-migrate
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

    ./target/debug/systemprompt infra db "$@"

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

# Assign admin role to a user (by username or email)
assign-admin USER:
    ./target/debug/systemprompt infra db assign-admin {{USER}}

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

# Read the candidate's green push run on next, then the promotion PR proof if one
# is open. Read-only; never dispatches.
gate REF="origin/next":
    python3 scripts/release-proof.py gate "{{REF}}"

# Open the release pull request that proves a frozen candidate for protected `main`.
#
# `main` refuses direct pushes, so a PR is the only way in. The commit is frozen
# on the `promote` ref first: a PR headed at `next` would merge whatever `next`
# points at when you merge it, so anything pushed meanwhile would ride along
# ungated. Refuses a SHA whose push run on next is missing, pending or red.
# This only OPENS the PR — you review and merge it.
promote SHA="":
    #!/usr/bin/env bash
    set -euo pipefail
    REPO=systempromptio/systemprompt-core
    SHA="{{SHA}}"; [ -n "$SHA" ] || SHA=$(git rev-parse origin/next)
    SHA=$(git rev-parse "$SHA")
    git fetch -q origin main
    if git merge-base --is-ancestor "$SHA" origin/main; then
        echo "main already contains ${SHA:0:9} — nothing to promote."; exit 0
    fi
    python3 scripts/release-proof.py pushed "$SHA"
    echo "Release PR will carry ${SHA:0:9} onto main:"
    git log --oneline origin/main.."$SHA" | sed 's/^/    /'
    git push --force origin "$SHA:refs/heads/promote"
    NUM=$(gh pr list -R "$REPO" --base main --head promote --state open --json number --jq '.[0].number // empty')
    if [ -z "$NUM" ]; then
        NUM=$(gh api -X POST "repos/$REPO/pulls" -f title="Release: promote next to main" \
                -f head=promote -f base=main \
                -f body="Frozen at $SHA. The required checks (CI, Quality, Supply Chain) run on this PR." --jq .number)
    fi
    echo
    echo "Opened https://github.com/$REPO/pull/$NUM"
    echo "Review it, then merge when you are ready:  gh pr merge $NUM --merge"

lint-discarded-results:
    ./scripts/check-discarded-results.sh

# A tracing message is a constant; values are structured fields.
lint-tracing-messages:
    ./scripts/lint-tracing-messages.sh

# #[async_trait] only for dyn-compatibility, and the trait says so.
lint-async-trait:
    ./scripts/lint-async-trait.sh

# serde_json::Value in a signature is a protocol boundary with a `// JSON:` line.
lint-json-value:
    ./scripts/lint-json-value.sh

# A crate queries only the tables its own schema/*.sql declares (infra may read infra).
lint-table-ownership:
    ./scripts/lint-table-ownership.sh

# Profiles are the source of truth; an env read outside the sanctioned boot
# readers (scripts/env-var-allowlist.txt) is an undocumented kill switch.
lint-env-vars:
    ./scripts/lint-env-vars.sh

lint-fail-open:
    ./scripts/check-fail-open.sh

# A `map_err(|_…| …)` on a network or database result that never logs what
# it saw: the closure names the error or logs it.
lint-swallowed-errors:
    ./scripts/lint-swallowed-errors.sh

# Bridge control flow decides on a host warning's `kind`, never its text.
lint-bridge-typed-warnings:
    ./scripts/lint-bridge-typed-warnings.sh
