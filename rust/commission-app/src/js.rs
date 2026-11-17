//! Helpers that copy JavaScript behaviour exactly.
//!
//! The old app was JavaScript. To keep every result identical (dates, amounts,
//! rounding, text), these functions copy the JavaScript rules one-to-one.

/// JavaScript `\s` (WhiteSpace + LineTerminator). Not the same as
/// `char::is_whitespace` (that one has U+0085 and does not have U+FEFF).
pub fn is_js_space(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{0B}' | '\u{0C}' | '\r' | ' ' | '\u{A0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200A}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202F}'
                | '\u{205F}'
                | '\u{3000}'
                | '\u{FEFF}'
    )
}

/// The `\s` class for the `regex` crate, with JavaScript members.
pub const JS_S: &str = r"[\t\n\x0B\x0C\r \x{A0}\x{1680}\x{2000}-\x{200A}\x{2028}\x{2029}\x{202F}\x{205F}\x{3000}\x{FEFF}]";

/// `String.prototype.trim`
pub fn trim(s: &str) -> &str {
    s.trim_matches(is_js_space)
}

/// Length in UTF-16 code units (`string.length`).
pub fn len16(s: &str) -> usize {
    s.encode_utf16().count()
}

/// `Math.round` (ties go to +∞).
pub fn math_round(x: f64) -> f64 {
    if !x.is_finite() {
        return x;
    }
    let f = x.floor();
    if x - f >= 0.5 { f + 1.0 } else { f }
}

/// `Math.round(v * 100)` as an integer: the cents of a euro value.
/// This is what the old `CommissionCore.cents()` did.
pub fn cents(v: f64) -> i64 {
    let v = if v.is_finite() { v } else { 0.0 };
    math_round(v * 100.0) as i64
}

/// `Math.round(r * 10000)` as an integer (rates and FX rates).
pub fn ten_thousandths(v: f64) -> i64 {
    let v = if v.is_finite() { v } else { 0.0 };
    math_round(v * 10_000.0) as i64
}

/// `+(+v).toFixed(2)` — the old `n2()`. Exact, ties go up (like JavaScript).
pub fn n2(v: f64) -> f64 {
    if !v.is_finite() || v.abs() >= 1e21 {
        return v;
    }
    to_fixed_units(v, 100) as f64 / 100.0
}

/// `+(+v).toFixed(4)`
pub fn n4(v: f64) -> f64 {
    if !v.is_finite() || v.abs() >= 1e21 {
        return v;
    }
    to_fixed_units(v, 10_000) as f64 / 10_000.0
}

/// The integer `n` that `toFixed` picks: `n / scale` nearest to `v`,
/// ties to the larger `n` (for negative `v`: mirror of the positive result).
fn to_fixed_units(v: f64, scale: u128) -> i128 {
    let neg = v < 0.0;
    let a = v.abs();
    if a == 0.0 {
        return 0;
    }
    let bits = a.to_bits();
    let exp = ((bits >> 52) & 0x7ff) as i32;
    let frac = bits & ((1u64 << 52) - 1);
    let (mant, e) = if exp == 0 {
        (frac as u128, -1074)
    } else {
        ((frac | (1u64 << 52)) as u128, exp - 1075)
    };
    // a = mant * 2^e
    let n: i128 = if e >= 0 {
        (mant << e) as i128 * scale as i128
    } else {
        let k = (-e) as u32;
        if k >= 120 {
            0
        } else {
            let num = mant * scale;
            let q = num >> k;
            let rem = num & ((1u128 << k) - 1);
            let half = 1u128 << (k - 1);
            (if rem >= half { q + 1 } else { q }) as i128
        }
    };
    if neg { -n } else { n }
}

/// `v.toFixed(2)` as text.
pub fn to_fixed2(v: f64) -> String {
    let n = to_fixed_units(v, 100);
    let sign = if n < 0 { "-" } else { "" };
    let a = n.unsigned_abs();
    format!("{sign}{}.{:02}", a / 100, a % 100)
}

/// `String(number)` for the values this app shows (shortest round-trip form).
pub fn num_to_string(v: f64) -> String {
    if v.is_nan() {
        return "NaN".into();
    }
    if v.is_infinite() {
        return if v > 0.0 {
            "Infinity".into()
        } else {
            "-Infinity".into()
        };
    }
    if v == 0.0 {
        return "0".into();
    }
    if v.abs() >= 1e21 || v.abs() < 1e-6 {
        // JavaScript uses exponent form here: 1e+21, 1.5e-7
        let s = format!("{v:e}");
        return match s.split_once('e') {
            Some((m, e)) if !e.starts_with('-') => format!("{m}e+{e}"),
            _ => s,
        };
    }
    format!("{v}")
}

