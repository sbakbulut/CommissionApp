//! EUR → TRY rate (TCMB): which day, labels, and reading the answers.
//!
//! v13.8 rule: the whole report uses ONE rate — the last published TCMB
//! "döviz satış" rate of the report month (the latest sales month). If that
//! month is not over yet, the latest rate up to today is used.

use crate::js;

pub const FX_SOURCE: &str = "T.C. Merkez Bankası (TCMB) döviz satış kuru";
pub const FX_MAX_BACK: usize = 15;
pub const TCMB_WORKER: &str = "https://tcmbproxy.sbakbulut.workers.dev";

#[derive(Debug, Clone, PartialEq)]
pub struct FxRate {
    pub rate: f64,
    /// Day of the rate that was used.
    pub date: String,
    /// Day that was asked for.
    pub target: String,
    pub month: String,
    pub is_current: bool,
}

/// `'YYYY-MM-DD'` → `'YYYY-MM'`
pub fn month_of(iso: &str) -> String {
    iso.chars().take(7).collect()
}

/// The old `fxReportMonth(isoDates)`: latest month, or today's month.
pub fn report_month<'a>(dates: impl Iterator<Item = &'a str>, today: &str) -> String {
    dates
        .filter(|d| !d.is_empty())
        .map(month_of)
        .max()
        .unwrap_or_else(|| month_of(today))
}

/// The old `fxTargetForMonth(month)`: (day to ask for, month not over yet).
pub fn target_for_month(month: &str, today: &str) -> (String, bool) {
    let mut parts = month.split('-');
    let y: i64 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(1970);
    let m: i64 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(1);
    let (ny, nm) = if m >= 12 { (y + 1, 1) } else { (y, m + 1) };
    let month_end = js::days_from_civil(ny, nm, 1) - 1;
    let today_days = js::days_from_iso(today).unwrap_or(month_end);
    let is_current = month_end > today_days;
    (
        js::iso_from_days(if is_current { today_days } else { month_end }),
        is_current,
    )
}

/// One day back: `YYYY-MM-DD` → previous day.
pub fn previous_day(iso: &str) -> String {
    js::days_from_iso(iso).map_or_else(|| iso.to_string(), |d| js::iso_from_days(d - 1))
}

/// The old `tcmbXmlUrl(ds)`.
pub fn tcmb_xml_url(ds: &str) -> String {
    let mut p = ds.split('-');
    let (y, m, d) = (
        p.next().unwrap_or(""),
        p.next().unwrap_or(""),
        p.next().unwrap_or(""),
    );
    format!("https://www.tcmb.gov.tr/kurlar/{y}{m}/{d}{m}{y}.xml")
}

