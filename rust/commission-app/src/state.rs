//! Saved data (browser `localStorage`), in the SAME format as the old app.
//! Old saved sales and settings load without change.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::calc::{CATS, key};

pub const DATA_KEY: &str = "prim_data_v2";
pub const SETTINGS_KEY: &str = "prim_settings_v1";
pub const COLLAPSE_KEY: &str = "prim_cat_collapsed";

/// One sales row. Unknown fields are kept as they are.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SaleRow {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "lenient_id"
    )]
    pub id: Option<i64>,
    #[serde(
        default,
        deserialize_with = "number_or_zero",
        serialize_with = "js_number"
    )]
    pub value: f64,
    #[serde(default, deserialize_with = "lenient_string")]
    pub date: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "lenient_opt_string"
    )]
    pub desc: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "lenient_opt_string"
    )]
    pub sku: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Writes `117` (not `117.0`), like `JSON.stringify`.
fn js_number<S: serde::Serializer>(v: &f64, s: S) -> Result<S::Ok, S::Error> {
    if v.is_finite() && v.fract() == 0.0 && v.abs() < 9.007_199_254_740_992e15 {
        s.serialize_i64(*v as i64)
    } else {
        s.serialize_f64(*v)
    }
}

/// `if (typeof r.value !== 'number') r.value = 0;`
fn number_or_zero<'de, D: serde::Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
    Ok(Value::deserialize(d)?.as_f64().unwrap_or(0.0))
}

fn lenient_id<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<i64>, D::Error> {
    let v = Value::deserialize(d)?;
    Ok(v.as_i64()
        .or_else(|| v.as_f64().filter(|f| f.fract() == 0.0).map(|f| f as i64)))
}

/// Text fields: a number or other value becomes text (the old app used `${...}`).
fn to_text(v: &Value) -> Option<String> {
    match v {
        Value::Null => None,
        Value::String(s) => Some(s.clone()),
        other => Some(other.to_string()),
    }
}

fn lenient_string<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    Ok(to_text(&Value::deserialize(d)?).unwrap_or_default())
}

fn lenient_opt_string<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    Ok(to_text(&Value::deserialize(d)?))
}

impl SaleRow {
    pub fn new(id: i64, value: f64, date: String) -> SaleRow {
        SaleRow {
            id: Some(id),
            value,
            date,
            desc: None,
            sku: None,
            extra: Map::new(),
        }
    }
}

/// Rows per category, in `CATS` order.
pub type Rows = [Vec<SaleRow>; 4];

pub fn default_rows(today: &str) -> Rows {
    [1, 2, 3, 4].map(|id| vec![SaleRow::new(id, 0.0, today.to_string())])
}

/// The old `loadState()`: (rows, next id).
pub fn load_rows(raw: Option<&str>, today: &str) -> (Rows, i64) {
    let parsed: Option<Map<String, Value>> = raw
        .and_then(|r| serde_json::from_str::<Value>(r).ok())
        .and_then(|v| match v {
            Value::Object(m) => Some(m),
            _ => None,
        });
    let Some(obj) = parsed else {
        return (default_rows(today), 5);
    };
    let mut max_id = 0;
    let rows: Rows = CATS.map(|c| {
        let list = obj
            .get(key(c))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        list.into_iter()
            .filter_map(|v| {
                if let Some(id) = v.get("id").and_then(Value::as_i64) {
                    max_id = max_id.max(id);
                }
                let mut row: SaleRow = serde_json::from_value(v).ok()?;
                if row.date.is_empty() {
                    row.date = today.to_string();
                }
                Some(row)
            })
            .collect()
    });
    (rows, max_id + 1)
}

/// The old `saveState()` text.
pub fn save_rows(rows: &Rows, saved_at: &str) -> String {
    let mut obj = Map::new();
    for (c, list) in CATS.iter().zip(rows) {
        obj.insert(
            key(*c).into(),
            serde_json::to_value(list).unwrap_or(Value::Array(vec![])),
        );
    }
    obj.insert("savedAt".into(), Value::String(saved_at.into()));
    Value::Object(obj).to_string()
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Settings {
    pub target_amount: f64,
    pub salesperson_name: String,
}

pub fn load_settings(raw: Option<&str>) -> Settings {
    let mut s = Settings::default();
    if let Some(Value::Object(m)) = raw.and_then(|r| serde_json::from_str::<Value>(r).ok()) {
        if let Some(t) = m.get("targetAmount").and_then(Value::as_f64) {
            s.target_amount = t;
        }
        if let Some(n) = m.get("salespersonName").and_then(Value::as_str) {
            s.salesperson_name = n.to_string();
        }
    }
    s
}

/// A number in `JSON.stringify` form (`100000`, not `100000.0`).
pub fn json_number(v: f64) -> String {
    if v.is_finite() {
        crate::js::num_to_string(v)
    } else {
        "null".into()
    }
}

/// The old `saveSettings()` text (same key order and number form as `JSON.stringify`).
pub fn save_settings(s: &Settings) -> String {
    format!(
        "{{\"targetAmount\":{},\"salespersonName\":{}}}",
        json_number(s.target_amount),
        serde_json::Value::String(s.salesperson_name.clone())
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_data_loads_and_saves_the_same_way() {
        let old = r#"{"silver":[{"id":7,"value":117,"date":"2026-10-15","desc":"SS RTT","sku":"123","note":"x"}],
            "icon":[{"id":9,"value":"bad"},{"id":3,"value":101.5,"date":""}],"others":null,"savedAt":"2026-10-15T10:00:00.000Z"}"#;
        let (rows, next) = load_rows(Some(old), "2026-11-17");
        assert_eq!(next, 10);
        assert_eq!(rows[0][0].value, 117.0);
        assert_eq!(
            rows[0][0].extra.get("note"),
            Some(&Value::String("x".into()))
        );
        assert_eq!(rows[1][0].value, 0.0);
        assert_eq!(rows[1][0].date, "2026-11-17");
        assert!(rows[2].is_empty() && rows[3].is_empty());
        let text = save_rows(&rows, "2026-11-17T00:00:00.000Z");
        let back: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(back["silver"][0]["sku"], "123");
        assert_eq!(back["silver"][0]["note"], "x");
        assert_eq!(back["icon"][1]["value"], 101.5);
        assert!(
            text.starts_with(r#"{"silver":[{"id":7,"value":117,"date":"2026-10-15""#),
            "{text}"
        );
    }

    #[test]
    fn missing_or_broken_data_gives_defaults() {
        let (rows, next) = load_rows(None, "2026-11-17");
        assert_eq!((rows[0].len(), next), (1, 5));
        let (_, next) = load_rows(Some("[1,2]"), "2026-11-17");
        assert_eq!(next, 5);
        let s = load_settings(Some(r#"{"targetAmount":31000,"salespersonName":"ALI"}"#));
        assert_eq!(
            (s.target_amount, s.salesperson_name.as_str()),
            (31000.0, "ALI")
        );
        assert_eq!(
            save_settings(&s),
            r#"{"targetAmount":31000,"salespersonName":"ALI"}"#
        );
    }
}
