//! `commission` — komisyon hesap ve denetim aracı.
//!
//! Kullanım:
//!   commission <dosya> [--target <tutar>]
//!
//! Desteklenen dosyalar:
//! - `.csv` / `.txt`: `category;amount` satırları (ayıraç `;` veya TAB).
//! - `.xlsx` / `.xls` / `.ods`: CommissionApp'in ürettiği Excel raporu.
//!   Program "SALES DETAIL" tablosunu okur, her satırı Rust ile yeniden
//!   hesaplar ve uygulamanın yazdığı komisyonla karşılaştırır (denetim).

use std::collections::BTreeMap;
use std::path::Path;
use std::process::ExitCode;
use std::{env, fs};

use calamine::{Data, Reader, open_workbook_auto};
use commission_core::{Category, Money, Report, Rules, Sale, calculate};

/// Dosyadan okunan bir satış satırı.
struct InputRow {
    /// Excel satır numarası (1'den başlar) veya CSV satır numarası.
    source_line: usize,
    person: Option<String>,
    sale: Sale,
    /// Uygulamanın Excel'e yazdığı komisyon (yalnız Excel raporunda var).
    app_commission: Option<Money>,
}

struct Input {
    rows: Vec<InputRow>,
    /// Excel raporundaki "Target Amount (€)" değeri.
    target_in_file: Option<Money>,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("Hata: {msg}");
            eprintln!("Kullanım: commission <satislar.csv | rapor.xlsx> [--target <tutar>]");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut path = None;
    let mut target_arg = None;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--target" | "-t" => {
                let value = args.next().ok_or("--target için tutar eksik")?;
                target_arg = Some(Money::parse(&value).map_err(|e| e.to_string())?);
            }
            "--help" | "-h" => {
                println!("commission <satislar.csv | rapor.xlsx> [--target <tutar>]");
                return Ok(());
            }
            _ if path.is_none() => path = Some(arg),
            _ => return Err(format!("beklenmeyen argüman: {arg}")),
        }
    }
    let path = path.ok_or("dosya verilmedi")?;

    let ext = Path::new(&path)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase)
        .unwrap_or_default();
    let input = match ext.as_str() {
        "csv" | "txt" => {
            let text = fs::read_to_string(&path).map_err(|e| format!("{path}: {e}"))?;
            read_csv(&text)?
        }
        "xlsx" | "xlsm" | "xls" | "ods" => read_report(&path)?,
        other => return Err(format!("desteklenmeyen dosya türü: .{other}")),
    };

    // Komut satırındaki hedef, dosyadakinden önce gelir.
    let target = target_arg.or(input.target_in_file);

    // Takım raporunda her kişi ayrı hesaplanır (her kişinin kendi hedefi).
    let mut groups: BTreeMap<Option<String>, Vec<&InputRow>> = BTreeMap::new();
    for row in &input.rows {
        groups.entry(row.person.clone()).or_default().push(row);
    }

    let mut mismatches = 0;
    let mut mismatch_total = Money::ZERO;
    let mut audited = 0;
    for (person, rows) in &groups {
        if let Some(name) = person {
            println!("\n═══ {name} ═══");
        }
        let sales: Vec<Sale> = rows.iter().map(|r| r.sale).collect();
        let report = calculate(&sales, target, &Rules::default());
        print_report(&report, rows);

        for (row, line) in rows.iter().zip(&report.lines) {
            if let Some(app) = row.app_commission {
                audited += 1;
                if app != line.commission {
                    mismatches += 1;
                    mismatch_total += line.commission - app;
                }
            }
        }
    }

    if audited > 0 {
        println!();
        if mismatches == 0 {
            println!("Denetim: {audited} satır kontrol edildi → uygulama ile Rust AYNI ✓");
        } else {
            println!(
                "Denetim: {audited} satırdan {mismatches} tanesi farklı. Toplam fark (Rust − uygulama): {mismatch_total}"
            );
        }
    }
    Ok(())
}

fn print_report(report: &Report, rows: &[&InputRow]) {
    let has_app = rows.iter().any(|r| r.app_commission.is_some());
    let rule = "─".repeat(if has_app { 64 } else { 48 });

    print!(
        "{:>4}  {:<8} {:>14} {:>7} {:>12}",
        "#", "Kategori", "Tutar", "Oran", "Komisyon"
    );
    if has_app {
        print!(" {:>15}", "Uygulama");
    }
    println!("\n{rule}");
    for (row, line) in rows.iter().zip(&report.lines) {
        print!(
            "{:>4}  {:<8} {:>14} {:>7} {:>12}",
            row.source_line, line.sale.category, line.sale.amount, line.rate, line.commission
        );
        if let Some(app) = row.app_commission {
            if app == line.commission {
                print!(" {:>15}", "✓");
            } else {
                print!(" {:>13} ✗", app.to_string());
            }
        }
        println!();
    }
    println!("{rule}");
    for c in report.by_category().iter().filter(|c| c.count > 0) {
        println!(
            "      {:<8} {:>14} {:>7} {:>12}",
            c.category, c.sales, "", c.commission
        );
    }
    println!("{rule}");
    println!(
        "      {:<8} {:>14} {:>7} {:>12}",
        "TOPLAM", report.total_sales, "", report.total_commission
    );
    match report.target {
        None => println!("Hedef: ayarlanmadı"),
        Some(t) if report.target_met => println!("Hedef: {t} → TUTTU ✓"),
        Some(t) => println!("Hedef: {t} → tutmadı (eksik: {})", t - report.total_sales),
    }
}

