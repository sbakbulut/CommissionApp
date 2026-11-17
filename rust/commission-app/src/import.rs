//! Reading sales files: Excel (.xlsx / .xls / .ods) and HTML exports saved as .xls.
//!
//! Port of `importExcelLocal()`, `readSheetItems()`, `parseTeamWorkbook()`,
//! `parseHTMLTable()`, `findHeaderRow()`, `findCol()`, `findPersonCol()`,
//! `parseDate()` and `parseAmount()` from the old JavaScript app.

use std::io::Cursor;
use std::sync::LazyLock;

use calamine::{Data, Reader, open_workbook_auto_from_rs};
use commission_core::Category;
use regex::Regex;

use crate::js;
use crate::tiffany;

/// One cell, as SheetJS `sheet_to_json(..., {header: 1, defval: ''})` gave it.
#[derive(Debug, Clone, PartialEq)]
pub enum Cell {
    Empty,
    Str(String),
    Num(f64),
    Bool(bool),
}

impl Cell {
    /// `c === '' || c == null`
    fn is_blank(&self) -> bool {
        matches!(self, Cell::Empty) || matches!(self, Cell::Str(s) if s.is_empty())
    }

    /// `String(c || '')`
    pub fn text(&self) -> String {
        match self {
            Cell::Empty => String::new(),
            Cell::Str(s) => s.clone(),
            Cell::Num(n) if *n == 0.0 || n.is_nan() => String::new(),
            Cell::Num(n) => js::num_to_string(*n),
            Cell::Bool(false) => String::new(),
            Cell::Bool(true) => "true".into(),
        }
    }

    /// `String(c)` (used in error messages: `Invalid amount: ${value}`)
    pub fn display(&self) -> String {
        match self {
            Cell::Empty => String::new(),
            Cell::Str(s) => s.clone(),
            Cell::Num(n) => js::num_to_string(*n),
            Cell::Bool(b) => b.to_string(),
        }
    }
}

pub type Grid = Vec<Vec<Cell>>;

/// A sheet: name + rows.
pub struct Sheet {
    pub name: String,
    pub rows: Grid,
}

pub struct Workbook {
    pub sheets: Vec<Sheet>,
    /// `true` when the file was an HTML table (ZeuPortal ".xls").
    pub is_html: bool,
}

// ─── File → sheets ───────────────────────────────────────────────────

/// `TextDecoder('utf-8').decode(...)` + the old HTML check.
fn looks_like_html(bytes: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(bytes);
    let start: String = text
        .trim_start_matches(js::is_js_space)
        .chars()
        .take(16)
        .collect::<String>()
        .to_ascii_lowercase();
    let start = start.strip_prefix("\u{EF}\u{BB}\u{BF}").unwrap_or(&start);
    ["<?xml", "<!doctype", "<html", "<table"]
        .iter()
        .any(|p| start.starts_with(p))
        .then(|| text.into_owned())
}

/// Reads all sheets of a file.
pub fn read_workbook(bytes: &[u8]) -> Result<Workbook, String> {
    if let Some(html) = looks_like_html(bytes) {
        return Ok(Workbook {
            sheets: vec![Sheet {
                name: "HTML".into(),
                rows: parse_html_table(&html),
            }],
            is_html: true,
        });
    }
    let mut wb =
        open_workbook_auto_from_rs(Cursor::new(bytes.to_vec())).map_err(|e| e.to_string())?;
    let mut sheets = Vec::new();
    for name in wb.sheet_names() {
        let range = wb.worksheet_range(&name).map_err(|e| e.to_string())?;
        sheets.push(Sheet {
            name,
            rows: range_to_grid(&range),
        });
    }
    Ok(Workbook {
        sheets,
        is_html: false,
    })
}

