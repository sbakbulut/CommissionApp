//! Parity helper for `tools/check-parity.mjs`: reads JSON cases on stdin,
//! writes the Rust results as JSON lines on stdout.
use std::io::{self, BufRead, Write};

use commission_app::import::{self, Cell};
use commission_app::{calc, tiffany};
use serde_json::{Value, json};

fn cell(v: &Value) -> Cell {
    match v {
        Value::Null => Cell::Empty,
        Value::String(s) => Cell::Str(s.clone()),
        Value::Number(n) => Cell::Num(n.as_f64().unwrap_or(f64::NAN)),
        Value::Bool(b) => Cell::Bool(*b),
        other => Cell::Str(other.to_string()),
    }
}

fn main() {
    let stdin = io::stdin();
    let mut out = io::BufWriter::new(io::stdout());
    for line in stdin.lock().lines() {
        let case: Value = serde_json::from_str(&line.unwrap()).unwrap();
        let r = match case["k"].as_str().unwrap() {
            "code" => {
                let (cat, p) = tiffany::categorize(
                    case["code"].as_str().unwrap(),
                    case["sku"].as_str().unwrap(),
                );
                json!([calc::key(cat), p.collection, p.material])
            }
            "amount" => json!(import::parse_amount(&cell(&case["v"]))),
            "date" => json!(import::parse_date(&cell(&case["v"]))),
            "header" => {
                let rows: Vec<Vec<Cell>> = case["rows"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|r| r.as_array().unwrap().iter().map(cell).collect())
                    .collect();
                let h = import::find_header_row(&rows);
                let heads: Vec<String> = rows
                    .get(h)
                    .map(|r| r.iter().map(|c| c.text().trim().to_lowercase()).collect())
                    .unwrap_or_default();
                json!([h, import::find_person_col(&heads)])
            }
            "html" => {
                let g = import::parse_html_table(case["html"].as_str().unwrap());
                json!(
                    g.iter()
                        .map(|r| r.iter().map(|c| c.display()).collect::<Vec<_>>())
                        .collect::<Vec<_>>()
                )
            }
            other => panic!("unknown kind {other}"),
        };
        writeln!(out, "{r}").unwrap();
    }
}