// ─── CSV ─────────────────────────────────────────────────────────────

fn read_csv(text: &str) -> Result<Input, String> {
    let mut rows = Vec::new();
    for (n, raw) in text.lines().enumerate() {
        let line_no = n + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (cat, amount) = line
            .split_once(';')
            .or_else(|| line.split_once('\t'))
            .ok_or_else(|| format!("satır {line_no}: ';' veya TAB yok: {line:?}"))?;
        // Başlık satırını atla.
        if rows.is_empty() && cat.trim().eq_ignore_ascii_case("category") {
            continue;
        }
        let category: Category = cat.parse().map_err(|e| format!("satır {line_no}: {e}"))?;
        let amount = Money::parse(amount).map_err(|e| format!("satır {line_no}: {e}"))?;
        rows.push(InputRow {
            source_line: line_no,
            person: None,
            sale: Sale { category, amount },
            app_commission: None,
        });
    }
    Ok(Input {
        rows,
        target_in_file: None,
    })
}

// ─── Excel (CommissionApp raporu) ────────────────────────────────────

/// Excel hücresini metne çevirir (başlık karşılaştırması için).
fn cell_text(cell: &Data) -> String {
    match cell {
        Data::String(s) => s.trim().to_string(),
        Data::Empty => String::new(),
        other => other.to_string(),
    }
}

/// Excel hücresini paraya çevirir.
///
/// Excel sayıları `f64` olarak saklar. Uygulama tutarları 2 ondalık ile yazar,
/// bu yüzden `× 100` ve en yakın tam sayıya yuvarlama güvenlidir.
fn cell_money(cell: &Data) -> Option<Money> {
    match cell {
        Data::Float(f) if f.is_finite() => Some(Money::from_cents((f * 100.0).round() as i64)),
        Data::Int(i) => i.checked_mul(100).map(Money::from_cents),
        Data::String(s) => Money::parse(s).ok(),
        _ => None,
    }
}

fn read_report(path: &str) -> Result<Input, String> {
    let mut workbook = open_workbook_auto(path).map_err(|e| format!("{path}: {e}"))?;
    let sheet = workbook
        .sheet_names()
        .first()
        .cloned()
        .ok_or("çalışma kitabında sayfa yok")?;
    let range = workbook
        .worksheet_range(&sheet)
        .map_err(|e| format!("{sheet}: {e}"))?;
    let grid: Vec<Vec<Data>> = range.rows().map(<[Data]>::to_vec).collect();

    // Özet bölümündeki hedef: ["Target Amount (€)", 31000]
    let target_in_file = grid.iter().find_map(|row| {
        let label = cell_text(row.first()?).to_lowercase();
        if label.starts_with("target amount") {
            cell_money(row.get(1)?).filter(|m| m.is_positive())
        } else {
            None
        }
    });

    // Detay tablosunun başlığı: hem "Category" hem "Amount" sütunu olan satır.
    let find = |headers: &[String], name: &str| headers.iter().position(|h| h.starts_with(name));
    let (header_idx, headers) = grid
        .iter()
        .enumerate()
        .map(|(i, row)| {
            (
                i,
                row.iter()
                    .map(|c| cell_text(c).to_lowercase())
                    .collect::<Vec<_>>(),
            )
        })
        .find(|(_, h)| {
            h.iter().any(|c| c == "category") && h.iter().any(|c| c.starts_with("amount"))
        })
        .ok_or("rapor başlığı bulunamadı (Category + Amount sütunları gerekli)")?;

    let col_category = find(&headers, "category").expect("başlıkta var");
    let col_amount = find(&headers, "amount").expect("başlıkta var");
    let col_commission = find(&headers, "commission");
    let col_person = find(&headers, "person");

    let mut rows = Vec::new();
    for (offset, row) in grid.iter().enumerate().skip(header_idx + 1) {
        let excel_row = offset + 1;
        let cat_text = row.get(col_category).map(cell_text).unwrap_or_default();
        if cat_text.is_empty() {
            break; // tablo sonu (TOTAL satırı veya boş satır)
        }
        let category: Category = cat_text
            .parse()
            .map_err(|e| format!("Excel satır {excel_row}: {e}"))?;
        let amount = row
            .get(col_amount)
            .and_then(cell_money)
            .ok_or_else(|| format!("Excel satır {excel_row}: tutar okunamadı"))?;
        rows.push(InputRow {
            source_line: excel_row,
            person: col_person
                .and_then(|c| row.get(c))
                .map(cell_text)
                .filter(|s| !s.is_empty()),
            sale: Sale { category, amount },
            app_commission: col_commission.and_then(|c| row.get(c)).and_then(cell_money),
        });
    }
    if rows.is_empty() {
        return Err("raporda satış satırı yok".into());
    }
    Ok(Input {
        rows,
        target_in_file,
    })
}