/// calamine range → rows from A1 (SheetJS starts at the sheet's `!ref`, normally A1).
fn range_to_grid(range: &calamine::Range<Data>) -> Grid {
    let Some((r0, c0)) = range.start() else {
        return Vec::new();
    };
    let width = c0 as usize + range.width();
    let mut grid: Grid = vec![vec![Cell::Empty; width]; r0 as usize];
    for row in range.rows() {
        let mut out = vec![Cell::Empty; c0 as usize];
        out.extend(row.iter().map(|d| match d {
            Data::Empty => Cell::Empty,
            Data::String(s) => Cell::Str(s.clone()),
            Data::Float(f) => Cell::Num(*f),
            Data::Int(i) => Cell::Num(*i as f64),
            Data::Bool(b) => Cell::Bool(*b),
            // cellDates: false → dates are serial numbers
            Data::DateTime(dt) => Cell::Num(dt.as_f64()),
            Data::DateTimeIso(s) | Data::DurationIso(s) => Cell::Str(s.clone()),
            Data::Error(e) => Cell::Str(format!("#{e:?}")),
        }));
        grid.push(out);
    }
    grid
}

static TABLE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)<table[^>]*>(.*?)</table>").unwrap());
static TR_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)<tr[^>]*>(.*?)</tr>").unwrap());
static TD_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)<t[hd][^>]*>(.*?)</t[hd]>").unwrap());
static TAG_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").unwrap());

/// Case-insensitive search (JavaScript `/…/i`): match on an ASCII lower-case copy
/// (same byte positions), then take the text from the original.
fn captures_ci<'h>(re: &Regex, text: &'h str) -> Vec<&'h str> {
    let lower = text.to_ascii_lowercase();
    re.captures_iter(&lower)
        .filter_map(|c| c.get(1))
        .map(|m| &text[m.range()])
        .collect()
}

/// The old `parseHTMLTable(html)`: cell text exactly as written (dates are not changed).
pub fn parse_html_table(html: &str) -> Grid {
    let html = html.strip_prefix('\u{FEFF}').unwrap_or(html);
    let Some(table) = captures_ci(&TABLE_RE, html).into_iter().next() else {
        return Vec::new();
    };
    let mut rows = Vec::new();
    for tr in captures_ci(&TR_RE, table) {
        let row: Vec<Cell> = captures_ci(&TD_RE, tr)
            .into_iter()
            .map(|td| {
                let text = TAG_RE
                    .replace_all(td, "")
                    .replace("&nbsp;", " ")
                    .replace("&amp;", "&")
                    .replace("&lt;", "<")
                    .replace("&gt;", ">")
                    .replace("&quot;", "\"")
                    .replace("&#39;", "'");
                Cell::Str(js::trim(&text).to_string())
            })
            .collect();
        if !row.is_empty() {
            rows.push(row);
        }
    }
    rows
}

// ─── Columns ─────────────────────────────────────────────────────────

const DATE_PATS: &[&str] = &[
    "belge tarihi",
    "tarih",
    "date",
    "invoice date",
    "işlem tarihi",
    "islem tarihi",
];
const CODE_PATS: &[&str] = &[
    "urun",
    "ürün",
    "product",
    "description",
    "kod açıklama",
    "urun adi",
    "ürün adı",
    "urun adı",
    "ürün adi",
    "malzeme açık",
    "malzeme acik",
];
const SKU_PATS: &[&str] = &[
    "malzeme no",
    "malzeme_no",
    "material no",
    "material number",
    "sku",
    "kod no",
    "kod",
    "code",
    "barkod",
    "barcode",
];
const SKU_EXCLUDE: &[&str] = &[
    "magaza",
    "mağaza",
    "açıklama",
    "aciklama",
    "fatura",
    "fiş",
    "fis",
];
const AMOUNT_PATS: &[&str] = &[
    "tutar",
    "toplam",
    "hasilat",
    "satis tutar",
    "satış tutar",
    "net tutar",
    "brüt tutar",
    "brut tutar",
    "amount",
    "total",
    "fiyat",
];
const PERSON_PATTERNS: &[&str] = &[
    "satisi yapan",
    "satışı yapan",
    "satis yapan",
    "salesperson",
    "satici",
    "satıcı",
    "seller",
    "kasiyer",
    "cashier",
];

fn norm(c: &Cell) -> String {
    js::trim(&c.text()).to_lowercase()
}

