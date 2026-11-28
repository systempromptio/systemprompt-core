# Migrated configuration tests: per-case results

Batch 2 of the reference-test migration (lane S2-B3). Each catalogue row of this batch is replaced here by one row per case, in the columns of `documentation/future/harness/testing/04-test-migration.md` section 2. `target` names a test as `<crate>::<module>::<function>`, where `config` is `systemprompt-migrated-config-tests` and `session` is `systemprompt-migrated-session-tests`; a `covered` target names an existing test elsewhere. `state` is `passing`, `ledger` (the harness differs; the test, when there is one, is `#[ignore = "parity ledger: ..."]` and asserts the reference behaviour, and `note` gives the observed value) `written` (a test exists that has not yet been run by the integrator) or `open` (not written; `note` says why).

Cases: 199 over 37 catalogue rows. ported 135, covered 5, not applicable 34, deferred 0, difference 23, open 0, written 2.

## Rows

| row | source | scope | cases | ported | covered | n/a | deferred | difference | open | written |
|---|---|---|---|---|---|---|---|---|---|---|
| AUI-87f43a30 | `packages/app/src/providers/models/agent.test.ts` |  | 2 | 2 | 0 | 0 | 0 | 0 | 0 | 0 |
| AUI-dc59bfdb | `packages/app/src/settings/workspaces/project-lsp.test.ts` |  | 3 | 3 | 0 | 0 | 0 | 0 | 0 | 0 |
| ENG-32c83534.1 | `packages/core/test/agent.test.ts` | cases about default ordering and built-in agent permissions | 3 | 2 | 0 | 0 | 0 | 1 | 0 | 0 |
| ENG-b25903a9 | `packages/core/test/config/agent.test.ts` |  | 18 | 14 | 0 | 4 | 0 | 0 | 0 | 0 |
| ENG-9f03c999.2 | `packages/core/test/config/command.test.ts` | cases about reloading on file changes | 5 | 5 | 0 | 0 | 0 | 0 | 0 | 0 |
| ENG-5b32fe8b | `packages/core/test/config/compaction.test.ts` |  | 1 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| ENG-eee2b4c4.1 | `packages/core/test/config/config.test.ts` | cases about discovery, layering, substitution and overrides | 16 | 6 | 2 | 4 | 0 | 4 | 0 | 0 |
| ENG-eee2b4c4.2 | `packages/core/test/config/config.test.ts` | cases about watching and reloading | 7 | 6 | 0 | 0 | 0 | 1 | 0 | 0 |
| ENG-6b14bdbe | `packages/core/test/config/discovery.test.ts` |  | 3 | 1 | 0 | 1 | 0 | 0 | 0 | 1 |
| ENG-e7da378b | `packages/core/test/config/entry-observer.test.ts` |  | 1 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| ENG-699bf0ff.2 | `packages/core/test/config/markdown.test.ts` | cases about repair edge cases and repeatability | 11 | 8 | 1 | 0 | 0 | 2 | 0 | 0 |
| ENG-397ffe19 | `packages/core/test/config/model.test.ts` |  | 2 | 2 | 0 | 0 | 0 | 0 | 0 | 0 |
| ENG-c21c79ed.1 | `packages/core/test/config/normalization.test.ts` | cases about tolerant decoding, per-entry recovery and diagnostics | 10 | 7 | 0 | 1 | 0 | 2 | 0 | 0 |
| ENG-ac00faab | `packages/core/test/config/reload.test.ts` |  | 7 | 5 | 0 | 2 | 0 | 0 | 0 | 0 |
| ENG-2841d499.1 | `packages/core/test/config/skill.test.ts` | cases about parsing, precedence and rescanning local skill directories | 11 | 10 | 0 | 1 | 0 | 0 | 0 | 0 |
| ENG-f378bae6 | `packages/core/test/config/snapshot.test.ts` |  | 1 | 0 | 0 | 0 | 0 | 1 | 0 | 0 |
| ENG-a7cef8c3 | `packages/core/test/config/watch.test.ts` |  | 2 | 2 | 0 | 0 | 0 | 0 | 0 | 0 |
| ENG-39e0a118 | `packages/core/test/instance-vanilla.test.ts` |  | 1 | 0 | 0 | 1 | 0 | 0 | 0 | 0 |
| ENG-77fb49ea | `packages/core/test/instruction-discovery.test.ts` |  | 14 | 8 | 0 | 2 | 0 | 4 | 0 | 0 |
| ENG-6a70c726.2 | `packages/core/test/location-layer.test.ts` | disabling a built-in agent | 1 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| ENG-9ea19c3a.1 | `packages/core/test/permission.test.ts` | rule evaluation and precedence | 19 | 12 | 0 | 4 | 0 | 3 | 0 | 0 |
| ENG-9c4cca40 | `packages/core/test/plugin/command.test.ts` |  | 1 | 0 | 0 | 1 | 0 | 0 | 0 | 0 |
| ENG-85d6a31a | `packages/core/test/plugin/plan.test.ts` |  | 6 | 1 | 2 | 0 | 0 | 2 | 0 | 1 |
| CLI-a9bfd4e0.3 | `packages/cli/test/config.test.ts` | cross-process locking of settings writes | 2 | 0 | 0 | 0 | 0 | 2 | 0 | 0 |
| CLI-9aa7ddae.2 | `packages/cli/test/debug-config.test.ts` | secret redaction | 1 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| SRV-5d58e04b.1 | `packages/server/test/config.test.ts` | cases about listing configuration layers | 1 | 0 | 0 | 0 | 0 | 1 | 0 | 0 |
| GPL-ded2881b.2 | `packages/gui-plugins/src/file/artifact.test.ts` | cases about front-matter parsing | 2 | 2 | 0 | 0 | 0 | 0 | 0 | 0 |
| GPL-dac877e6 | `packages/plugin-browser/test/wildcard.test.ts` |  | 27 | 27 | 0 | 0 | 0 | 0 | 0 | 0 |
| NRS-5e155f32 | `packages/cli-rs/crates/local/src/jsonc.rs` |  | 1 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| NRS-a2041a6b | `packages/cli-rs/crates/local/src/paths.rs` |  | 5 | 0 | 0 | 5 | 0 | 0 | 0 | 0 |
| NRS-9fe1300e | `packages/cli-rs/crates/local/src/watch.rs` |  | 5 | 3 | 0 | 2 | 0 | 0 | 0 | 0 |
| NRS-704c3050.2 | `packages/cli-rs/crates/local/tests/all/config_interop.rs` | case about watching a not-yet-existing settings directory | 1 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| NRS-55f17bf3.2 | `packages/cli-rs/crates/tui/src/plugins/host.rs` | re-created watched directory | 1 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| OTH-6c23742c | `packages/schema/test/agent.test.ts` |  | 1 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| OTH-5c9a8967.1 | `packages/schema/test/config.test.ts` | permission policy statement cases | 2 | 1 | 0 | 1 | 0 | 0 | 0 | 0 |
| OTH-32b19ac5 | `packages/util/test/global-roots.test.ts` |  | 2 | 0 | 0 | 2 | 0 | 0 | 0 | 0 |
| OTH-2b6a9e9d | `packages/util/test/global.test.ts` |  | 3 | 0 | 0 | 3 | 0 | 0 | 0 | 0 |

