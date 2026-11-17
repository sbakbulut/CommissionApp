//! Excel report contents (rows of cells). Port of `buildExcelData()`,
//! `buildPersonSheetData()`, `buildTeamSummaryData()` and `buildAllSalesData()`.
//!
//! Numbers follow the old app exactly: sums are added as JavaScript numbers
//! and written with `n2()` (= `toFixed(2)`).

use std::cmp::Ordering;

use commission_core::Category;

use crate::calc::{self, CATS};
use crate::fx::{self, FxRate};
use crate::import::Item;
use crate::js::{self, n2, n4};
use crate::state::Rows;

/// One output cell.
#[derive(Debug, Clone, PartialEq)]
pub enum Out {
    S(String),
    N(f64),
}

pub type SheetData = Vec<Vec<Out>>;

fn s(v: impl Into<String>) -> Out {
    Out::S(v.into())
}

fn n(v: f64) -> Out {
    Out::N(v)
}

fn row<const K: usize>(cells: [Out; K]) -> Vec<Out> {
    cells.into()
}

/// `value || '—'`
fn or_dash_num(v: f64) -> Out {
    if v == 0.0 || v.is_nan() {
        s("—")
    } else {
        n(v)
    }
}

fn or_dash(v: &str) -> Out {
    if v.is_empty() { s("—") } else { s(v) }
}

/// One sale line in a report.
#[derive(Debug, Clone)]
pub struct Line {
    pub date: String,
    pub category: Category,
    pub amount: f64,
    /// Rate as a fraction (0.015).
    pub rate: f64,
    /// Commission in euros.
    pub prim: f64,
    pub day: String,
    pub desc: String,
    pub sku: String,
}

/// Currency settings of an export: `None` = EUR only.
pub type Fx<'a> = Option<&'a FxRate>;

fn k_of(fx: Fx) -> f64 {
    fx.map_or(0.0, |f| f.rate)
}

/// The old `flattenRows()`: rows with a value > 0, sorted by date.
pub fn flatten_rows(rows: &Rows, met: bool, today: &str) -> Vec<Line> {
    let mut all = Vec::new();
    for (cat, list) in CATS.iter().zip(rows) {
        for r in list {
            if r.value.is_nan() || r.value <= 0.0 {
                continue;
            }
            let date = if r.date.is_empty() {
                today.to_string()
            } else {
                r.date.clone()
            };
            all.push(Line {
                day: js::day_name(&date).into(),
                date,
                category: *cat,
                amount: r.value,
                rate: calc::rate(*cat, r.value, met),
                prim: calc::commission(*cat, r.value, met),
                desc: r.desc.clone().unwrap_or_default(),
                sku: r.sku.clone().unwrap_or_default(),
            });
        }
    }
    all.sort_by(|a, b| js::cmp_str(&a.date, &b.date));
    all
}

/// Total sales of all rows, exact (the old `getTotalSales()`).
pub fn total_sales(rows: &Rows) -> f64 {
    calc::sum(
        rows.iter()
            .flatten()
            .map(|r| if r.value.is_nan() { 0.0 } else { r.value }),
    )
}

fn value(v: f64) -> f64 {
    if v.is_nan() { 0.0 } else { v }
}

fn rates_head(fx: bool) -> Vec<Out> {
    let mut h = vec![
        s("Category"),
        s("Rate (%)"),
        s("Total Sales (€)"),
        s("Total Commission (€)"),
    ];
    if fx {
        h.extend([s("Total Sales (₺)"), s("Total Commission (₺)")]);
    }
    h
}

fn daily_head(fx: bool) -> Vec<Out> {
    let mut h = vec![
        s("Date"),
        s("Day"),
        s("Sales Count"),
        s("Total Sales (€)"),
        s("Total Commission (€)"),
    ];
    if fx {
        h.extend([
            s("EUR/TRY"),
            s("Total Sales (₺)"),
            s("Total Commission (₺)"),
        ]);
    }
    h
}

#[derive(Default)]
struct Day {
    count: usize,
    sum: f64,
    prim: f64,
    ts: f64,
    tp: f64,
}