/// The old `findCol(headers, patterns, excludeWords)` (3 passes). `headers` are already normalized.
pub fn find_col(headers: &[String], patterns: &[&str], exclude: &[&str]) -> Option<usize> {
    let skip = |h: &str| h.is_empty() || exclude.iter().any(|w| h.contains(w));
    let heads: Vec<String> = headers.iter().map(|h| js::trim(h).to_lowercase()).collect();
    for (i, h) in heads.iter().enumerate() {
        if !skip(h) && patterns.iter().take(3).any(|p| h == p) {
            return Some(i);
        }
    }
    for (i, h) in heads.iter().enumerate() {
        if !skip(h) && patterns.contains(&h.as_str()) {
            return Some(i);
        }
    }
    for (i, h) in heads.iter().enumerate() {
        if !skip(h) && patterns.iter().any(|p| h.contains(p)) {
            return Some(i);
        }
    }
    None
}

static PERSON_SKIP: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"kimlik|(?-u:\b)no(?-u:\b)|kod|id").unwrap());

/// The old `findPersonCol(headers)`.
pub fn find_person_col(headers: &[String]) -> Option<usize> {
    let heads: Vec<String> = headers.iter().map(|h| js::trim(h).to_lowercase()).collect();
    if let Some(i) = heads
        .iter()
        .position(|h| PERSON_PATTERNS.contains(&h.as_str()))
    {
        return Some(i);
    }
    heads.iter().position(|h| {
        !h.is_empty() && !PERSON_SKIP.is_match(h) && PERSON_PATTERNS.iter().any(|p| h.contains(p))
    })
}

/// The old `findHeaderRow(rows)`: skips summary rows above the real header.
pub fn find_header_row(rows: &Grid) -> usize {
    let code_pats = CODE_PATS;
    let amount_pats: &[&str] = &[
        "tutar",
        "amount",
        "total",
        "fiyat",
        "toplam",
        "hasilat",
        "satis tutar",
        "satış tutar",
        "net tutar",
        "brüt tutar",
        "brut tutar",
    ];
    let sku_pats: &[&str] = &["malzeme", "material", "sku", "barkod", "barcode"];
    let mut best: Option<usize> = None;
    let mut best_score = 0;
    for (i, r) in rows.iter().enumerate().take(20) {
        if r.iter().all(Cell::is_blank) {
            continue;
        }
        let headers: Vec<String> = r.iter().map(norm).filter(|h| !h.is_empty()).collect();
        if headers.len() < 3 {
            continue;
        }
        let has = |pats: &[&str]| pats.iter().any(|p| headers.iter().any(|h| h.contains(p)));
        let score = [DATE_PATS, code_pats, amount_pats, PERSON_PATTERNS, sku_pats]
            .iter()
            .filter(|p| has(p))
            .count();
        if score > best_score {
            best_score = score;
            best = Some(i);
        }
        if score >= 3 {
            return i;
        }
    }
    if best_score >= 2 {
        best.unwrap_or(0)
    } else {
        0
    }
}

// ─── Values ──────────────────────────────────────────────────────────

/// The old `parseAmount(value)`: euros as a JavaScript number (0 when not a number).
pub fn parse_amount(value: &Cell) -> f64 {
    let s = match value {
        Cell::Num(n) => return *n,
        Cell::Empty => return 0.0,
        other => other.display(),
    };
    let s = js::trim(&s);
    if s.is_empty() || s == "-" {
        return 0.0;
    }
    let mut s: String = s
        .chars()
        .filter(|c| !matches!(c, '€' | '$' | '£' | '¥' | '₺') && !js::is_js_space(*c))
        .collect();
    if s.is_empty() {
        return 0.0;
    }
    if s.contains(',') && s.contains('.') {
        if s.rfind(',') > s.rfind('.') {
            s = s.replace('.', "").replacen(',', ".", 1);
        } else {
            s = s.replace(',', "");
        }
    } else if s.contains(',') {
        let parts: Vec<&str> = s.split(',').collect();
        s = if parts.len() == 2 && js::len16(parts[1]) == 3 {
            s.replacen(',', "", 1)
        } else {
            s.replacen(',', ".", 1)
        };
    }
    match js::parse_float(&s) {
        Some(v) if v != 0.0 && !v.is_nan() => v,
        _ => 0.0,
    }
}

