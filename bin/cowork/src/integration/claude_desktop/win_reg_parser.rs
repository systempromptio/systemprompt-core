#[must_use]
pub fn parse_reg_line(line: &str) -> Option<(String, String)> {
    let mut iter = line.split_whitespace();
    let name = iter.next()?.to_string();
    let kind = iter.next()?;
    let value: String = iter.collect::<Vec<_>>().join(" ");
    let value = if kind == "REG_DWORD" {
        value
            .strip_prefix("0x")
            .and_then(|hex| u64::from_str_radix(hex, 16).ok())
            .map(|n| n.to_string())
            .unwrap_or(value)
    } else {
        value
    };
    Some((name, value))
}