/// DAILY SUMMARY rows (sorted by date). TRY = sum of the converted lines of that day.
fn daily_rows(lines: &[Line], fx: Fx) -> Vec<Vec<Out>> {
    let k = k_of(fx);
    let mut days: Vec<(String, Day)> = Vec::new();
    for x in lines {
        let i = match days.iter().position(|(d, _)| *d == x.date) {
            Some(i) => i,
            None => {
                days.push((x.date.clone(), Day::default()));
                days.len() - 1
            }
        };
        let b = &mut days[i].1;
        b.count += 1;
        b.sum += x.amount;
        b.prim += x.prim;
        b.ts += calc::convert(x.amount, k);
        b.tp += calc::convert(x.prim, k);
    }
    days.sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()));
    days.into_iter()
        .map(|(d, b)| {
            let mut r = vec![
                s(js::format_date(&d)),
                s(js::day_name(&d)),
                n(b.count as f64),
                n(n2(b.sum)),
                n(n2(b.prim)),
            ];
            if fx.is_some() {
                r.extend([n(n4(k)), n(n2(b.ts)), n(n2(b.tp))]);
            }
            r
        })
        .collect()
}

/// The old `buildExcelData()` — "Excel (My Sales)".
pub struct MyReport<'a> {
    pub rows: &'a Rows,
    pub target_amount: f64,
    pub salesperson: &'a str,
    pub generated_on: &'a str,
    pub today: &'a str,
    pub fx: Fx<'a>,
}

pub fn build_my_report(p: &MyReport) -> SheetData {
    let met = calc::target_met(total_sales(p.rows), p.target_amount);
    let k = k_of(p.fx);
    let fx = p.fx.is_some();
    let mut sums = [0.0f64; 4];
    let mut prims = [0.0f64; 4];
    for (i, (cat, list)) in CATS.iter().zip(p.rows).enumerate() {
        for r in list {
            let v = value(r.value);
            sums[i] += v;
            prims[i] += calc::commission(*cat, v, met);
        }
    }
    let total_sales_f = sums.iter().fold(0.0, |a, v| a + v);
    let total = prims.iter().fold(0.0, |a, v| a + v);
    let flat = flatten_rows(p.rows, met, p.today);

    let mut t_cat = [(0.0f64, 0.0f64); 4];
    let (mut t_sales, mut t_prim) = (0.0, 0.0);
    if fx {
        for x in &flat {
            t_sales += calc::convert(x.amount, k);
            t_prim += calc::convert(x.prim, k);
            let i = x.category.index() as usize;
            t_cat[i].0 += calc::convert(x.amount, k);
            t_cat[i].1 += calc::convert(x.prim, k);
        }
    }

    let mut d: SheetData = vec![row([s("COMMISSION CALCULATOR — SALES REPORT")]), vec![]];
    d.push(row([s("Salesperson"), or_dash(p.salesperson)]));
    d.push(row([s("Generated On"), s(p.generated_on)]));
    d.push(row([
        s("Target Status"),
        s(if met { "Met ✓" } else { "Not Met ✗" }),
    ]));
    d.push(row([s("Target Amount (€)"), or_dash_num(p.target_amount)]));
    d.push(row([s("Total Sales (€)"), n(n2(total_sales_f))]));
    d.push(row([s("Total Commission (€)"), n(n2(total))]));
    if fx {
        d.push(row([s("Total Sales (₺)"), n(n2(t_sales))]));
        d.push(row([s("Total Commission (₺)"), n(n2(t_prim))]));
        d.push(row([s("FX Source"), s(fx::source_label(p.fx))]));
    }

    d.push(vec![]);
    d.push(row([s("RATES")]));
    d.push(rates_head(fx));
    for (i, cat) in CATS.iter().enumerate() {
        let mut r = vec![
            s(cat.label()),
            n(n2(calc::table_rate_bp(*cat, met) as f64 / 100.0)),
            n(n2(sums[i])),
            n(n2(prims[i])),
        ];
        if fx {
            r.extend([n(n2(t_cat[i].0)), n(n2(t_cat[i].1))]);
        }
        d.push(r);
    }
    let mut tot = vec![s("TOTAL"), s(""), n(n2(total_sales_f)), n(n2(total))];
    if fx {
        tot.extend([n(n2(t_sales)), n(n2(t_prim))]);
    }
    d.push(tot);

    d.push(vec![]);
    d.push(row([s("DAILY SUMMARY")]));
    d.push(daily_head(fx));
    d.extend(daily_rows(&flat, p.fx));

    d.push(vec![]);
    d.push(row([s("SALES DETAIL")]));
    let mut head: Vec<Out> = [
        "#",
        "Date",
        "Day",
        "SKU",
        "Product Description",
        "Category",
        "Amount (€)",
        "Rate (%)",
        "Commission (€)",
    ]
    .into_iter()
    .map(s)
    .collect();
    if fx {
        head.extend([s("EUR/TRY"), s("Amount (₺)"), s("Commission (₺)")]);
    }
    d.push(head);
    for (i, x) in flat.iter().enumerate() {
        let mut r = vec![
            n((i + 1) as f64),
            s(js::format_date(&x.date)),
            s(x.day.clone()),
            or_dash(&x.sku),
            or_dash(&x.desc),
            s(x.category.label()),
            n(n2(x.amount)),
            n(n2(x.rate * 100.0)),
            n(n2(x.prim)),
        ];
        if fx {
            r.extend([
                n(n4(k)),
                n(n2(calc::convert(x.amount, k))),
                n(n2(calc::convert(x.prim, k))),
            ]);
        }
        d.push(r);
    }
    if flat.is_empty() {
        d.push(row([s("— No sales entered yet —")]));
    }
    d
}

