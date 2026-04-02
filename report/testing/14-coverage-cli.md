# Entry: CLI Crate Coverage

## Current State

The CLI crate is the largest crate in the project with 447 source files spanning commands across admin (7+ files), analytics (11+ files), build, plugins, cloud, core, infrastructure, and web modules. Core commands include agents, skills, plugins, and hooks with create/edit/list/sync/show/validate operations.

Test coverage stands at 1.8% of source files (8 out of 447), with all 226 tests running synchronously in `crates/tests/unit/entry/cli/src/`.

### What IS Tested

- **cli_settings.rs**: Configuration loading and validation.
- **descriptor.rs**: `CommandDescriptor` method behavior (recently fixed to test methods rather than fields).
- **shared**: Utility function tests.
- **lib.rs**: Wrapper-level tests.

### What is NOT Tested

The following areas have zero test coverage:

- **All command execution**: create agent, create skill, sync, and every other command that the CLI exposes.
- **Command argument parsing validation**: No tests verify that clap argument definitions produce correct parsing behavior.
- **Interactive mode**: No tests for interactive prompts or user input handling.
- **File I/O operations**: Reading and writing configuration files, templates, and manifests.
- **Error handling paths**: No tests verify that error messages are correct or that failures are handled gracefully.
- **CLI output formatting**: No tests verify that table output, JSON output, or progress indicators render correctly.
- **Bootstrap/initialization logic**: No tests for the startup sequence.
- **admin commands** (7+ files): Zero tests.
- **analytics commands** (11+ files): Zero tests.
- **build commands**: Zero tests.
- **cloud commands**: Zero tests.
- **core commands**: Zero tests.
- **infrastructure commands**: Zero tests.
- **web commands**: Zero tests.

### Risk Assessment

The CLI is the primary user interface for developers interacting with the platform. Untested command execution means broken workflows will not be caught until users report them. With 447 source files and only 8 test files, the vast majority of CLI behavior is unverified. Argument parsing errors, incorrect file operations, and broken command flows are all possible regression vectors.

## Desired State

- Every command module has at least one test verifying successful execution with valid arguments.
- Argument parsing tests verify that required arguments are enforced and optional arguments have correct defaults.
- Error handling tests verify that user-facing error messages are clear and that exit codes are correct.
- File I/O operations are tested with temporary directories to verify read/write behavior.
- CLI output formatting tests verify table rendering, JSON output, and progress display.
- Overall CLI crate coverage reaches 30%+ of source files with meaningful assertions.

## How to Get There

### Phase 1: Command Argument Parsing (Highest Priority)

1. Write tests for each top-level command module (admin, analytics, build, cloud, core, infrastructure, web) verifying that clap definitions parse valid arguments correctly.
2. Write tests verifying that missing required arguments produce appropriate errors.
3. Write tests verifying that conflicting arguments are rejected.

### Phase 2: Core Command Execution

1. Write tests for agent commands (create, list, show, edit, sync) using mock service layers.
2. Write tests for skill commands (create, list, show, validate) with mock file systems.
3. Write tests for plugin and hook commands.
4. Each test should verify the command produces the expected output and side effects.

### Phase 3: Infrastructure and Admin Commands

1. Write tests for infrastructure commands (services start/stop/status, db migrate/status/query).
2. Write tests for admin commands (agents list/status, user management).
3. Write tests for cloud commands (auth, tenant, deploy).

### Phase 4: Error Handling and Edge Cases

1. Write tests for error paths in each command module (missing files, network errors, invalid input).
2. Write tests for interactive mode prompts.
3. Write tests for CLI output formatting (table rendering, JSON mode, verbose mode).

## Incremental Improvement Strategy

### Week 1-2: Argument Parsing Foundation

Target: 7 new test files, one per top-level command module, testing argument parsing with clap's `try_parse_from`. This establishes a baseline that catches argument definition regressions. Expected result: coverage rises to approximately 3.4%.

### Week 3-4: Core Commands (agent, skill, plugin)

Target: 10 new test files covering the most-used commands: agent create/list/show, skill create/list/validate, and plugin operations. These are the workflows developers use daily. Expected result: coverage rises to approximately 5.7%.

### Week 5-6: Infrastructure and Admin Commands

Target: 10 new test files covering infrastructure service management, database operations, and admin commands. Expected result: coverage rises to approximately 8%.

### Week 7-8: Cloud, Build, and Analytics Commands

Target: 10 new test files covering cloud authentication and deployment, build pipelines, and analytics commands. Expected result: coverage rises to approximately 10%.

### Ongoing

The CLI crate's size (447 files) means reaching high coverage percentages requires sustained effort. Enforce a policy that new commands ship with argument parsing tests and at least one execution test. Prioritize testing commands that users interact with most frequently. Target 30% coverage over two quarters, focusing on high-value command paths rather than exhaustive coverage of every file.