/// Shortest decimal digits of `|v|` (like JavaScript `String(v)`): (digits, point position).
/// `1234.5` → ("12345", 4); `0.05` → ("5", -1).
fn shortest_digits(v: f64) -> (Vec<u8>, i32) {
    let s = format!("{:e}", v.abs());
    let (mant, exp) = s.split_once('e').unwrap_or((&s, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let digits: Vec<u8> = mant
        .bytes()
        .filter(u8::is_ascii_digit)
        .map(|b| b - b'0')
        .collect();
    (digits, exp + 1)
}

/// `n.toLocaleString('de-DE', {minimumFractionDigits: 2, maximumFractionDigits: 2})`.
/// Intl rounds the SHORTEST decimal form half away from zero (1.005 → "1,01",
/// while `toFixed(2)` gives "1.00"). Negative numbers and -0 keep the sign ("-0,00").
pub fn to_locale_de2(v: f64) -> String {
    if v.is_nan() {
        return "NaN".into();
    }
    let sign = if v.is_sign_negative() { "-" } else { "" };
    if v.is_infinite() {
        return format!("{sign}∞");
    }
    // cents as a decimal digit string, rounded half away from zero
    let (digits, point) = shortest_digits(v);
    let digit = |i: i32| -> u8 {
        if i >= 0 && (i as usize) < digits.len() {
            digits[i as usize]
        } else {
            0
        }
    };
    let keep = point + 2; // number of digits up to and including the 2nd decimal
    let mut cents: Vec<u8> = (0..keep.max(0)).map(digit).collect();
    if digit(keep) >= 5 {
        let mut i = cents.len();
        loop {
            if i == 0 {
                cents.insert(0, 1);
                break;
            }
            i -= 1;
            if cents[i] == 9 {
                cents[i] = 0;
            } else {
                cents[i] += 1;
                break;
            }
        }
    }
    while cents.len() < 3 {
        cents.insert(0, 0);
    }
    let split = cents.len() - 2;
    let mut int: Vec<u8> = cents[..split].to_vec();
    while int.len() > 1 && int[0] == 0 {
        int.remove(0);
    }
    let int_text: String = int.iter().map(|d| (b'0' + d) as char).collect();
    let mut grouped = String::new();
    for (i, ch) in int_text.chars().enumerate() {
        if i > 0 && (int_text.len() - i).is_multiple_of(3) {
            grouped.push('.');
        }
        grouped.push(ch);
    }
    format!("{sign}{grouped},{}{}", cents[split], cents[split + 1])
}

/// `'€' + n.toLocaleString('de-DE', …)` — the old `formatEUR()`.
pub fn format_eur(v: f64) -> String {
    format!("€{}", to_locale_de2(v))
}

/// `parseFloat(s)`: the longest number at the start; `None` for NaN.
pub fn parse_float(s: &str) -> Option<f64> {
    let s = s.trim_start_matches(is_js_space);
    let b = s.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    if s[i..].starts_with("Infinity") {
        return Some(if b.first() == Some(&b'-') {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        });
    }
    let int_start = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    let mut digits = i - int_start;
    if i < b.len() && b[i] == b'.' {
        let dot = i;
        i += 1;
        let f = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        digits += i - f;
        if digits == 0 {
            i = dot;
        }
    }
    if digits == 0 {
        return None;
    }
    // exponent, only if complete
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        let mut j = i + 1;
        if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
            j += 1;
        }
        let e = j;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        if j > e {
            i = j;
        }
    }
    let text = &s[..i];
    let text = text.strip_suffix('.').unwrap_or(text);
    text.parse::<f64>().ok()
}

// ─── Dates ───────────────────────────────────────────────────────────

/// Days since 1970-01-01 for a proleptic Gregorian date (month 1..=12, any day).
pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// (year, month, day) for days since 1970-01-01.
pub fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// `YYYY-MM-DD` text for days since 1970-01-01 (like `toISOString().slice(0, 10)`).
pub fn iso_from_days(days: i64) -> String {
    let (y, m, d) = civil_from_days(days);
    if (0..=9999).contains(&y) {
        format!("{y:04}-{m:02}-{d:02}")
    } else {
        let sign = if y < 0 { '-' } else { '+' };
        format!("{sign}{:06}-{m:02}-{d:02}", y.abs())
    }
}

/// Reads strict `YYYY-MM-DD` the way `new Date(iso + 'T00:00:00')` accepts it:
/// month 01..12, day 01..31 (a day past the month end rolls over, e.g. 02-31 → 03-03).
/// Returns days since 1970-01-01.
pub fn days_from_iso(iso: &str) -> Option<i64> {
    let b = iso.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let num = |r: std::ops::Range<usize>| -> Option<i64> {
        let s = &iso[r];
        s.bytes()
            .all(|c| c.is_ascii_digit())
            .then(|| s.parse().ok())?
    };
    let (y, m, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    Some(days_from_civil(y, m, 1) + d - 1)
}

const DAY_NAMES: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];

/// The old `getDayName(iso)`.
pub fn day_name(iso: &str) -> &'static str {
    match days_from_iso(iso) {
        // 1970-01-01 was a Thursday (4)
        Some(days) => DAY_NAMES[(days + 4).rem_euclid(7) as usize],
        None => "",
    }
}