static ISO_DATE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([0-9]{4})-([0-9]{1,2})-([0-9]{1,2})").unwrap());
static TR_DATE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([0-9]{1,2})[/\-.]([0-9]{1,2})[/\-.]([0-9]{2,4})").unwrap());

fn pad2(s: &str) -> String {
    format!("{s:0>2}")
}

/// The old `parseDate(value)`: `YYYY-MM-DD`, or empty when not a date.
pub fn parse_date(value: &Cell) -> String {
    let s = match value {
        Cell::Empty => return String::new(),
        Cell::Str(s) if s.is_empty() => return String::new(),
        Cell::Num(n) if *n > 1000.0 => return js::excel_serial_to_iso(*n),
        other => other.display(),
    };
    let s = js::trim(&s);
    if let Some(m) = ISO_DATE.captures(s) {
        return format!("{}-{}-{}", &m[1], pad2(&m[2]), pad2(&m[3]));
    }
    if let Some(m) = TR_DATE.captures(s) {
        let y = if m[3].len() == 2 {
            format!("20{}", &m[3])
        } else {
            m[3].to_string()
        };
        return format!("{y}-{}-{}", pad2(&m[2]), pad2(&m[1]));
    }
    String::new()
}

// ─── Rows → sales ────────────────────────────────────────────────────

/// One imported sale.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub sku: String,
    pub code: String,
    pub date: String,
    /// Euros, as the old app kept it (a JavaScript number).
    pub amount: f64,
    pub collection: String,
    pub material: String,
    pub category: Category,
    pub person: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportError {
    pub row: usize,
    pub reason: String,
}

struct Cols {
    date: Option<usize>,
    code: Option<usize>,
    sku: Option<usize>,
    amount: Option<usize>,
    person: Option<usize>,
}

fn detect_cols(rows: &Grid, header_idx: usize) -> Cols {
    let headers: Vec<String> = rows
        .get(header_idx)
        .map(|r| r.iter().map(norm).collect())
        .unwrap_or_default();
    Cols {
        date: find_col(&headers, DATE_PATS, &[]),
        code: find_col(&headers, CODE_PATS, &[]),
        sku: find_col(&headers, SKU_PATS, SKU_EXCLUDE),
        amount: find_col(&headers, AMOUNT_PATS, &[]),
        person: find_person_col(&headers),
    }
}

fn cell(row: &[Cell], col: Option<usize>) -> &Cell {
    static EMPTY: Cell = Cell::Empty;
    col.and_then(|c| row.get(c)).unwrap_or(&EMPTY)
}

fn make_item(row: &[Cell], cols: &Cols, code: String, amount: f64, date: String) -> Item {
    let sku = js::trim(&cell(row, cols.sku).text()).to_string();
    let person = js::trim(&cell(row, cols.person).text()).to_string();
    let (category, parsed) = tiffany::categorize(&code, &sku);
    Item {
        sku,
        code,
        date,
        amount,
        collection: parsed.collection,
        material: parsed.material,
        category,
        person,
    }
}

/// The most common value (first one wins a tie), like the old `Object.keys(nc).sort(...)[0]`.
fn most_common(values: impl Iterator<Item = String>) -> Option<String> {
    let mut counts: Vec<(String, usize)> = Vec::new();
    for v in values {
        match counts.iter_mut().find(|(k, _)| *k == v) {
            Some((_, n)) => *n += 1,
            None => counts.push((v, 1)),
        }
    }
    let max = counts.iter().map(|(_, n)| *n).max()?;
    counts.into_iter().find(|(_, n)| *n == max).map(|(k, _)| k)
}

/// Result of "Import Excel" (my sales).
pub struct SingleImport {
    pub items: Vec<Item>,
    pub errors: Vec<ImportError>,
    pub header_row: usize,
    /// New salesperson name (already upper case), if the file has one.
    pub salesperson: Option<String>,
}

static SHEET_NAME_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Za-zÇĞİÖŞÜçğıöşü .'-]{4,}$").unwrap());

