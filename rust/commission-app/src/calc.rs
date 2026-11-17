//! Commission maths for the screen and the reports.
//!
//! Same rules as the old `CommissionCore` (which already used Rust):
//! amounts become whole cents with `Math.round(v * 100)`, then the integer
//! rules of `commission-core` apply (half-up rounding).

use commission_core::{Category, FxRate, Money, Rules};

use crate::js;

pub const CATS: [Category; 4] = Category::ALL;

pub fn key(c: Category) -> &'static str {
    match c {
        Category::Silver => "silver",
        Category::Icon => "icon",
        Category::Others => "others",
        Category::Watches => "watches",
    }
}

pub fn from_key(k: &str) -> Option<Category> {
    CATS.into_iter().find(|c| key(*c) == k)
}

pub fn emoji(c: Category) -> &'static str {
    match c {
        Category::Silver => "🥈",
        Category::Icon => "💎",
        Category::Others => "📦",
        Category::Watches => "⌚",
    }
}

fn rules() -> Rules {
    Rules::default()
}

/// Rate of one row in basis points (1 bp = 0.01 %).
pub fn rate_bp(cat: Category, value: f64, met: bool) -> i64 {
    rules()
        .rate_for(cat, Money::from_cents(js::cents(value)), met)
        .bp()
}

/// Rate as a fraction (0.015 for 1.5 %), like the old `getRateForRow()`.
pub fn rate(cat: Category, value: f64, met: bool) -> f64 {
    rate_bp(cat, value, met) as f64 / 10_000.0
}

/// Category rate from the rate table (no "≥ €31,000" rule), like `getRates()[cat]`, in bp.
pub fn table_rate_bp(cat: Category, met: bool) -> i64 {
    let r = rules();
    if met {
        r.target_met.get(cat).bp()
    } else {
        r.base.get(cat).bp()
    }
}

/// Commission of one row in euros (exact cents).
pub fn commission(cat: Category, value: f64, met: bool) -> f64 {
    let amount = Money::from_cents(js::cents(value));
    rules()
        .rate_for(cat, amount, met)
        .checked_apply(amount)
        .map_or(0.0, |m| m.cents() as f64 / 100.0)
}

/// EUR → TRY in euros-as-number form (exact cents).
pub fn convert(value: f64, fx: f64) -> f64 {
    FxRate::from_ten_thousandths(js::ten_thousandths(fx))
        .convert(Money::from_cents(js::cents(value)))
        .map_or(0.0, |m| m.cents() as f64 / 100.0)
}

/// Target met? A target of 0 means "not set".
pub fn target_met(total: f64, target: f64) -> bool {
    let t = js::cents(target);
    t > 0 && js::cents(total) >= t
}

/// Exact sum (in cents) of euro values.
pub fn sum(values: impl IntoIterator<Item = f64>) -> f64 {
    values.into_iter().map(js::cents).sum::<i64>() as f64 / 100.0
}

/// `1.50` for 150 bp — the old `(r * 100).toFixed(2)`.
pub fn pct_text(bp: i64) -> String {
    format!("{}.{:02}", bp / 100, bp % 100)
}

pub fn high_value_threshold() -> f64 {
    rules().high_value_threshold.cents() as f64 / 100.0
}

/// The footer text under the total.
pub fn footer_note() -> String {
    let r = rules();
    let thr = r.high_value_threshold.cents() / 100;
    let thr_text = {
        let s = thr.to_string();
        let mut out = String::new();
        for (i, ch) in s.chars().enumerate() {
            if i > 0 && (s.len() - i).is_multiple_of(3) {
                out.push('.');
            }
            out.push(ch);
        }
        out
    };
    format!(
        "When target is met, Icon & Watches rates change ({}% → {}%). Others rule: rows ≥ €{} also change ({}% → {}%). Calc engine: Rust/WASM.",
        pct_text(r.base.icon.bp()),
        pct_text(r.target_met.icon.bp()),
        thr_text,
        pct_text(r.base.others.bp()),
        pct_text(r.high_value_rate.bp()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_as_old_core() {
        assert_eq!(commission(Category::Icon, 101.0, false), 1.52);
        assert_eq!(commission(Category::Silver, 117.0, false), 0.59);
        assert_eq!(commission(Category::Others, 31000.0, true), 620.0);
        assert_eq!(rate(Category::Others, 31000.0, true), 0.02);
        assert_eq!(convert(1.52, 36.1234), 54.91);
        assert!(target_met(sum([0.1; 10]), 1.0));
        assert!(!target_met(100.0, 0.0));
        assert_eq!(pct_text(150), "1.50");
        assert!(footer_note().contains("rows ≥ €31.000 also change (1.00% → 2.00%)"));
    }
}