pub fn encode_uri_component(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Public CORS proxies (backup when the worker cannot be reached): (url, JSON-wrapped).
pub fn proxies(target: &str) -> Vec<(String, bool)> {
    let e = encode_uri_component(target);
    vec![
        (
            format!("https://api.codetabs.com/v1/proxy?quest={e}"),
            false,
        ),
        (format!("https://api.allorigins.win/raw?url={e}"), false),
        (format!("https://api.cors.lol/url={target}"), false),
        (format!("https://corsproxy.io/?url={e}"), false),
        (format!("https://whateverorigin.org/get?url={e}"), true),
    ]
}

/// The old `fxSourceLabel()`.
pub fn source_label(fx: Option<&FxRate>) -> String {
    match fx {
        None => format!("{FX_SOURCE} — kur alınmadı"),
        Some(f) => format!(
            "{FX_SOURCE} — rapor ayı ({}) son iş günü TEK kuru: {} = {}{} — tüm satırlara aynı kur uygulandı",
            f.month,
            f.date,
            js::num_to_string(f.rate),
            if f.is_current { " [ay sürüyor]" } else { "" }
        ),
    }
}

/// The old `fxRateSummaryShort()`.
pub fn summary_short(fx: Option<&FxRate>) -> String {
    fx.map_or_else(
        || "—".into(),
        |f| format!("{} ({})", f.date, js::num_to_string(f.rate)),
    )
}

/// The "💱 TRY kuru" box (HTML), or `None` when EUR is selected.
pub fn note_html(is_try: bool, fx: Option<&FxRate>) -> Option<String> {
    if !is_try {
        return None;
    }
    Some(match fx {
        None => "💱 TRY seçildi — TCMB kuru Excel indirilirken çekilecek (rapor ayının son iş günü kuru, tüm satırlara tek kur).".into(),
        Some(f) => format!(
            "💱 TRY kuru (TCMB, rapor ayı {}{} son iş günü, tüm satırlara tek kur): <strong>{} = {}</strong>",
            f.month,
            if f.is_current { " <em>— ay sürüyor</em>" } else { "" },
            f.date,
            js::num_to_string(f.rate)
        ),
    })
}

/// Reads the EUR "ForexSelling" value from a TCMB XML day file.
pub fn parse_tcmb_xml(text: &str) -> Result<f64, String> {
    let mut rest = text;
    while let Some(start) = rest.find("<Currency") {
        let after = &rest[start..];
        let open_end = after.find('>').ok_or("bad xml")?;
        let open = &after[..open_end];
        let close = after.find("</Currency>").unwrap_or(after.len());
        let body = &after[open_end..close];
        let is_eur = open.contains("Kod=\"EUR\"") || open.contains("CurrencyCode=\"EUR\"");
        if is_eur {
            let v = body
                .split_once("<ForexSelling>")
                .and_then(|(_, r)| r.split_once("</ForexSelling>"))
                .map(|(v, _)| js::trim(v).replacen(',', ".", 1))
                .ok_or("no rate")?;
            return match js::parse_float(&v) {
                Some(x) if x != 0.0 && !x.is_nan() => Ok(x),
                _ => Err("no rate".into()),
            };
        }
        rest = &after[close.min(after.len())..];
        if close == after.len() {
            break;
        }
    }
    Err("EUR not found".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_day() {
        assert_eq!(
            target_for_month("2026-10", "2026-11-17"),
            ("2026-10-31".into(), false)
        );
        assert_eq!(
            target_for_month("2026-11", "2026-11-17"),
            ("2026-11-17".into(), true)
        );
        assert_eq!(
            target_for_month("2026-12", "2027-01-05"),
            ("2026-12-31".into(), false)
        );
        assert_eq!(
            target_for_month("2024-02", "2026-01-01"),
            ("2024-02-29".into(), false)
        );
        assert_eq!(
            report_month(["2026-08-31", "2026-09-01", ""].into_iter(), "2026-11-17"),
            "2026-09"
        );
        assert_eq!(report_month([].into_iter(), "2026-11-17"), "2026-11");
        assert_eq!(previous_day("2026-03-01"), "2026-02-28");
    }

    #[test]
    fn urls_and_labels() {
        assert_eq!(
            tcmb_xml_url("2026-10-30"),
            "https://www.tcmb.gov.tr/kurlar/202610/30102026.xml"
        );
        let f = FxRate {
            rate: 36.1234,
            date: "2026-10-30".into(),
            target: "2026-10-31".into(),
            month: "2026-10".into(),
            is_current: false,
        };
        assert_eq!(summary_short(Some(&f)), "2026-10-30 (36.1234)");
        assert!(
            source_label(Some(&f))
                .contains("rapor ayı (2026-10) son iş günü TEK kuru: 2026-10-30 = 36.1234 —")
        );
    }

    #[test]
    fn xml() {
        let xml = r#"<Tarih_Date><Currency CrossOrder="0" Kod="USD" CurrencyCode="USD"><ForexSelling>32.1</ForexSelling></Currency>
        <Currency CrossOrder="9" Kod="EUR" CurrencyCode="EUR"><Unit>1</Unit><ForexSelling>36,1234</ForexSelling></Currency></Tarih_Date>"#;
        assert_eq!(parse_tcmb_xml(xml), Ok(36.1234));
        assert!(parse_tcmb_xml("<x/>").is_err());
    }
}