## Cases

| id | case | class | reason | target | needs | state | note |
|---|---|---|---|---|---|---|---|
| AUI-87f43a30:4 | hasCustomAgent detects only explicitly custom agents | port |  | config::agent_tests::a_custom_agent_counts_as_primary_only_when_its_mode_says_so | unit | passing |  |
| AUI-87f43a30:23 | resolveAgent uses $name | port |  | config::agent_tests::the_default_agent_is_the_configured_one_when_usable_otherwise_the_first_primary | unit | passing |  |
| AUI-dc59bfdb:17 | merges inherited entries and project overrides by name | port |  | config::settings_tests::configured_language_servers_merge_by_name_across_layers | unit | passing |  |
| AUI-dc59bfdb:44 | does not invent servers for omitted or boolean-only configuration | port |  | config::settings_tests::no_language_server_is_invented_when_the_setting_is_absent | unit | passing |  |
| AUI-dc59bfdb:56 | allows named configuration after a disabled parent without reviving earlier entries | port |  | config::settings_tests::a_disabled_language_server_in_a_nearer_layer_does_not_revive_an_earlier_entry | unit | passing |  |
| ENG-32c83534.1:70 | lists the selected default agent first | port |  | config::agent_tests::the_agent_listing_puts_the_selected_default_agent_first | unit | ledger | observed: the first primary agent is "build", table order is kept |
| ENG-32c83534.1:154 | allows external directories without opting built-in agents into bash | port |  | config::permission_tests::built_in_agents_gain_no_shell_access_from_an_external_directory_allowance | unit | passing |  |
| ENG-32c83534.1:204 | denies the subagent tool for built-in subagents | port |  | config::agent_tests::built_in_subagents_cannot_start_further_subagents | unit | passing |  |
| ENG-b25903a9:35 | ignores named agent color tokens | port |  | config::agent_tests::an_agent_colour_is_kept_as_given | unit | passing |  |
| ENG-b25903a9:41 | keeps schema fields and name out of legacy agent options | not-applicable | reference-internal: legacy agent file formats of the reference |  | unit | - |  |
| ENG-b25903a9:93 | loads Markdown ${item.name}${native ? " with native request and permissions" : ""} | port |  | config::agent_tests::a_markdown_agent_loads_its_fields_prompt_and_rules | unit | passing |  |
| ENG-b25903a9:124 | keeps Markdown legacy permission precedence in source order | port |  | config::agent_tests::rules_in_an_agent_file_keep_their_source_order | unit | passing |  |
| ENG-b25903a9:133 | loads Markdown agents with flow collections alongside unquoted-colon descriptions | port |  | config::agent_tests::an_agent_file_with_flow_collections_and_an_unquoted_colon_still_loads | unit | passing |  |
| ENG-b25903a9:148 | loads Markdown legacy temperature ${variant ? "with" : "without"} a separate variant | port |  | config::agent_tests::a_temperature_with_and_without_other_sampling_keys_is_kept | unit | passing |  |
| ENG-b25903a9:159 | matches POSIX paths against home-relative permissions | port |  | config::agent_tests::home_relative_rule_patterns_match_posix_and_windows_spellings_of_the_path | unit | passing |  |
| ENG-b25903a9:172 | matches Windows paths against home-relative permissions | port |  | config::agent_tests::home_relative_rule_patterns_match_posix_and_windows_spellings_of_the_path | unit | passing |  |
| ENG-b25903a9:189 | applies remote permission defaults before explicit global and build rules | not-applicable | no-feature: remote (well-known) configuration documents |  | unit | - |  |
| ENG-b25903a9:279 | applies all global permissions before agent-specific permissions | port |  | config::permission_tests::global_rules_apply_before_agent_specific_ones | unit | passing |  |
| ENG-b25903a9:369 | maps configured agent fields and preserves an unspecified model variant | port |  | config::agent_tests::configured_agent_fields_override_and_an_unset_model_stays_unset | unit | passing |  |
| ENG-b25903a9:431 | removes a built-in agent disabled by configuration | port |  | config::agent_tests::a_built_in_agent_disabled_in_settings_disappears | unit | passing |  |
| ENG-b25903a9:452 | loads legacy file-based agents from config directories | not-applicable | reference-internal: legacy agent file formats of the reference |  | unit | - |  |
| ENG-b25903a9:538 | reads agent files that use V2 fields as V2 | not-applicable | reference-internal: legacy agent file formats of the reference |  | unit | - |  |
| ENG-b25903a9:592 | rebuilds agents when a source file is ${testCase.name} | port |  | config::agent_tests::an_agent_file_that_changes_makes_the_configuration_stale_and_a_reload_picks_it_up | unit | passing |  |
| ENG-b25903a9:627 | coalesces updates inside the debounce window into one rebuild | port |  | config::command_tests::a_burst_of_edits_is_seen_as_one_change_with_the_final_content | unit | passing |  |
| ENG-b25903a9:664 | rebuilds on a config update published immediately after startup | port |  | config::agent_tests::an_agent_file_that_changes_makes_the_configuration_stale_and_a_reload_picks_it_up | unit | passing |  |
| ENG-b25903a9:688 | ignores updates outside agent source directories | port |  | config::agent_tests::a_change_outside_agent_sources_leaves_the_configuration_current | unit | passing |  |
| ENG-9f03c999.2:287 | rebuilds commands when a source file is ${testCase.name} | port |  | config::command_tests::adding_changing_and_deleting_a_command_file_shows_on_the_next_load | unit | passing |  |
| ENG-9f03c999.2:330 | coalesces updates inside the debounce window into one rebuild | port |  | config::command_tests::a_burst_of_edits_is_seen_as_one_change_with_the_final_content | unit | passing |  |
| ENG-9f03c999.2:369 | rebuilds on a config update published immediately after startup | port |  | config::command_tests::a_command_written_right_after_the_first_load_is_picked_up | unit | passing |  |
| ENG-9f03c999.2:394 | ignores updates outside command source directories | port |  | config::command_tests::a_change_outside_command_directories_leaves_commands_current | unit | passing |  |
| ENG-9f03c999.2:438 | reloads commands from real file edits | port |  | config::command_tests::adding_changing_and_deleting_a_command_file_shows_on_the_next_load | unit | passing |  |
| ENG-5b32fe8b:58 | merges settings and reloads changed config | port |  | config::settings_tests::compaction_settings_merge_leaf_by_leaf_across_layers | unit | passing |  |
| ENG-eee2b4c4.1:98 | excludes home-level claude and agents directories when global is disabled | port |  | config::settings_tests::turning_compatibility_directories_off_leaves_only_the_harness_locations | unit | passing |  |
| ENG-eee2b4c4.1:141 | excludes claude directories when claude is disabled | port |  | config::settings_tests::turning_compatibility_directories_off_leaves_only_the_harness_locations | unit | passing |  |
| ENG-eee2b4c4.1:186 | excludes global config reached through the project walk when global is disabled | not-applicable | no-feature: switching off global configuration discovery |  | unit | - |  |
| ENG-eee2b4c4.1:252 | discovers the global config directory once when the project walk reaches it | port |  | config::settings_tests::a_settings_file_reached_both_as_global_and_by_the_project_walk_is_read_once | unit | passing |  |
| ENG-eee2b4c4.1:385 | loads explicit file and content overrides in priority order | not-applicable | no-feature: explicit settings file and inline content overrides from the host |  | unit | - |  |
| ENG-eee2b4c4.1:672 | skips project configuration when project discovery is disabled | not-applicable | no-feature: switching off project configuration discovery |  | unit | - |  |
| ENG-eee2b4c4.1:736 | excludes missing files under symlinked global roots when global is disabled | not-applicable | no-feature: switching off global configuration discovery |  | unit | - |  |
| ENG-eee2b4c4.1:1000 | returns the latest defined scalar from priority-ordered documents | covered | same | systemprompt-engine-tests::config::settings_tests::global_and_project_merge | unit | passing |  |
| ENG-eee2b4c4.1:1251 | logs redacted source-aware diagnostics for every config source | port |  | config::settings_tests::server_credentials_never_appear_in_debug_output | unit | passing |  |
| ENG-eee2b4c4.1:1606 | returns an empty configuration when directory files do not exist | covered | same | systemprompt-engine-tests::config::settings_tests::missing_and_empty_files | unit | passing |  |
| ENG-eee2b4c4.1:1654 | loads reference JSON and JSONC files from lowest to highest priority | port |  | config::settings_tests::settings_load_from_global_then_ancestors_then_the_working_directory | unit | passing |  |
| ENG-eee2b4c4.1:1699 | substitutes environment variables and relative file contents | port |  | config::settings_tests::file_substitutions_in_settings_values_are_resolved | unit | ledger | observed: the value is kept literally: Some("{file:./model.txt}") |
| ENG-eee2b4c4.1:1757 | keeps env values unescaped when the config already parses | port |  | config::settings_tests::file_substitutions_in_settings_values_are_resolved | unit | ledger | observed: the value is kept literally: Some("{file:./model.txt}") |
| ENG-eee2b4c4.1:1789 | accepts $schema metadata without writing it into config files | port |  | config::settings_tests::a_schema_key_is_accepted_and_kept_as_written | unit | passing |  |
| ENG-eee2b4c4.1:1997 | ignores an invalid file while loading valid config values | port |  | config::settings_tests::an_invalid_file_is_skipped_while_valid_files_still_load | unit | ledger | observed: the load fails with a parse error for the invalid file |
| ENG-eee2b4c4.1:2024 | loads global and ancestor configuration across the project boundary | port |  | config::settings_tests::settings_above_the_project_root_still_apply | unit | ledger | observed: model is None: files above the worktree root are not read |
| ENG-eee2b4c4.2:702 | reloads file substitutions when their source changes | port |  |  | unit | ledger | asserts: a file substituted into a setting is watched and reloads it; observed: no substitution exists, so nothing is watched |
| ENG-eee2b4c4.2:765 | exposes filesystem updates under config roots through changes | port |  | config::reload_tests::a_settings_file_that_changes_makes_the_load_stale | unit | passing |  |
| ENG-eee2b4c4.2:816 | does not reload when an unrelated file under a config root changes | port |  | config::reload_tests::an_unrelated_file_change_does_not_make_the_load_stale | unit | passing |  |
| ENG-eee2b4c4.2:863 | reloads canonical events for a symlinked global root, even after removal | port |  | config::reload_tests::a_settings_file_reached_through_a_symlink_reloads_when_its_target_changes | unit | passing |  |
| ENG-eee2b4c4.2:907 | reloads ${source} config when its symlink target changes under a watched root | port |  | config::reload_tests::a_settings_file_reached_through_a_symlink_reloads_when_its_target_changes | unit | passing |  |
| ENG-eee2b4c4.2:948 | keeps watching a deleted config file so recreating it reloads | port |  | config::reload_tests::a_deleted_then_recreated_settings_file_is_noticed_both_times | unit | passing |  |
| ENG-eee2b4c4.2:1621 | does not recursively watch ecosystem config roots | port |  | config::reload_tests::candidate_files_that_do_not_exist_yet_are_part_of_what_a_load_watches | unit | passing |  |
| ENG-6b14bdbe:56 | skips resolve and stat calls for absent ancestor candidates while keeping watch paths | port |  | config::reload_tests::candidate_files_that_do_not_exist_yet_are_part_of_what_a_load_watches | unit | passing |  |
| ENG-6b14bdbe:103 | falls back to direct candidate probes when listing an ancestor directory is denied | port | | config::reload_tests::settings_in_an_ancestor_that_cannot_be_listed_are_still_found | unit | written | |
| ENG-6b14bdbe:130 | probes candidates when readDirectory reports a differently cased entry | not-applicable | reference-internal: probing strategy of the reference file reader |  | unit | - |  |
| ENG-e7da378b:11 | closes the startup race and reloads later updates | port |  | config::reload_tests::the_first_settings_file_written_after_start_up_is_loaded | unit | passing |  |
| ENG-699bf0ff.2:10 | recovers unquoted-colon frontmatter via the sanitize fallback | covered | same | systemprompt-engine-tests::config::frontmatter_tests::colon_in_plain_value_is_repaired | unit | passing |  |
| ENG-699bf0ff.2:16 | recovers a frontmatter value that starts with a bracketed tag | port |  | config::frontmatter_tests::a_value_starting_with_a_bracketed_tag_is_recovered | unit | ledger | observed: the file is refused with a front matter error |
| ENG-699bf0ff.2:22 | preserves flow mappings and flow sequences when repairing another field | port |  | config::frontmatter_tests::flow_collections_survive_the_repair_of_another_field | unit | passing |  |
| ENG-699bf0ff.2:39 | preserves nested flow collections, quoted delimiters, and inline comments during repair | port |  | config::frontmatter_tests::nested_flow_collections_quoted_delimiters_and_comments_survive_repair | unit | passing |  |
| ENG-699bf0ff.2:53 | preserves multi-line flow collections while repairing fields before and after | port |  | config::frontmatter_tests::multi_line_flow_collections_survive_repair_of_fields_around_them | unit | passing |  |
| ENG-699bf0ff.2:72 | recovers leading bracketed and braced tags with colons or apostrophes | port |  | config::frontmatter_tests::leading_braced_tags_with_colons_or_apostrophes_are_recovered | unit | ledger | observed: the file is refused with a front matter error |
| ENG-699bf0ff.2:86 | preserves flow collections with CRLF line endings during repair | port |  | config::frontmatter_tests::crlf_line_endings_do_not_stop_repair | unit | passing |  |
| ENG-699bf0ff.2:94 | keeps block scalar headers with commented colons during repair | port |  | config::frontmatter_tests::block_scalar_headers_with_a_commented_colon_are_kept | unit | passing |  |
| ENG-699bf0ff.2:105 | still throws on unclosed or mismatched flow collections | port |  | config::frontmatter_tests::unclosed_or_mismatched_flow_collections_still_fail_and_keep_failing | unit | passing |  |
| ENG-699bf0ff.2:110 | recovers the same content again after a previous failed parse | port |  | config::frontmatter_tests::the_same_repairable_content_parses_the_same_way_every_time | unit | passing |  |
| ENG-699bf0ff.2:118 | keeps throwing for the same unparseable content on every call | port |  | config::frontmatter_tests::unclosed_or_mismatched_flow_collections_still_fail_and_keep_failing | unit | passing |  |
| ENG-397ffe19:10 | normalizes short and explicit model selections | port |  | config::settings_tests::a_model_selection_is_kept_as_written_in_short_and_explicit_forms | unit | passing |  |
| ENG-397ffe19:23 | rejects malformed selections and reserved fragments | port |  | config::settings_tests::a_model_selection_that_is_not_text_fails_the_load | unit | passing |  |
| ENG-c21c79ed.1:89 | rejects every non-object root with one root diagnostic | port |  | config::settings_tests::a_non_object_root_is_refused_with_a_single_error_naming_the_file | unit | passing |  |
| ENG-c21c79ed.1:258 | recovers malformed named entries and retains a valid legacy collision | port |  | config::settings_tests::a_malformed_server_entry_is_dropped_alone_and_reported | unit | passing |  |
| ENG-c21c79ed.1:353 | redacts permission resource keys from invalid diagnostics | port |  | config::settings_tests::server_credentials_never_appear_in_debug_output | unit | passing |  |
| ENG-c21c79ed.1:367 | recovers list items for skills, instructions, and permissions | port |  |  | unit | ledger | asserts: a malformed item in an instructions list is dropped alone; observed: the whole settings file is refused (instructions must be a list of strings) |
| ENG-c21c79ed.1:382 | omits malformed collection roots instead of synthesizing empty values | port |  |  | unit | ledger | asserts: a malformed collection root is omitted and the rest of the file loads; observed: the load fails |
| ENG-c21c79ed.1:402 | omits all-invalid formatter and LSP maps while preserving explicit empty maps | port |  | config::settings_tests::formatter_and_language_server_entries_without_a_command_are_dropped_while_an_empty_map_stays_empty | unit | passing |  |
| ENG-c21c79ed.1:468 | expands numeric MCP timeouts to every phase | port |  | config::settings_tests::a_numeric_server_timeout_is_kept_for_every_phase_of_that_server | unit | passing |  |
| ENG-c21c79ed.1:517 | normalizes MCP timeout fields in schema order with per-leaf recovery | not-applicable | no-feature: per-phase MCP timeouts (the harness has one timeout per server) |  | unit | - |  |
| ENG-c21c79ed.1:531 | merges bounded compaction leaves and omits unsupported leaves | port |  | config::settings_tests::compaction_settings_merge_leaf_by_leaf_across_layers | unit | passing |  |
| ENG-c21c79ed.1:832 | preserves explicit false, zero, empty list, and empty map presence | port |  | config::settings_tests::explicit_false_zero_and_empty_values_are_preserved | unit | passing |  |
| ENG-ac00faab:60 | loads skills when ${input.root} appears after startup | port |  | config::skill_tests::skills_appearing_after_start_up_under_a_missing_root_are_picked_up | unit | passing |  |
| ENG-ac00faab:102 | retains readiness signalled synchronously during initial config startup | not-applicable | reference-internal: start-up readiness signalling of the reference watcher |  | unit | - |  |
| ENG-ac00faab:127 | loads the first config written while a new directory watch is starting | port |  | config::reload_tests::a_settings_directory_created_after_start_up_is_still_watched | unit | passing |  |
| ENG-ac00faab:175 | loads references when ${input.file} is first created and keeps watching it | port |  | config::instruction_tests::a_new_instruction_file_is_discovered_on_the_next_load | unit | passing |  |
| ENG-ac00faab:239 | preserves reference precedence and insertion order across documents | port |  | config::instruction_tests::configured_files_globs_and_home_paths_load_alongside_the_discovered_file | unit | passing |  |
| ENG-ac00faab:274 | reloads config-backed domains without reloading external plugins | not-applicable | plugin-runtime: external plugin reload |  | unit | - |  |
| ENG-ac00faab:329 | publishes catalog updates on boot and real changes but skips unchanged reloads | port |  | config::reload_tests::a_reload_that_changes_nothing_reports_nothing_changed | unit | passing |  |
| ENG-2841d499.1:128 | reads disable-model-invocation as autoinvoke | port |  | config::skill_tests::a_skill_file_keeps_its_identifier_description_and_invocation_flags | unit | passing |  |
| ENG-2841d499.1:140 | parses root and nested skill ids and metadata flags | port |  | config::skill_tests::a_skill_file_keeps_its_identifier_description_and_invocation_flags | unit | passing |  |
| ENG-2841d499.1:180 | maps config entry types to skill directories | not-applicable | reference-internal: mapping of the reference configuration entry types |  | unit | - |  |
| ENG-2841d499.1:216 | loads directory and individual downloaded skill roots with later-source precedence | port |  | config::skill_tests::later_sources_win_over_earlier_ones | unit | passing |  |
| ENG-2841d499.1:254 | prefers a worktree skill over the parent checkout copy | port |  | config::skill_tests::a_nested_checkout_copy_wins_over_the_parent_checkout | unit | passing |  |
| ENG-2841d499.1:306 | rescans directory sources when watched files change | port |  | config::skill_tests::a_skill_directory_is_rescanned_when_its_files_change | unit | passing |  |
| ENG-2841d499.1:340 | watches canonical files behind symlinked skills | port |  | config::skill_tests::a_skill_reached_through_a_symlink_reloads_when_its_target_changes | unit | passing |  |
| ENG-2841d499.1:367 | reloads symlinked sources when their target changes | port |  | config::skill_tests::a_skill_reached_through_a_symlink_reloads_when_its_target_changes | unit | passing |  |
| ENG-2841d499.1:408 | follows missing source directories as their parents appear | port |  | config::skill_tests::skills_appearing_after_start_up_under_a_missing_root_are_picked_up | unit | passing |  |
| ENG-2841d499.1:432 | rescans on a config update only when the skill sources changed | port |  | config::skill_tests::an_unrelated_settings_change_does_not_change_the_skill_set | unit | passing |  |
| ENG-2841d499.1:554 | ignores unrelated config updates and coalesces compatibility bursts into one reload | port |  | config::skill_tests::an_unrelated_settings_change_does_not_change_the_skill_set | unit | passing |  |
| ENG-f378bae6:22 | applies availability and reloads changed config | port |  |  | unit | ledger | asserts: workspace snapshots can be switched off in settings and the switch applies on change; observed: no such setting exists; the key is kept in Settings::extra and snapshots are always taken |
| ENG-a7cef8c3:18 | groups missing candidates and keeps parent watches when roots appear | port |  | config::reload_tests::candidate_files_that_do_not_exist_yet_are_part_of_what_a_load_watches | unit | passing |  |
| ENG-a7cef8c3:40 | adds exact watches for explicit files only when not already covered | port |  | config::instruction_tests::a_new_instruction_file_is_discovered_on_the_next_load | unit | passing |  |
| ENG-39e0a118:72 | boots without filesystem config discovery | not-applicable | no-feature: a host-level switch that turns settings and instruction discovery off (the harness offers compat_dirs only, covered in config::settings_tests::turning_compatibility_directories_off_leaves_only_the_harness_locations) |  | unit | - |  |
| ENG-77fb49ea:124 | stores ordered values with last-write-wins precedence | port |  |  | unit | ledger | asserts: instruction values are tracked per source and only changes are rendered to the model; observed: the full instruction text is rebuilt into the system prompt on every call |
| ENG-77fb49ea:145 | preserves admitted values while the source is unavailable | port |  |  | unit | ledger | asserts: instruction values are tracked per source and only changes are rendered to the model; observed: the full instruction text is rebuilt into the system prompt on every call |
| ENG-77fb49ea:158 | renders granular instruction updates | port |  |  | unit | ledger | asserts: instruction values are tracked per source and only changes are rendered to the model; observed: the full instruction text is rebuilt into the system prompt on every call |
| ENG-77fb49ea:204 | loads global and upward project files and rescans them on change | port |  | config::instruction_tests::global_then_upward_project_files_load_and_reload_on_change | unit | passing |  |
| ENG-77fb49ea:301 | loads configured files, globs, home paths, and URLs alongside AGENTS.md | port |  | config::instruction_tests::configured_files_globs_and_home_paths_load_alongside_the_discovered_file | unit | passing |  |
| ENG-77fb49ea:388 | reloads configured sources on config updates and keeps the last fetched URL | not-applicable | no-feature: instruction sources fetched from URLs |  | unit | - |  |
| ENG-77fb49ea:430 | falls back to CLAUDE.md where no AGENTS.md exists | port |  | config::instruction_tests::the_compatibility_name_is_used_only_where_no_primary_file_exists | unit | passing |  |
| ENG-77fb49ea:491 | keeps an empty AGENTS.md as available context | port |  | config::instruction_tests::an_empty_instruction_file_is_kept_as_present | unit | passing |  |
| ENG-77fb49ea:514 | discovers a newly created instruction file above the project root | port |  | config::instruction_tests::a_new_instruction_file_is_discovered_on_the_next_load | unit | passing |  |
| ENG-77fb49ea:558 | stops instruction candidates at the project root outside home | port |  | config::instruction_tests::discovery_stops_at_the_project_root | unit | passing |  |
| ENG-77fb49ea:646 | isolates source failure without failing activation | port |  | config::instruction_tests::an_unreadable_instruction_file_fails_alone | unit | passing |  |
| ENG-77fb49ea:676 | marks a discovered file that disappears before read as unavailable | port |  | config::instruction_tests::a_missing_configured_file_is_reported_without_failing_the_load | unit | passing |  |
| ENG-77fb49ea:711 | skips discovered files a sandbox denies and keeps the rest available | not-applicable | no-feature: sandboxed reads of instruction files |  | unit | - |  |
| ENG-77fb49ea:743 | canonicalizes boundaries and honors project opt-out | port | | | unit | ledger | asserts: project instruction discovery can be turned off and its boundaries are canonicalised; observed by reading: config/instruction.rs collect has no switch that skips the project levels |
| ENG-6a70c726.2:546 | allows the built-in Plan agent to be disabled | port |  | config::agent_tests::a_built_in_agent_disabled_in_settings_disappears | unit | passing |  |
| ENG-9ea19c3a.1:111 | Permission.site saves a web URL's site as written, and only the URL itself when its site isn't plain | not-applicable | no-feature: saving a web site, rather than a URL, as an approval |  | unit | - |  |
| ENG-9ea19c3a.1:132 | never reports an action as denied when evaluate leaves a resource reachable | port |  | config::permission_tests::a_blanket_deny_lifted_by_a_later_allow_no_longer_denies_everything | unit | passing |  |
| ENG-9ea19c3a.1:159 | reports an action as denied when a blanket deny is only followed by narrow denies | port |  | config::permission_tests::a_blanket_deny_followed_only_by_narrow_denies_still_denies_everything | unit | ledger | observed: denies_all is false: only the last rule for the permission is looked at |
| ENG-9ea19c3a.1:174 | composes override, restrict, and fallback with sound algebraic laws | not-applicable | reference-internal: algebra of the reference rule combinators |  | unit | - |  |
| ENG-9ea19c3a.1:268 | returns the evaluated effect and only queues prompts | not-applicable | plugin-runtime: evaluate hooks and unattended answer sets |  | unit | - |  |
| ENG-9ea19c3a.1:283 | denies when Unattended is empty and an evaluate hook downgrades allow to ask | not-applicable | plugin-runtime: evaluate hooks and unattended answer sets |  | unit | - |  |
| ENG-9ea19c3a.1:300 | evaluates against an explicit agent | port |  | config::permission_tests::global_rules_apply_before_agent_specific_ones | unit | passing |  |
| ENG-9ea19c3a.1:323 | allows and denies from explicit rules without asking | port |  | config::permission_tests::explicit_rules_allow_and_deny_without_asking_and_unmatched_subjects_ask | unit | passing |  |
| ENG-9ea19c3a.1:335 | checks for a deny without asking | port |  | config::permission_tests::explicit_rules_allow_and_deny_without_asking_and_unmatched_subjects_ask | unit | passing |  |
| ENG-9ea19c3a.1:346 | reports whether a request is already allowed without asking | port |  | config::permission_tests::explicit_rules_allow_and_deny_without_asking_and_unmatched_subjects_ask | unit | passing |  |
| ENG-9ea19c3a.1:359 | matches Windows rule resources against slash-normalized file access resources | port |  | config::permission_tests::rules_written_with_backslashes_match_slash_normalised_subjects | unit | passing |  |
| ENG-9ea19c3a.1:392 | matches file rules without case where file systems ignore it by default | port |  | config::permission_tests::file_rules_ignore_letter_case_only_on_case_insensitive_systems | unit | passing |  |
| ENG-9ea19c3a.1:461 | preserves global ${action} asks and denies | port |  | config::permission_tests::global_asks_and_denies_survive_a_project_that_says_nothing_about_them | unit | passing |  |
| ENG-9ea19c3a.1:491 | allows managed output reads without granting external directory access | port |  | config::permission_tests::the_engine_temporary_directory_is_reachable_without_opening_other_external_directories | unit | passing |  |
| ENG-9ea19c3a.1:506 | uses build permissions when the Session agent is omitted | port |  | config::agent_tests::the_default_agent_is_the_configured_one_when_usable_otherwise_the_first_primary | unit | passing |  |
| ENG-9ea19c3a.1:532 | denies omitted-agent permissions when no primary default agent exists | port | | | unit | ledger | asserts: a permission question for a session with no agent and no primary default agent is denied; observed by reading: the step never reaches a question, it fails with EngineError::UnknownAgent (run/session/run.rs), so the outcome is a refusal by another route |
| ENG-9ea19c3a.1:554 | evaluates bash with the normal configured-rule semantics | port |  | config::permission_tests::explicit_rules_allow_and_deny_without_asking_and_unmatched_subjects_ask | unit | passing |  |
| ENG-9ea19c3a.1:567 | merges session rules after agent rules and before saved approvals | port |  | config::permission_tests::session_rules_apply_after_agent_rules | unit | passing |  |
| ENG-9ea19c3a.1:595 | uses saved bash approvals while preserving configured deny precedence | port |  | config::permission_tests::a_saved_approval_never_overrides_a_configured_deny | unit | ledger | observed: Allow: the session approval layer is last and wins |
| ENG-9c4cca40:36 | registers built-in init and review commands | not-applicable | no-feature: built-in project-initialisation and review commands (the harness ships compact, undo and redo) |  | unit | - |  |
| ENG-85d6a31a:87 | gives the plan agent its own enter and leave texts without replacing its prompt | covered | same | systemprompt-engine-tests::prompt::notices_tests::plan_mode_names_the_plan_file_only_when_given | unit | passing |  |
| ENG-85d6a31a:103 | does not create the Plan directory during activation | port | | config::permission_tests::building_the_planning_rules_creates_no_plan_directory | unit | written | |
| ENG-85d6a31a:110 | allows edits only inside the Plan directory | port |  | config::permission_tests::in_planning_mode_only_the_plan_directory_may_be_written | unit | passing |  |
| ENG-85d6a31a:121 | allows external directories without asking | port |  |  | unit | ledger | asserts: the plan agent reaches external directories without asking; observed: plan_defaults ask for external directories (only the engine temporary directory is allowed) |
| ENG-85d6a31a:131 | rewrites blocked mutation failures with the Plan directory | port | | | unit | ledger | asserts: a blocked edit, write or patch in planning mode fails with a message naming the Plan directory; observed by reading: the permission error is returned as is, nothing rewrites mutation failures |
| ENG-85d6a31a:153 | preserves mutation failures unrelated to permissions | covered | same | migrated-tools::edit_tests::a_missing_file_is_named_and_creating_it_is_suggested | unit | | nothing rewrites tool failures, so a failure unrelated to permissions reaches the model unchanged; the named test asserts one such failure |
| CLI-a9bfd4e0.3:427 | serializes migration and updates across processes | port |  |  | unit | ledger | asserts: concurrent writers to the global settings file are serialised by a lock; observed: update_global_setting writes atomically through a temporary file but takes no lock; two writers race (last rename wins) |
| CLI-a9bfd4e0.3:466 | config reads remain interruptible while waiting for the file lock | port |  |  | unit | ledger | asserts: concurrent writers to the global settings file are serialised by a lock; observed: update_global_setting writes atomically through a temporary file but takes no lock; two writers race (last rename wins) |
| CLI-9aa7ddae.2:99 | redacts URL credentials and credential environment variables without mutating config | port |  | config::settings_tests::server_credentials_never_appear_in_debug_output | unit | passing |  |
| SRV-5d58e04b.1:11 | returns ordered config entries for the requested directory | port |  |  | unit | ledger | asserts: the configuration of a directory can be listed as its contributing layers in precedence order; observed: Loaded keeps the merged settings and the ordered list of files read (settings_files), not per-layer values; the file order is asserted in config::settings_tests::settings_load_from_global_then_ancestors_then_the_working_directory |
| GPL-ded2881b.2:84 | reads scalars, block scalars, and nested values | port |  | config::frontmatter_tests::front_matter_is_split_off_only_when_it_is_a_well_formed_leading_block | unit | passing |  |
| GPL-ded2881b.2:141 | reads hashes that are not comments | port |  | config::frontmatter_tests::quoted_block_folded_and_nested_values_read_as_written_with_hashes_as_text | unit | passing |  |
| GPL-dac877e6:1 | * on tabs.list is true | port |  | config::permission_tests::a_star_matches_any_run_of_characters_including_none_and_line_breaks | unit | passing |  |
| GPL-dac877e6:2 | * on  is true | port |  | config::permission_tests::a_star_matches_any_run_of_characters_including_none_and_line_breaks | unit | passing |  |
| GPL-dac877e6:3 | tabs.* on tabs.list is true | port |  | config::permission_tests::a_star_matches_any_run_of_characters_including_none_and_line_breaks | unit | passing |  |
| GPL-dac877e6:4 | tabs.* on tabs. is true | port |  | config::permission_tests::a_star_matches_any_run_of_characters_including_none_and_line_breaks | unit | passing |  |
| GPL-dac877e6:5 | tabs.* on files.upload is false | port |  | config::permission_tests::a_star_matches_any_run_of_characters_including_none_and_line_breaks | unit | passing |  |
| GPL-dac877e6:6 | *.env on a/b/c.env is true | port |  | config::permission_tests::a_star_matches_any_run_of_characters_including_none_and_line_breaks | unit | passing |  |
| GPL-dac877e6:7 | first* on first\nsecond is true | port |  | config::permission_tests::a_star_matches_any_run_of_characters_including_none_and_line_breaks | unit | passing |  |
| GPL-dac877e6:8 | tabs.ope? on tabs.open is true | port |  | config::permission_tests::a_question_mark_matches_exactly_one_character | unit | passing |  |
| GPL-dac877e6:9 | tabs.ope? on tabs.ope is false | port |  | config::permission_tests::a_question_mark_matches_exactly_one_character | unit | passing |  |
| GPL-dac877e6:10 | tabs.ope? on tabs.opens is false | port |  | config::permission_tests::a_question_mark_matches_exactly_one_character | unit | passing |  |
| GPL-dac877e6:11 | git push * on git push is true | port |  | config::permission_tests::a_trailing_space_star_also_matches_the_bare_command | unit | passing |  |
| GPL-dac877e6:12 | git push * on git push origin dev is true | port |  | config::permission_tests::a_trailing_space_star_also_matches_the_bare_command | unit | passing |  |
| GPL-dac877e6:13 | git push * on git pushx is false | port |  | config::permission_tests::a_trailing_space_star_also_matches_the_bare_command | unit | passing |  |
| GPL-dac877e6:14 | git push* on git push is true | port |  | config::permission_tests::a_trailing_space_star_also_matches_the_bare_command | unit | passing |  |
| GPL-dac877e6:15 | git push* on git pushx is true | port |  | config::permission_tests::a_trailing_space_star_also_matches_the_bare_command | unit | passing |  |
| GPL-dac877e6:16 | C:/Users/x/project/* on C:\\Users\\x\\project\\a.ts is true | port |  | config::permission_tests::backslashes_count_as_slashes_in_subject_and_pattern | unit | passing |  |
| GPL-dac877e6:17 | C:\\Users\\x\\* on C:/Users/x/project/a.ts is true | port |  | config::permission_tests::backslashes_count_as_slashes_in_subject_and_pattern | unit | passing |  |
| GPL-dac877e6:18 | C:/Users/x/* on C:\\Users\\y\\a.ts is false | port |  | config::permission_tests::backslashes_count_as_slashes_in_subject_and_pattern | unit | passing |  |
| GPL-dac877e6:19 | b.owser on browser is false | port |  | config::permission_tests::every_other_character_is_literal | unit | passing |  |
| GPL-dac877e6:20 | b.owser on b.owser is true | port |  | config::permission_tests::every_other_character_is_literal | unit | passing |  |
| GPL-dac877e6:21 | a+b(c)[d]{e}\|f^g$h on a+b(c)[d]{e}\|f^g$h is true | port |  | config::permission_tests::every_other_character_is_literal | unit | passing |  |
| GPL-dac877e6:22 | browser on browsers is false | port |  | config::permission_tests::every_other_character_is_literal | unit | passing |  |
| GPL-dac877e6:23 | browser on a browser is false | port |  | config::permission_tests::every_other_character_is_literal | unit | passing |  |
| GPL-dac877e6:24 | .reference/* on .reference/x is false | port |  | config::permission_tests::letter_case_counts_except_where_the_file_system_ignores_it | unit | passing |  |
| GPL-dac877e6:25 | .reference/* on .reference/x (ignoring case) is true | port |  | config::permission_tests::letter_case_counts_except_where_the_file_system_ignores_it | unit | passing |  |
| GPL-dac877e6:26 | tabs.list on Tabs.List is false | port |  | config::permission_tests::letter_case_counts_except_where_the_file_system_ignores_it | unit | passing |  |
| GPL-dac877e6:27 | tabs.list on Tabs.List (ignoring case) is true | port |  | config::permission_tests::letter_case_counts_except_where_the_file_system_ignores_it | unit | passing |  |
| NRS-5e155f32:1106 | deep_jsonc_is_invalid_not_a_crash | port |  | config::settings_tests::a_settings_file_nested_beyond_reason_is_reported_instead_of_crashing | unit | passing |  |
| NRS-a2041a6b:158 | defaults_follow_home | not-applicable | no-feature: deriving data, cache and config directories from the environment (Paths is injected by the shell; the engine reads no environment) |  | unit | - |  |
| NRS-a2041a6b:172 | xdg_roots_and_config_dir | not-applicable | no-feature: deriving data, cache and config directories from the environment (Paths is injected by the shell; the engine reads no environment) |  | unit | - |  |
| NRS-a2041a6b:200 | home_and_tmpdir_follow_node_per_platform | not-applicable | no-feature: deriving data, cache and config directories from the environment (Paths is injected by the shell; the engine reads no environment) |  | unit | - |  |
| NRS-a2041a6b:225 | empty_config_dir_is_unset | not-applicable | no-feature: deriving data, cache and config directories from the environment (Paths is injected by the shell; the engine reads no environment) |  | unit | - |  |
| NRS-a2041a6b:231 | enter_resolves_symlinks_and_reports_failures | not-applicable | no-feature: deriving data, cache and config directories from the environment (Paths is injected by the shell; the engine reads no environment) |  | unit | - |  |
| NRS-9fe1300e:214 | disable_switches_follow_typescript | not-applicable | no-feature: a settings watcher with threads and an environment switch (staleness is checked by fingerprint on demand) |  | unit | - |  |
| NRS-9fe1300e:225 | a_burst_of_writes_gives_one_callback | port |  | config::command_tests::a_burst_of_edits_is_seen_as_one_change_with_the_final_content | unit | passing |  |
| NRS-9fe1300e:242 | an_atomic_save_over_a_watched_file_calls_back | port |  | config::reload_tests::a_settings_file_that_changes_makes_the_load_stale | unit | passing |  |
| NRS-9fe1300e:255 | a_permission_change_calls_back | port |  | config::reload_tests::a_permission_change_on_a_settings_file_is_noticed_as_a_change_only_if_it_changes_content | unit | passing |  |
| NRS-9fe1300e:268 | dropping_the_watch_ends_its_threads | not-applicable | no-feature: a settings watcher with threads and an environment switch (staleness is checked by fingerprint on demand) |  | unit | - |  |
| NRS-704c3050.2:715 | watch_reloads_a_config_file_created_in_a_missing_directory | port |  | config::reload_tests::a_settings_directory_created_after_start_up_is_still_watched | unit | passing |  |
| NRS-55f17bf3.2:1756 | a_directory_made_again_is_watched_again | port |  | config::reload_tests::a_directory_removed_and_created_again_at_the_same_path_is_seen_afresh | unit | passing |  |
| OTH-6c23742c:5 | Agent.Color preserves configured colors at the public boundary | port |  | config::agent_tests::an_agent_colour_is_kept_as_given | unit | passing |  |
| OTH-5c9a8967.1:15 | round-trips tool.use and rejects unsupported action spellings | port |  | config::permission_tests::an_unknown_action_spelling_is_refused_at_load | unit | passing |  |
| OTH-5c9a8967.1:44 | normalizes legacy prefixed integration.use statements into mcp.use, skill.use and plugin.use | not-applicable | no-feature: legacy policy statement spellings (the harness never had them) |  | unit | - |  |
| OTH-32b19ac5:9 | uses XDG overrides | not-applicable | no-feature: deriving data, cache and config directories from the environment (Paths is injected by the shell) |  | unit | - |  |
| OTH-32b19ac5:27 | empty XDG overrides use home directory defaults | not-applicable | no-feature: deriving data, cache and config directories from the environment (Paths is injected by the shell) |  | unit | - |  |
| OTH-2b6a9e9d:10 | importing the module does not create directories | not-applicable | no-feature: lazy creation of application directories (Paths is injected by the shell) |  | unit | - |  |
| OTH-2b6a9e9d:34 | building layerWith creates service directories and preserves an explicit tmp | not-applicable | no-feature: lazy creation of application directories (Paths is injected by the shell) |  | unit | - |  |
| OTH-2b6a9e9d:53 | building a layer with default tmp creates and canonicalizes it | not-applicable | no-feature: lazy creation of application directories (Paths is injected by the shell) |  | unit | - |  |
