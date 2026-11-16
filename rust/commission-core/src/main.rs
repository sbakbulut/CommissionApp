//! Komut satırı programı.
//!
//! Kullanım:
//!   cargo run -- examples/sales.csv --target 31000
//!
//! CSV biçimi (ayıraç `;` veya TAB, çünkü tutarlarda virgül olabilir):
//!   category;amount
//!   icon;1.234,56
//!   silver;117

use std::process::ExitCode;
use std::{env, fs};

use commission_core::{Category, Money, Rules, Sale, calculate};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("Hata: {msg}");
            eprintln!("Kullanım: commission-core <satislar.csv> [--target <tutar>]");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut path = None;
    let mut target = None;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--target" | "-t" => {
                let value = args.next().ok_or("--target için tutar eksik")?;
                target = Some(Money::parse(&value).map_err(|e| e.to_string())?);
            }
            _ if path.is_none() => path = Some(arg),
            _ => return Err(format!("beklenmeyen argüman: {arg}")),
        }
    }
    let path = path.ok_or("CSV dosyası verilmedi")?;
    let text = fs::read_to_string(&path).map_err(|e| format!("{path}: {e}"))?;
    let sales = parse_csv(&text)?;

    let report = calculate(&sales, target, &Rules::default());

    println!(
        "{:>3}  {:<8} {:>14} {:>7} {:>12}",
        "#", "Kategori", "Tutar", "Oran", "Komisyon"
    );
    println!("{}", "─".repeat(48));
    for (i, line) in report.lines.iter().enumerate() {
        println!(
            "{:>3}  {:<8} {:>14} {:>7} {:>12}",
            i + 1,
            line.sale.category,
            line.sale.amount,
            line.rate,
            line.commission
        );
    }
    println!("{}", "─".repeat(48));
    for c in report.by_category().iter().filter(|c| c.count > 0) {
        println!(
            "     {:<8} {:>14} {:>7} {:>12}",
            c.category, c.sales, "", c.commission
        );
    }
    println!("{}", "─".repeat(48));
    println!(
        "     {:<8} {:>14} {:>7} {:>12}",
        "TOPLAM", report.total_sales, "", report.total_commission
    );
    println!();
    match report.target {
        None => println!("Hedef: ayarlanmadı"),
        Some(t) if report.target_met => println!("Hedef: {t} → TUTTU ✓"),
        Some(t) => println!("Hedef: {t} → tutmadı (eksik: {})", t - report.total_sales),
    }
    Ok(())
}

fn parse_csv(text: &str) -> Result<Vec<Sale>, String> {
    let mut sales = Vec::new();
    for (n, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (cat, amount) = line
            .split_once(';')
            .or_else(|| line.split_once('\t'))
            .ok_or_else(|| format!("satır {}: ';' veya TAB yok: {line:?}", n + 1))?;
        // Başlık satırını atla.
        if sales.is_empty() && cat.trim().eq_ignore_ascii_case("category") {
            continue;
        }
        let category: Category = cat.parse().map_err(|e| format!("satır {}: {e}", n + 1))?;
        let amount = Money::parse(amount).map_err(|e| format!("satır {}: {e}", n + 1))?;
        sales.push(Sale { category, amount });
    }
    Ok(sales)
}
