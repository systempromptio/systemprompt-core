use serde_json::{Value, json};
use systemprompt_device::protocol::jcs::{canonicalize, canonicalize_str, is_canonical};

fn number(value: f64) -> String {
    canonicalize(&Value::from(value))
}

#[test]
fn rfc_8785_section_3_2_2_whitespace_escapes_and_numbers() {
    let input = r#"{
      "numbers": [333333333.33333329, 1E30, 4.50, 2e-3, 0.000000000000000000000000001],
      "string": "\u20ac$\u000F\u000aA'\u0042\u0022\u005c\\\"\/",
      "literals": [null, true, false]
    }"#;
    let expected = r#"{"literals":[null,true,false],"numbers":[333333333.3333333,1e+30,4.5,0.002,1e-27],"string":"€$\u000f\nA'B\"\\\\\"/"}"#;
    assert_eq!(canonicalize_str(input).unwrap(), expected);
}

#[test]
fn rfc_8785_section_3_2_3_keys_sort_by_utf16_code_units() {
    let input = r#"{
      "\u20ac": "Euro Sign",
      "\r": "Carriage Return",
      "\ufb33": "Hebrew Letter Dalet With Dagesh",
      "1": "One",
      "\ud83d\ude00": "Emoji: Grinning Face",
      "\u0080": "Control",
      "\u00f6": "Latin Small Letter O With Diaeresis"
    }"#;
    let expected = "{\"\\r\":\"Carriage Return\",\"1\":\"One\",\"\u{80}\":\"Control\",\"\u{f6}\":\"Latin Small Letter O With Diaeresis\",\"\u{20ac}\":\"Euro Sign\",\"\u{1f600}\":\"Emoji: Grinning Face\",\"\u{fb33}\":\"Hebrew Letter Dalet With Dagesh\"}";
    assert_eq!(canonicalize_str(input).unwrap(), expected);
}

#[test]
fn rfc_8785_appendix_b_numbers() {
    let cases: [(f64, &str); 11] = [
        (0.0, "0"),
        (-0.0, "0"),
        (5e-324, "5e-324"),
        (1.797_693_134_862_315_7e308, "1.7976931348623157e+308"),
        (9_007_199_254_740_992.0, "9007199254740992"),
        (295_147_905_179_352_830_000.0, "295147905179352830000"),
        (1e21, "1e+21"),
        (1e20, "100000000000000000000"),
        (0.000_001, "0.000001"),
        (1e-7, "1e-7"),
        (-1.5e-8, "-1.5e-8"),
    ];
    for (value, expected) in cases {
        assert_eq!(number(value), expected, "{value:e}");
    }
}

#[test]
fn integers_print_as_integers() {
    assert_eq!(
        canonicalize(&json!([1, -42, 1_000_000, 0])),
        "[1,-42,1000000,0]"
    );
    assert_eq!(
        canonicalize_str("295147905179352830000").unwrap(),
        "295147905179352830000"
    );
}

#[test]
fn nested_objects_sort_at_every_level() {
    let value = json!({ "b": [{ "z": 1, "y": { "d": 1, "c": 2 } }], "a": "x" });
    assert_eq!(
        canonicalize(&value),
        r#"{"a":"x","b":[{"y":{"c":2,"d":1},"z":1}]}"#
    );
}

#[test]
fn control_characters_escape_lowercase() {
    assert_eq!(
        canonicalize(&json!("\u{1}\u{1f}\u{7f}\u{2028}")),
        "\"\\u0001\\u001f\u{7f}\u{2028}\""
    );
    assert_eq!(canonicalize(&json!("\u{8}\u{c}\t")), r#""\b\f\t""#);
}

#[test]
fn canonical_text_is_recognized() {
    assert!(is_canonical(r#"{"a":1,"b":[true,null]}"#));
    assert!(!is_canonical(r#"{"b":1,"a":2}"#));
    assert!(!is_canonical(r#"{"a": 1}"#));
}
