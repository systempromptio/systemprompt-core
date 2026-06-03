use systemprompt_bridge::mcp_registry::normalize_key;

const FALLBACK: &str = "mcp-server";

#[test]
fn simple_name_is_lowercased() {
    assert_eq!(normalize_key("MyServer"), "myserver");
}

#[test]
fn already_lowercase_alphanumeric_is_unchanged() {
    assert_eq!(normalize_key("server123"), "server123");
}

#[test]
fn uppercase_alphanumeric_is_lowercased() {
    assert_eq!(normalize_key("ABC123XYZ"), "abc123xyz");
}

#[test]
fn underscores_are_preserved() {
    assert_eq!(normalize_key("foo_bar"), "foo_bar");
}

#[test]
fn pure_underscores_are_safe_and_kept() {
    // `_` is in the safe set, so it is neither collapsed nor stripped.
    assert_eq!(normalize_key("___"), "___");
}

#[test]
fn underscore_mixed_with_alphanumerics() {
    assert_eq!(normalize_key("My_Server_01"), "my_server_01");
}

#[test]
fn spaces_and_specials_collapse_to_single_dash() {
    assert_eq!(normalize_key("Foo Bar!!Baz"), "foo-bar-baz");
}

#[test]
fn single_space_becomes_single_dash() {
    assert_eq!(normalize_key("foo bar"), "foo-bar");
}

#[test]
fn consecutive_specials_collapse_to_one_dash() {
    assert_eq!(normalize_key("a!!!b"), "a-b");
}

#[test]
fn run_of_mixed_separators_collapses_to_one_dash() {
    assert_eq!(normalize_key("a @#$ b"), "a-b");
}

#[test]
fn leading_spaces_do_not_yield_leading_dash() {
    assert_eq!(normalize_key("  hello"), "hello");
}

#[test]
fn leading_specials_do_not_yield_leading_dash() {
    // Leading `!!!` are skipped (prev_dash starts true); trailing `!!!`
    // emits one dash then is stripped.
    assert_eq!(normalize_key("!!!x!!!"), "x");
}

#[test]
fn leading_specials_only_then_alnum() {
    assert_eq!(normalize_key("###name"), "name");
}

#[test]
fn trailing_double_dash_separators_are_stripped() {
    assert_eq!(normalize_key("name--"), "name");
}

#[test]
fn trailing_specials_are_stripped() {
    assert_eq!(normalize_key("name!!"), "name");
}

#[test]
fn trailing_spaces_are_stripped() {
    assert_eq!(normalize_key("name   "), "name");
}

#[test]
fn all_separators_yield_fallback() {
    // Every char collapses to dashes which are then all stripped -> empty.
    assert_eq!(normalize_key("!!!"), FALLBACK);
}

#[test]
fn whitespace_only_yields_fallback() {
    assert_eq!(normalize_key("   "), FALLBACK);
}

#[test]
fn empty_string_yields_fallback() {
    assert_eq!(normalize_key(""), FALLBACK);
}

#[test]
fn non_ascii_alnum_collapses_to_dash() {
    // `é` is not ASCII-alphanumeric, so it is treated as a separator.
    assert_eq!(normalize_key("héllo"), "h-llo");
}

#[test]
fn unicode_only_yields_fallback() {
    // None of these are ASCII-alphanumeric or `_`, so all collapse and strip.
    assert_eq!(normalize_key("日本語"), FALLBACK);
}

#[test]
fn interior_dash_run_via_specials_collapses() {
    assert_eq!(normalize_key("a---b"), "a-b");
}

#[test]
fn dot_and_slash_treated_as_separators() {
    assert_eq!(normalize_key("foo.bar/baz"), "foo-bar-baz");
}

#[test]
fn mixed_leading_trailing_and_interior() {
    assert_eq!(normalize_key("  Foo  Bar  "), "foo-bar");
}

#[test]
fn is_deterministic_for_repeated_calls() {
    let input = "Foo Bar!!Baz héllo  ";
    let first = normalize_key(input);
    let second = normalize_key(input);
    assert_eq!(first, second);
}

#[test]
fn fallback_inputs_are_deterministic() {
    assert_eq!(normalize_key("!!!"), normalize_key("@@@"));
    assert_eq!(normalize_key(""), normalize_key("   "));
}