/// The old `importExcelLocal(file)` without the screen parts.
///
/// Fix: the old code read the sheet name from a variable that did not exist
/// when the file had no salesperson column, so such imports failed.
pub fn import_single(wb: &Workbook) -> Result<SingleImport, String> {
    let sheet = wb.sheets.first();
    let rows: &Grid = match sheet {
        Some(s) => &s.rows,
        None => return Err("Excel is empty or has only a header row".into()),
    };
    if rows.len() < 2 {
        return Err("Excel is empty or has only a header row".into());
    }
    let header_row = find_header_row(rows);
    let cols = detect_cols(rows, header_row);

    let salesperson = if cols.person.is_some() {
        most_common(
            rows.iter()
                .skip(header_row + 1)
                .map(|r| js::trim(&cell(r, cols.person).text()).to_string())
                .filter(|v| !v.is_empty()),
        )
        .map(|n| n.to_uppercase())
    } else {
        sheet
            .filter(|s| !wb.is_html && SHEET_NAME_RE.is_match(&s.name))
            .map(|s| js::trim(&s.name).to_uppercase())
    };

    if cols.date.is_none() {
        return Err("Date column not found (Date / Invoice Date)".into());
    }
    if cols.code.is_none() {
        return Err("Product code column not found (Product / Description)".into());
    }
    if cols.amount.is_none() {
        return Err("Amount column not found (Amount / Total / Price)".into());
    }

    let mut items = Vec::new();
    let mut errors = Vec::new();
    for (i, row) in rows.iter().enumerate().skip(header_row + 1) {
        if row.iter().all(Cell::is_blank) {
            continue;
        }
        let code = js::trim(&cell(row, cols.code).text()).to_string();
        if code.is_empty() {
            errors.push(ImportError {
                row: i + 1,
                reason: "Product code is empty".into(),
            });
            continue;
        }
        let amount_cell = cell(row, cols.amount);
        let amount = parse_amount(amount_cell);
        if amount.is_nan() || amount <= 0.0 {
            errors.push(ImportError {
                row: i + 1,
                reason: format!("Invalid amount: {}", amount_cell.display()),
            });
            continue;
        }
        let date_cell = cell(row, cols.date);
        let date = parse_date(date_cell);
        if date.is_empty() {
            errors.push(ImportError {
                row: i + 1,
                reason: format!("Invalid date: {}", date_cell.display()),
            });
            continue;
        }
        items.push(make_item(row, &cols, code, amount, date));
    }
    items.sort_by(|a, b| js::cmp_str(&a.date, &b.date));
    Ok(SingleImport {
        items,
        errors,
        header_row,
        salesperson,
    })
}

/// The old `readSheetItems(rows)`: (items, most common salesperson in the sheet).
pub fn read_sheet_items(rows: &Grid) -> (Vec<Item>, String) {
    if rows.len() < 2 {
        return (Vec::new(), String::new());
    }
    let header_row = find_header_row(rows);
    let cols = detect_cols(rows, header_row);
    if cols.date.is_none() || cols.code.is_none() || cols.amount.is_none() {
        return (Vec::new(), String::new());
    }
    let mut items = Vec::new();
    for row in rows.iter().skip(header_row + 1) {
        if row.iter().all(Cell::is_blank) {
            continue;
        }
        let code = js::trim(&cell(row, cols.code).text()).to_string();
        let amount = parse_amount(cell(row, cols.amount));
        let date = parse_date(cell(row, cols.date));
        if code.is_empty() || amount.is_nan() || amount <= 0.0 || date.is_empty() {
            continue;
        }
        items.push(make_item(row, &cols, code, amount, date));
    }
    let name = most_common(
        items
            .iter()
            .map(|it| it.person.clone())
            .filter(|p| !p.is_empty()),
    )
    .unwrap_or_default();
    (items, name)
}

/// The old `parseTeamWorkbook()`: sales grouped by salesperson (names sorted).
pub fn import_team(wb: &Workbook) -> Vec<(String, Vec<Item>)> {
    let mut by_person: Vec<(String, Vec<Item>)> = Vec::new();
    for sheet in &wb.sheets {
        let (items, name_from_col) = read_sheet_items(&sheet.rows);
        for it in items {
            let raw = [&it.person, &name_from_col, &sheet.name]
                .into_iter()
                .find(|s| !s.is_empty())
                .map_or("Unknown", |s| s.as_str());
            let name = js::trim(raw).to_uppercase();
            match by_person.iter_mut().find(|(n, _)| *n == name) {
                Some((_, list)) => list.push(it),
                None => by_person.push((name, vec![it])),
            }
        }
    }
    by_person.sort_by_key(|p| sort_key16(&p.0));
    by_person
}

