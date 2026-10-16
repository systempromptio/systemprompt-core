//! The `-- @compat:` directive: only `breaking` is a value, it counts only in
//! the leading comment block, and anything else is refused so a typo never
//! turns a contract migration into an additive one.

use systemprompt_extension::Migration;
use systemprompt_extension::compat::{self, CompatDirectiveError};

#[test]
fn absent_directive_is_additive() {
    assert_eq!(compat::parse("ALTER TABLE t ADD COLUMN c INT;"), Ok(false));
}

#[test]
fn breaking_in_the_leading_block_is_recognised() {
    let sql = "-- drops the legacy column\n-- @compat: breaking\nALTER TABLE t DROP COLUMN c;";
    assert_eq!(compat::parse(sql), Ok(true));
    assert!(Migration::new(9, "drop_c", sql).breaks_older());
}

#[test]
fn a_directive_after_sql_is_a_comment_not_a_declaration() {
    let sql = "ALTER TABLE t DROP COLUMN c;\n-- @compat: breaking";
    assert_eq!(compat::parse(sql), Ok(false));
}

#[test]
fn any_other_value_is_refused() {
    assert_eq!(
        compat::parse("-- @compat: breakng\nSELECT 1;"),
        Err(CompatDirectiveError::Value("breakng".to_owned()))
    );
}

#[test]
fn a_repeated_directive_is_refused() {
    assert_eq!(
        compat::parse("-- @compat: breaking\n-- @compat: breaking\nSELECT 1;"),
        Err(CompatDirectiveError::Repeated(2))
    );
}
