//! `ToDbValue` conversion matrix.
//!
//! Repository query builders bind identifiers both by value and by reference;
//! every `&T` impl must agree with its owned counterpart and every type must
//! map to the correct SQL null variant.

use chrono::{TimeZone, Utc};
use systemprompt_identifiers::{
    DbValue, Email, JwtToken, LocaleCode, ProfileName, ToDbValue, UserId, ValidatedFilePath,
    ValidatedUrl,
};

fn as_string(value: &DbValue) -> &str {
    match value {
        DbValue::String(s) => s,
        other => panic!("expected DbValue::String, got {other:?}"),
    }
}

#[test]
fn reference_impls_agree_with_their_owned_counterparts() {
    let s = "hello".to_owned();
    assert_eq!((&s).to_db_value(), s.to_db_value());

    assert_eq!((&7i32).to_db_value(), 7i32.to_db_value());
    assert_eq!((&7i64).to_db_value(), 7i64.to_db_value());
    assert_eq!((&1.5f64).to_db_value(), 1.5f64.to_db_value());
    assert_eq!((&true).to_db_value(), true.to_db_value());

    let dt = Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap();
    assert_eq!((&dt).to_db_value(), dt.to_db_value());

    let arr = vec!["a".to_owned(), "b".to_owned()];
    assert_eq!((&arr).to_db_value(), arr.to_db_value());
    assert_eq!(arr.as_slice().to_db_value(), arr.to_db_value());
}

#[test]
fn null_db_value_maps_each_type_to_its_sql_null_variant() {
    assert_eq!(<&String>::null_db_value(), DbValue::NullString);
    assert_eq!(<&i32>::null_db_value(), DbValue::NullInt);
    assert_eq!(<&i64>::null_db_value(), DbValue::NullInt);
    assert_eq!(<&f64>::null_db_value(), DbValue::NullFloat);
    assert_eq!(<&bool>::null_db_value(), DbValue::NullBool);
    assert_eq!(
        <&chrono::DateTime<Utc>>::null_db_value(),
        DbValue::NullTimestamp
    );
    assert_eq!(<&Vec<String>>::null_db_value(), DbValue::NullStringArray);
    assert_eq!(f32::null_db_value(), DbValue::NullFloat);
    assert_eq!(u32::null_db_value(), DbValue::NullInt);
    assert_eq!(u64::null_db_value(), DbValue::NullInt);
}

#[test]
fn unsigned_and_f32_widen_into_their_sql_column_types() {
    assert_eq!(9u32.to_db_value(), DbValue::Int(9));
    assert_eq!(9u64.to_db_value(), DbValue::Int(9));
    assert_eq!(
        u64::MAX.to_db_value(),
        DbValue::Int(i64::MAX),
        "u64 overflow saturates rather than wrapping"
    );
    assert_eq!(2.5f32.to_db_value(), DbValue::Float(2.5));
}

#[test]
fn validated_wrapper_types_bind_as_their_inner_string_by_ref_and_value() {
    let email = Email::try_new("a@b.com").unwrap();
    assert_eq!(as_string(&email.to_db_value()), "a@b.com");
    assert_eq!((&email).to_db_value(), email.to_db_value());

    let locale = LocaleCode::try_new("en-US").unwrap();
    assert_eq!((&locale).to_db_value(), locale.to_db_value());

    let path = ValidatedFilePath::try_new("a/b.txt").unwrap();
    assert_eq!(as_string(&path.to_db_value()), "a/b.txt");
    assert_eq!((&path).to_db_value(), path.to_db_value());

    let profile = ProfileName::try_new("local").unwrap();
    assert_eq!(as_string(&profile.to_db_value()), "local");
    assert_eq!((&profile).to_db_value(), profile.to_db_value());

    let url = ValidatedUrl::try_new("https://example.com").unwrap();
    assert_eq!((&url).to_db_value(), url.to_db_value());

    let id = UserId::new("user-1");
    assert_eq!(as_string(&id.to_db_value()), "user-1");
    assert_eq!((&id).to_db_value(), id.to_db_value());

    let token = JwtToken::new("tok");
    assert_eq!(as_string(&token.to_db_value()), "tok");
    assert_eq!((&token).to_db_value(), token.to_db_value());
}
