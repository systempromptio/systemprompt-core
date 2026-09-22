//! The `-- @cost:` directive parser: what a migration is allowed to claim
//! about its own bulk work, and what it may not.
//!
//! The value is load-bearing — the migration runner turns `measured` into
//! that migration's `statement_timeout` — so a directive that parses loosely
//! would hand a backfill a bound nobody chose. Every rejection below is a
//! shape that would otherwise do exactly that.

use std::time::Duration;

use systemprompt_extension::cost::{self, TriggerPolicy};

#[test]
fn a_full_directive_parses() {
    let declared = cost::parse("-- @cost: rows=3644 measured=2.0s triggers=suspended\nUPDATE t;")
        .expect("parses")
        .expect("present");
    assert_eq!(declared.rows, 3644);
    assert_eq!(declared.measured, Duration::from_millis(2000));
    assert_eq!(declared.triggers, TriggerPolicy::Suspended);
}

#[test]
fn every_unit_is_understood() {
    for (value, expected) in [
        ("250ms", Duration::from_millis(250)),
        ("2.5s", Duration::from_millis(2500)),
        ("3m", Duration::from_secs(180)),
    ] {
        let sql = format!("-- @cost: rows=1 measured={value} triggers=live\n");
        let declared = cost::parse(&sql).expect("parses").expect("present");
        assert_eq!(declared.measured, expected, "{value}");
    }
}

// Why: a bare number is the shape most likely to be written by hand, and it
// is ambiguous between seconds and milliseconds — a 1000x error in the bound.
#[test]
fn a_duration_without_a_unit_is_refused() {
    for value in ["2", "", "s", "-1s", "abcs", "2sec"] {
        let sql = format!("-- @cost: rows=1 measured={value} triggers=live\n");
        assert!(cost::parse(&sql).is_err(), "{value:?} must be refused");
    }
}

#[test]
fn every_field_is_required() {
    for sql in [
        "-- @cost: measured=1s triggers=live\n",
        "-- @cost: rows=1 triggers=live\n",
        "-- @cost: rows=1 measured=1s\n",
    ] {
        assert!(cost::parse(sql).is_err(), "{sql:?} must be refused");
    }
}

#[test]
fn malformed_shapes_are_refused() {
    for sql in [
        "-- @cost: rows=1 measured=1s triggers=maybe\n",
        "-- @cost: rows=lots measured=1s triggers=live\n",
        "-- @cost: rows=1 measured=1s triggers=live extra=1\n",
        "-- @cost: rows\n",
        "-- @cost: rows=1 rows=2 measured=1s triggers=live\n",
    ] {
        assert!(cost::parse(sql).is_err(), "{sql:?} must be refused");
    }
}

// Why: two directives mean two different claims about the same migration and
// there is no rule for picking one; the author has to say which is true.
#[test]
fn a_repeated_directive_is_refused() {
    let sql = "-- @cost: rows=1 measured=1s triggers=live\n\
               -- @cost: rows=2 measured=2s triggers=live\n";
    assert!(cost::parse(sql).is_err());
}

// Why: the directive must be a header, not something buried mid-file where a
// reader would never look for it — the same rule the other directives follow.
#[test]
fn a_directive_after_the_sql_is_not_a_declaration() {
    let sql = "UPDATE t SET x = 1;\n-- @cost: rows=1 measured=1s triggers=live\n";
    assert_eq!(cost::parse(sql).expect("parses"), None);
}

#[test]
fn an_ordinary_migration_declares_nothing() {
    assert_eq!(
        cost::parse("-- adds a column\nALTER TABLE t ADD COLUMN c TEXT;")
            .expect("parses"),
        None
    );
    assert_eq!(cost::parse("").expect("parses"), None);
}
