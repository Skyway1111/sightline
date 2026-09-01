//! CPython `repr` and `format` spellings (R8, R18).

use super::chars::is_printable;

/// CPython `repr(str)`: `'` quotes unless the string holds one and no `"`.
pub fn repr_str(s: &str) -> String {
    let quote = if s.contains('\'') && !s.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut out = String::with_capacity(s.len() + 2);
    out.push(quote);
    for c in s.chars() {
        match c {
            _ if c == quote || c == '\\' => {
                out.push('\\');
                out.push(c);
            }
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            _ if is_printable(c) => out.push(c),
            _ => out.push_str(&escape_hex(c as u32)),
        }
    }
    out.push(quote);
    out
}

fn escape_hex(code: u32) -> String {
    match code {
        0..=0xff => format!("\\x{code:02x}"),
        0x100..=0xffff => format!("\\u{code:04x}"),
        _ => format!("\\U{code:08x}"),
    }
}

/// CPython `repr(bytes)`: printable ASCII stays, everything else is `\xNN`.
pub fn repr_bytes(b: &[u8]) -> String {
    let quote = if b.contains(&b'\'') && !b.contains(&b'"') {
        b'"'
    } else {
        b'\''
    };
    let mut out = String::with_capacity(b.len() + 3);
    out.push('b');
    out.push(quote as char);
    for &byte in b {
        match byte {
            _ if byte == quote || byte == b'\\' => {
                out.push('\\');
                out.push(byte as char);
            }
            b'\t' => out.push_str("\\t"),
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            0x20..=0x7e => out.push(byte as char),
            _ => out.push_str(&format!("\\x{byte:02x}")),
        }
    }
    out.push(quote as char);
    out
}

/// CPython `repr(int)`.
pub fn repr_int(n: i64) -> String {
    n.to_string()
}

/// CPython `repr(list[str])`.
pub fn repr_str_list<S: AsRef<str>>(items: &[S]) -> String {
    let inner: Vec<String> = items.iter().map(|s| repr_str(s.as_ref())).collect();
    format!("[{}]", inner.join(", "))
}

/// The shortest round-trip digits of `x` and the position of its decimal
/// point: `value = 0.<digits> * 10^decpt`. Rust's `{:e}` is the same shortest
/// conversion CPython's `dtoa` mode 0 makes.
fn shortest(x: f64) -> (String, i32) {
    split_exp(&format!("{:e}", x.abs()))
}

fn split_exp(text: &str) -> (String, i32) {
    let (mantissa, exp) = text
        .split_once('e')
        .expect("`{:e}` always writes an exponent");
    let exp: i32 = exp
        .parse()
        .expect("`{:e}` always writes an integer exponent");
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    (digits, exp + 1)
}

fn fixed(digits: &str, decpt: i32, add_dot_0: bool) -> String {
    let width = digits.len() as i32;
    if decpt <= 0 {
        format!("0.{}{}", "0".repeat((-decpt) as usize), digits)
    } else if decpt < width {
        let at = decpt as usize;
        format!("{}.{}", &digits[..at], &digits[at..])
    } else {
        let whole = format!("{}{}", digits, "0".repeat((decpt - width) as usize));
        if add_dot_0 {
            format!("{whole}.0")
        } else {
            whole
        }
    }
}

fn scientific(digits: &str, decpt: i32) -> String {
    let exp = decpt - 1;
    let tail = &digits[1..];
    let frac = if tail.is_empty() {
        String::new()
    } else {
        format!(".{tail}")
    };
    let sign = if exp < 0 { '-' } else { '+' };
    format!("{}{}e{}{:02}", &digits[..1], frac, sign, exp.abs())
}

/// CPython `repr(float)`: shortest round trip, `1e+16` at the upper boundary
/// and `1e-05` at the lower, `.0` on an integral value (R18).
pub fn repr_float(x: f64) -> String {
    if x.is_nan() {
        return "nan".to_string();
    }
    let sign = if x.is_sign_negative() { "-" } else { "" };
    if x.is_infinite() {
        return format!("{sign}inf");
    }
    let (digits, decpt) = shortest(x);
    let body = if decpt <= -4 || decpt > 16 {
        scientific(&digits, decpt)
    } else {
        fixed(&digits, decpt, true)
    };
    format!("{sign}{body}")
}

/// Python `f"{x:g}"`: six significant digits, trailing zeros dropped, and no
/// `.0` on an integral value (R18).
pub fn format_g(x: f64) -> String {
    if x.is_nan() {
        return "nan".to_string();
    }
    let sign = if x.is_sign_negative() { "-" } else { "" };
    if x.is_infinite() {
        return format!("{sign}inf");
    }
    let (rounded, decpt) = split_exp(&format!("{:.5e}", x.abs()));
    let stripped = rounded.trim_end_matches('0');
    let digits = if stripped.is_empty() { "0" } else { stripped };
    // CPython's `format_float_short` for 'g' at precision 6
    let exp = decpt - 1;
    let body = if !(-4..6).contains(&exp) {
        scientific(digits, decpt)
    } else {
        fixed(digits, decpt, false)
    };
    format!("{sign}{body}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floats_print_at_the_exponent_boundaries() {
        assert_eq!(repr_float(1e15), "1000000000000000.0");
        assert_eq!(repr_float(1e16), "1e+16");
        assert_eq!(repr_float(1e-4), "0.0001");
        assert_eq!(repr_float(1e-5), "1e-05");
        assert_eq!(repr_float(-0.0), "-0.0");
        assert_eq!(format_g(1.0 / 3.0), "0.333333");
        assert_eq!(format_g(1e16), "1e+16");
    }
}
