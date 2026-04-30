use systemprompt_cowork::integration::claude_desktop::win_reg_parser::parse_reg_line;

#[test]
fn reg_sz_with_comma_separated_value_keeps_full_value() {
    let line =
        "    inferenceModels    REG_SZ    claude-opus-4-7,claude-sonnet-4-6,claude-haiku-4-5";
    let (name, value) = parse_reg_line(line.trim_start()).expect("should parse");
    assert_eq!(name, "inferenceModels");
    assert_eq!(value, "claude-opus-4-7,claude-sonnet-4-6,claude-haiku-4-5");
}

#[test]
fn reg_sz_with_embedded_spaces_in_value_is_preserved() {
    let line = "    inferenceGatewayBaseUrl    REG_SZ    http://localhost:8080/v1";
    let (name, value) = parse_reg_line(line.trim_start()).unwrap();
    assert_eq!(name, "inferenceGatewayBaseUrl");
    assert_eq!(value, "http://localhost:8080/v1");
}

#[test]
fn reg_dword_hex_decodes_to_decimal() {
    let line = "    EnableTelemetry    REG_DWORD    0x0000002a";
    let (name, value) = parse_reg_line(line.trim_start()).unwrap();
    assert_eq!(name, "EnableTelemetry");
    assert_eq!(value, "42");
}

#[test]
fn reg_dword_non_hex_value_passes_through_unchanged() {
    let line = "    Weird    REG_DWORD    notahex";
    let (name, value) = parse_reg_line(line.trim_start()).unwrap();
    assert_eq!(name, "Weird");
    assert_eq!(value, "notahex");
}

#[test]
fn unknown_kind_keeps_remainder_as_value() {
    let line = "Foo REG_BINARY 01 02 03";
    let (name, value) = parse_reg_line(line).unwrap();
    assert_eq!(name, "Foo");
    assert_eq!(value, "01 02 03");
}

#[test]
fn returns_none_when_only_name_present() {
    assert!(parse_reg_line("OnlyOneToken").is_none());
}