/// Turkish month names for file names.
pub const TR_MONTHS: [&str; 12] = [
    "ocak", "şubat", "mart", "nisan", "mayıs", "haziran", "temmuz", "ağustos", "eylül", "ekim",
    "kasım", "aralık",
];

/// The old `myReportFileName()`: "<salesperson> <month>.xlsx".
pub fn my_report_file_name(salesperson: &str, lines: &[Line], today: &str) -> String {
    let base = if salesperson.is_empty() {
        "commission report"
    } else {
        salesperson
    };
    let cleaned: String = base
        .replace('İ', "i")
        .to_lowercase()
        .chars()
        .filter(|c| !matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        .collect();
    let collapsed = cleaned
        .split(js::is_js_space)
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let name = if collapsed.is_empty() {
        "commission report".to_string()
    } else {
        collapsed
    };
    let month = fx::report_month(lines.iter().map(|l| l.date.as_str()), today);
    let idx: usize = month.get(5..7).and_then(|m| m.parse().ok()).unwrap_or(0);
    let month_name = if (1..=12).contains(&idx) {
        TR_MONTHS[idx - 1]
    } else {
        ""
    };
    format!("{}.xlsx", js::trim(&format!("{name} {month_name}")))
}

// ─── Team ────────────────────────────────────────────────────────────

/// One salesperson in the team report (the old `computePersonStats()`).
#[derive(Debug, Clone)]
pub struct Person {
    pub name: String,
    pub items: Vec<Item>,
    pub total_sales: f64,
    pub total_prim: f64,
    pub achieved: bool,
    pub sums: [f64; 4],
    pub prims: [f64; 4],
    /// Sorted by date.
    pub flat: Vec<Line>,
}

pub fn person_stats(name: String, items: Vec<Item>, target: f64) -> Person {
    let total_sales = calc::sum(items.iter().map(|it| it.amount));
    let achieved = calc::target_met(total_sales, target);
    let mut sums = [0.0; 4];
    let mut prims = [0.0; 4];
    let mut flat: Vec<Line> = items
        .iter()
        .map(|it| {
            let i = it.category.index() as usize;
            let prim = calc::commission(it.category, it.amount, achieved);
            sums[i] += it.amount;
            prims[i] += prim;
            Line {
                date: it.date.clone(),
                category: it.category,
                amount: it.amount,
                rate: calc::rate(it.category, it.amount, achieved),
                prim,
                day: js::day_name(&it.date).into(),
                desc: it.code.clone(),
                sku: it.sku.clone(),
            }
        })
        .collect();
    flat.sort_by(|a, b| js::cmp_str(&a.date, &b.date));
    let total_prim = prims.iter().fold(0.0, |a, v| a + v);
    Person {
        name,
        items,
        total_sales,
        total_prim,
        achieved,
        sums,
        prims,
        flat,
    }
}

/// The old `tryTotals(st)`: (sales, commission, per category) in TRY.
fn try_totals(p: &Person, k: f64) -> (f64, f64, [(f64, f64); 4]) {
    let mut per = [(0.0, 0.0); 4];
    let (mut sales, mut prim) = (0.0, 0.0);
    for x in &p.flat {
        let (a, c) = (calc::convert(x.amount, k), calc::convert(x.prim, k));
        sales += a;
        prim += c;
        let i = x.category.index() as usize;
        per[i].0 += calc::convert(x.amount, k);
        per[i].1 += calc::convert(x.prim, k);
    }
    (sales, prim, per)
}

/// The old `buildPersonSheetData(person, target)`.
pub fn build_person_sheet(p: &Person, target: f64, generated_on: &str, fx_rate: Fx) -> SheetData {
    let fx = fx_rate.is_some();
    let k = k_of(fx_rate);
    let t = try_totals(p, k);
    let mut d: SheetData = vec![row([s("COMMISSION REPORT")]), vec![]];
    d.push(row([s("Salesperson"), s(p.name.clone())]));
    d.push(row([s("Generated On"), s(generated_on)]));
    d.push(row([s("Target Amount (€)"), or_dash_num(target)]));
    d.push(row([
        s("Target Status"),
        s(if p.achieved { "Met ✓" } else { "Not Met ✗" }),
    ]));
    d.push(row([s("Total Sales (€)"), n(n2(p.total_sales))]));
    d.push(row([s("Total Commission (€)"), n(n2(p.total_prim))]));
    if fx {
        d.push(row([s("Total Sales (₺)"), n(n2(t.0))]));
        d.push(row([s("Total Commission (₺)"), n(n2(t.1))]));
        d.push(row([s("FX Source"), s(fx::source_label(fx_rate))]));
    }
    d.push(vec![]);
    d.push(row([s("RATES")]));
    d.push(rates_head(fx));
    for (i, cat) in CATS.iter().enumerate() {
        let mut r = vec![
            s(cat.label()),
            n(n2(calc::table_rate_bp(*cat, p.achieved) as f64 / 100.0)),
            n(n2(p.sums[i])),
            n(n2(p.prims[i])),
        ];
        if fx {
            r.extend([n(n2(t.2[i].0)), n(n2(t.2[i].1))]);
        }
        d.push(r);
    }
    let mut tot = vec![s("TOTAL"), s(""), n(n2(p.total_sales)), n(n2(p.total_prim))];
    if fx {
        tot.extend([n(n2(t.0)), n(n2(t.1))]);
    }
    d.push(tot);

    d.push(vec![]);
    d.push(row([s("DAILY SUMMARY")]));
    d.push(daily_head(fx));
    d.extend(daily_rows(&p.flat, fx_rate));

    d.push(vec![]);
    d.push(row([s("SALES DETAIL")]));
    let mut head: Vec<Out> = [
        "#",
        "Date",
        "Day",
        "Salesperson",
        "SKU",
        "Product Description",
        "Category",
        "Amount (€)",
        "Rate (%)",
        "Commission (€)",
    ]
    .into_iter()
    .map(s)
    .collect();
    if fx {
        head.extend([s("EUR/TRY"), s("Amount (₺)"), s("Commission (₺)")]);
    }
    d.push(head);
    for (i, x) in p.flat.iter().enumerate() {
        let mut r = vec![
            n((i + 1) as f64),
            s(js::format_date(&x.date)),
            s(x.day.clone()),
            s(p.name.clone()),
            or_dash(&x.sku),
            or_dash(&x.desc),
            s(x.category.label()),
            n(n2(x.amount)),
            n(n2(x.rate * 100.0)),
            n(n2(x.prim)),
        ];
        if fx {
            r.extend([
                n(n4(k)),
                n(n2(calc::convert(x.amount, k))),
                n(n2(calc::convert(x.prim, k))),
            ]);
        }
        d.push(r);
    }
    d
}

/// The old `buildTeamSummaryData(target)`.
pub fn build_team_summary(
    team: &[Person],
    target: f64,
    generated_on: &str,
    fx_rate: Fx,
) -> SheetData {
    let fx = fx_rate.is_some();
    let k = k_of(fx_rate);
    let mut d: SheetData = vec![row([s("TEAM COMMISSION SUMMARY")]), vec![]];
    d.push(row([s("Generated On"), s(generated_on)]));
    d.push(row([s("Target per Person (€)"), or_dash_num(target)]));
    d.push(row([s("Salespeople"), n(team.len() as f64)]));
    d.push(row([
        s("Currency"),
        s(if fx {
            "EUR + TRY (converted at each sale date)"
        } else {
            "EUR"
        }),
    ]));
    if fx {
        d.push(row([s("FX Source"), s(fx::FX_SOURCE)]));
    }
    d.push(vec![]);
    let mut head: Vec<Out> = [
        "#",
        "Salesperson",
        "Sales Count",
        "Silver (€)",
        "Icon (€)",
        "Others (€)",
        "Watches (€)",
        "Total Sales (€)",
        "Total Commission (€)",
        "Target",
    ]
    .into_iter()
    .map(s)
    .collect();
    if fx {
        head.extend([s("Total Sales (₺)"), s("Total Commission (₺)")]);
    }
    d.push(head);
    let (mut gs, mut gp) = (0.0, 0.0);
    for (i, p) in team.iter().enumerate() {
        let mut r = vec![
            n((i + 1) as f64),
            s(p.name.clone()),
            n(p.items.len() as f64),
            n(n2(p.sums[0])),
            n(n2(p.sums[1])),
            n(n2(p.sums[2])),
            n(n2(p.sums[3])),
            n(n2(p.total_sales)),
            n(n2(p.total_prim)),
            s(if p.achieved { "Met ✓" } else { "Not Met ✗" }),
        ];
        if fx {
            let t = try_totals(p, k);
            gs += t.0;
            gp += t.1;
            r.extend([n(n2(t.0)), n(n2(t.1))]);
        }
        d.push(r);
    }
    let sum = |f: &dyn Fn(&Person) -> f64| team.iter().fold(0.0, |a, p| a + f(p));
    let mut tot = vec![
        s("TOTAL"),
        s(""),
        n(sum(&|p| p.items.len() as f64)),
        n(n2(sum(&|p| p.sums[0]))),
        n(n2(sum(&|p| p.sums[1]))),
        n(n2(sum(&|p| p.sums[2]))),
        n(n2(sum(&|p| p.sums[3]))),
        n(n2(sum(&|p| p.total_sales))),
        n(n2(sum(&|p| p.total_prim))),
        s(""),
    ];
    if fx {
        tot.extend([n(n2(gs)), n(n2(gp))]);
    }
    d.push(tot);
    d
}

/// The old `buildAllSalesData()`. `cmp_names` = `localeCompare` for names.
pub fn build_all_sales(
    team: &[Person],
    fx_rate: Fx,
    cmp_names: &dyn Fn(&str, &str) -> Ordering,
) -> SheetData {
    let fx = fx_rate.is_some();
    let k = k_of(fx_rate);
    let mut d: SheetData = vec![row([s("ALL SALES DETAIL")]), vec![]];
    let mut head: Vec<Out> = [
        "#",
        "Salesperson",
        "Date",
        "Day",
        "SKU",
        "Product Description",
        "Category",
        "Amount (€)",
        "Rate (%)",
        "Commission (€)",
    ]
    .into_iter()
    .map(s)
    .collect();
    if fx {
        head.extend([s("EUR/TRY"), s("Amount (₺)"), s("Commission (₺)")]);
    }
    d.push(head);
    let mut all: Vec<(&str, &Line)> = team
        .iter()
        .flat_map(|p| p.flat.iter().map(move |x| (p.name.as_str(), x)))
        .collect();
    all.sort_by(|a, b| js::cmp_str(&a.1.date, &b.1.date).then_with(|| cmp_names(a.0, b.0)));
    let (mut ts, mut tp) = (0.0, 0.0);
    for (i, (person, x)) in all.iter().enumerate() {
        let mut r = vec![
            n((i + 1) as f64),
            s(*person),
            s(js::format_date(&x.date)),
            s(x.day.clone()),
            or_dash(&x.sku),
            or_dash(&x.desc),
            s(x.category.label()),
            n(n2(x.amount)),
            n(n2(x.rate * 100.0)),
            n(n2(x.prim)),
        ];
        if fx {
            ts += calc::convert(x.amount, k);
            tp += calc::convert(x.prim, k);
            r.extend([
                n(n4(k)),
                n(n2(calc::convert(x.amount, k))),
                n(n2(calc::convert(x.prim, k))),
            ]);
        }
        d.push(r);
    }
    let mut tot = vec![s("TOTAL"), s(""), s(""), s(""), s(""), s(""), s("")];
    tot.push(n(n2(all.iter().fold(0.0, |a, x| a + x.1.amount))));
    tot.push(s(""));
    tot.push(n(n2(all.iter().fold(0.0, |a, x| a + x.1.prim))));
    if fx {
        tot.extend([s(""), n(n2(ts)), n(n2(tp))]);
    }
    d.push(tot);
    d
}

/// The old `safeSheetName(name, used)`.
pub fn safe_sheet_name(name: &str, used: &mut Vec<String>) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if "[]*/\\?:".contains(c) { ' ' } else { c })
        .collect();
    let trimmed: String = js::trim(&cleaned).chars().take(28).collect();
    let base = if trimmed.is_empty() {
        "Sheet".to_string()
    } else {
        trimmed
    };
    let mut n = base.clone();
    let mut i = 2;
    while used.contains(&n) {
        n = format!("{} {i}", base.chars().take(26).collect::<String>());
        i += 1;
    }
    used.push(n.clone());
    n
}