/// JavaScript default sort compares UTF-16 code units.
fn sort_key16(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// The old `futureDateCount(items)`.
pub fn future_date_count<'a>(dates: impl Iterator<Item = &'a str>, today: &str) -> usize {
    dates.filter(|d| !d.is_empty() && *d > today).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &str) -> Cell {
        Cell::Str(v.into())
    }

    #[test]
    fn amounts() {
        assert_eq!(parse_amount(&s("1.234,56")), 1234.56);
        assert_eq!(parse_amount(&s("1,234.56")), 1234.56);
        assert_eq!(parse_amount(&s("1,234")), 1234.0);
        assert_eq!(parse_amount(&s("12,5")), 12.5);
        assert_eq!(parse_amount(&s("€ 31.000,00")), 31000.0);
        assert_eq!(parse_amount(&s("abc")), 0.0);
        assert_eq!(parse_amount(&Cell::Num(99.5)), 99.5);
        assert_eq!(parse_amount(&Cell::Bool(true)), 0.0);
    }

    #[test]
    fn dates() {
        assert_eq!(parse_date(&s("31.08.2026")), "2026-08-31");
        assert_eq!(parse_date(&s("1/2/26")), "2026-02-01");
        assert_eq!(parse_date(&s("2026-8-1 10:00")), "2026-08-01");
        assert_eq!(parse_date(&Cell::Num(46310.0)), "2026-10-15");
        assert_eq!(parse_date(&Cell::Num(45.0)), "");
        assert_eq!(parse_date(&s("x")), "");
    }

    #[test]
    fn html_table() {
        let g = parse_html_table(
            "\u{FEFF}<html><table><tr><th>Tarih</th><th>Ürün</th></tr><tr><td> 31.08.2026 </td><td><span>HW &amp; KNOT</span></td></tr></table>",
        );
        assert_eq!(
            g,
            vec![
                vec![s("Tarih"), s("Ürün")],
                vec![s("31.08.2026"), s("HW & KNOT")]
            ]
        );
    }

    #[test]
    fn header_detection_skips_summary() {
        let rows: Grid = vec![
            vec![s("Ad Soyad"), s("Hasilat"), s("Adet")],
            vec![s("ALİ"), Cell::Num(5.0), Cell::Num(1.0)],
            vec![],
            vec![
                s("Belge Tarihi"),
                s("Malzeme No"),
                s("Ürün Adı"),
                s("Tutar"),
                s("Satışı Yapan"),
            ],
            vec![
                s("15.10.2026"),
                s("60153062"),
                s("HW LINK"),
                s("1.250,00"),
                s("Ali Veli"),
            ],
        ];
        assert_eq!(find_header_row(&rows), 3);
        let wb = Workbook {
            sheets: vec![Sheet {
                name: "Sheet1".into(),
                rows,
            }],
            is_html: false,
        };
        let r = import_single(&wb).unwrap();
        assert_eq!(r.salesperson.as_deref(), Some("ALI VELI"));
        assert_eq!(r.items.len(), 1);
        assert_eq!(r.items[0].amount, 1250.0);
        assert_eq!(r.items[0].category, Category::Icon);
        assert_eq!(r.items[0].sku, "60153062");
    }

    #[test]
    fn import_without_person_column_uses_sheet_name() {
        let rows: Grid = vec![
            vec![s("Date"), s("Product"), s("Amount")],
            vec![s("2026-10-15"), s("SS RTT TAG"), Cell::Num(300.0)],
        ];
        let wb = Workbook {
            sheets: vec![Sheet {
                name: "Ayşe Yılmaz".into(),
                rows,
            }],
            is_html: false,
        };
        let r = import_single(&wb).unwrap();
        assert_eq!(r.salesperson.as_deref(), Some("AYŞE YILMAZ"));
    }
}