/// The old `formatDate(iso)`: `YYYY-MM-DD` → `DD.MM.YYYY`.
pub fn format_date(iso: &str) -> String {
    if iso.is_empty() {
        return String::new();
    }
    let mut parts = iso.split('-');
    let y = parts.next().unwrap_or("undefined");
    let m = parts.next().unwrap_or("undefined");
    let d = parts.next().unwrap_or("undefined");
    format!("{d}.{m}.{y}")
}

/// The old `excelSerialToIso(serial)`.
pub fn excel_serial_to_iso(serial: f64) -> String {
    let ms = (serial - 25569.0) * 86400.0 * 1000.0;
    if !ms.is_finite() || ms.abs() > 8.64e15 {
        return "NaN-NaN-NaN".into();
    }
    let ms = ms.trunc() as i64;
    iso_from_days(ms.div_euclid(86_400_000))
}

/// `a.localeCompare(b)` for ISO dates and plain names: code-point order.
/// (The old app compared ISO dates, so plain order is the same.)
pub fn cmp_str(a: &str, b: &str) -> std::cmp::Ordering {
    a.cmp(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_fixed_like_js() {
        assert_eq!(to_fixed2(0.125), "0.13");
        assert_eq!(to_fixed2(1.005), "1.00");
        assert_eq!(to_fixed2(105.16999999999999), "105.17");
        assert_eq!(to_fixed2(-1.5), "-1.50");
        assert_eq!(n2(0.1 + 0.2), 0.3);
        assert_eq!(n4(36.12344999), 36.1234);
    }

    #[test]
    fn format_eur_like_js() {
        assert_eq!(format_eur(1234.5), "€1.234,50");
        assert_eq!(format_eur(-1.5), "€-1,50");
        assert_eq!(format_eur(0.0), "€0,00");
        assert_eq!(format_eur(1_000_000.0), "€1.000.000,00");
        // values checked in node: toLocaleString('de-DE', 2 digits)
        for (v, js) in [
            (22838.535, "22.838,54"),
            (-0.001, "-0,00"),
            (-0.0, "-0,00"),
            (0.005, "0,01"),
            (1.005, "1,01"),
            (2.675, "2,68"),
            (1e21, "1.000.000.000.000.000.000.000,00"),
            (123456789.125, "123.456.789,13"),
            (-1234.5, "-1.234,50"),
            (0.000001, "0,00"),
            (99.995, "100,00"),
        ] {
            assert_eq!(to_locale_de2(v), js, "{v}");
        }
    }

    #[test]
    fn parse_float_like_js() {
        assert_eq!(parse_float("12abc"), Some(12.0));
        assert_eq!(parse_float(".5"), Some(0.5));
        assert_eq!(parse_float("1e3x"), Some(1000.0));
        assert_eq!(parse_float("-.5e-1"), Some(-0.05));
        assert_eq!(parse_float("+3"), Some(3.0));
        assert_eq!(parse_float("1.2.3"), Some(1.2));
        assert_eq!(parse_float("e5"), None);
        assert_eq!(parse_float("5."), Some(5.0));
        assert_eq!(parse_float("1e"), Some(1.0));
        assert_eq!(parse_float("Infinityx"), Some(f64::INFINITY));
        assert_eq!(parse_float(""), None);
        assert_eq!(parse_float("."), None);
    }

    #[test]
    fn dates_like_js() {
        assert_eq!(day_name("2026-02-31"), "Tuesday"); // rolls over to 03-03
        assert_eq!(day_name("2026-13-01"), "");
        assert_eq!(day_name("2026-1-5"), "");
        assert_eq!(day_name("0000-01-01"), "Saturday");
        assert_eq!(day_name("2026-10-15"), "Thursday");
        assert_eq!(format_date("2026-10-15"), "15.10.2026");
        assert_eq!(excel_serial_to_iso(46310.0), "2026-10-15");
        assert_eq!(excel_serial_to_iso(46310.99), "2026-10-15");
        assert_eq!(iso_from_days(days_from_civil(2024, 2, 29)), "2024-02-29");
    }

    #[test]
    fn cents_like_math_round() {
        assert_eq!(cents(12.345), 1235); // 12.345 * 100 = 1234.5 in JavaScript too
        assert_eq!(cents(1.005), 100);
        assert_eq!(cents(101.0), 10_100);
        assert_eq!(math_round(-0.5), 0.0);
        assert_eq!(math_round(2.5), 3.0);
    }

    #[test]
    fn num_to_string_like_js() {
        assert_eq!(num_to_string(123.456), "123.456");
        assert_eq!(num_to_string(1e21), "1e+21");
        assert_eq!(num_to_string(0.1 + 0.2), "0.30000000000000004");
        assert_eq!(num_to_string(1234.0), "1234");
    }
}