/// `p.name.replace(/[^A-Za-z0-9]+/g, '_').replace(/^_|_$/g, '')`
pub fn safe_file_part(name: &str) -> String {
    let mut out = String::new();
    let mut in_gap = false;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
            in_gap = false;
        } else if !in_gap {
            out.push('_');
            in_gap = true;
        }
    }
    let out = out.strip_prefix('_').unwrap_or(&out);
    out.strip_suffix('_').unwrap_or(out).to_string()
}

/// Column widths (characters) — the old `sheetCols(kind)` and the "My Sales" list.
pub fn col_widths(kind: &str, fx: bool) -> Vec<f64> {
    let mut v: Vec<f64> = match kind {
        "summary" => vec![5., 26., 11., 13., 13., 13., 13., 15., 17., 12.],
        "all" => vec![5., 26., 12., 11., 13., 34., 11., 12., 9., 14.],
        "person" => vec![5., 14., 11., 26., 13., 34., 11., 12., 9., 14.],
        _ => vec![6., 12., 12., 14., 34., 12., 12., 10., 14.],
    };
    if fx {
        v.extend([if kind == "summary" { 16. } else { 10. }, 16., 18.]);
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::SaleRow;

    fn sale(v: f64, date: &str) -> SaleRow {
        SaleRow::new(1, v, date.into())
    }

    #[test]
    fn my_report_totals_match_detail() {
        let rows: Rows = [
            vec![sale(117.0, "2026-10-15"), sale(119.0, "2026-10-16")],
            vec![sale(101.0, "2026-10-15"), sale(1001.5, "2026-10-16")],
            vec![sale(31000.0, "2026-10-15")],
            vec![sale(0.0, "2026-10-15")],
        ];
        let fx = FxRate {
            rate: 36.1234,
            date: "2026-10-30".into(),
            target: "2026-10-31".into(),
            month: "2026-10".into(),
            is_current: false,
        };
        let d = build_my_report(&MyReport {
            rows: &rows,
            target_amount: 50000.0,
            salesperson: "ALI",
            generated_on: "x",
            today: "2026-11-17",
            fx: Some(&fx),
        });
        let get = |label: &str| {
            d.iter()
                .find(|r| r.first() == Some(&Out::S(label.into())))
                .map(|r| r[1].clone())
        };
        assert_eq!(get("Total Commission (€)"), Some(Out::N(327.73)));
        let detail: f64 = d
            .iter()
            .skip_while(|r| r.first() != Some(&Out::S("#".into())))
            .skip(1)
            .filter_map(|r| match r.get(11) {
                Some(Out::N(v)) => Some(*v),
                _ => None,
            })
            .sum();
        let total_try = match get("Total Commission (₺)") {
            Some(Out::N(v)) => v,
            _ => panic!(),
        };
        assert!((detail - total_try).abs() < 1e-9);
    }

    #[test]
    fn file_names() {
        let l = flatten_rows(
            &[vec![sale(10.0, "2026-10-02")], vec![], vec![], vec![]],
            false,
            "2026-11-17",
        );
        assert_eq!(
            my_report_file_name("SAFA BEKİR AKBULUT", &l, "2026-11-17"),
            "safa bekir akbulut ekim.xlsx"
        );
        assert_eq!(
            my_report_file_name("", &[], "2026-11-17"),
            "commission report kasım.xlsx"
        );
        assert_eq!(safe_file_part("ALİ VELİ"), "AL_VEL");
        let mut used = vec![];
        assert_eq!(safe_sheet_name("A/B", &mut used), "A B");
        assert_eq!(safe_sheet_name("A/B", &mut used), "A B 2");
    }
}
