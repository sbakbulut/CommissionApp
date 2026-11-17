//! Writing .xlsx files (replaces SheetJS `aoa_to_sheet` + `writeFile`).
//!
//! Same cells, column widths and merged title rows as the old app. The old
//! code also asked for header colours, but the free SheetJS version ignored
//! them; here they are applied.

use rust_xlsxwriter::{Color, Format, FormatAlign, Workbook, Worksheet, XlsxError};

use crate::report::{Out, SheetData};

/// Colour scheme of a report.
#[derive(Clone, Copy)]
pub enum Theme {
    /// "Excel (My Sales)"
    Single,
    /// Team reports
    Team,
}

const HEADINGS: [&str; 7] = [
    "COMMISSION CALCULATOR — SALES REPORT",
    "COMMISSION REPORT",
    "TEAM COMMISSION SUMMARY",
    "ALL SALES DETAIL",
    "RATES",
    "DAILY SUMMARY",
    "SALES DETAIL",
];

pub struct SheetSpec<'a> {
    pub name: &'a str,
    pub data: &'a SheetData,
    pub widths: &'a [f64],
    pub theme: Theme,
}

fn add_sheet(wb: &mut Workbook, spec: &SheetSpec) -> Result<(), XlsxError> {
    let ws: &mut Worksheet = wb.add_worksheet();
    ws.set_name(spec.name)?;
    for (c, w) in spec.widths.iter().enumerate() {
        ws.set_column_width(c as u16, *w)?;
    }
    let (title_bg, head_bg, total_bg) = match spec.theme {
        Theme::Single => (0x5B21B6, 0x10B981, 0xF59E0B),
        Theme::Team => (0x0ABAB5, 0x088A85, 0xC9A961),
    };
    let white_bold = Format::new().set_bold().set_font_color(Color::White);
    let title = white_bold
        .clone()
        .set_font_size(12)
        .set_background_color(Color::RGB(title_bg))
        .set_align(FormatAlign::Left);
    let head = white_bold
        .clone()
        .set_background_color(Color::RGB(head_bg))
        .set_align(FormatAlign::Center);
    let total = white_bold.set_background_color(Color::RGB(total_bg));

    let last_col = spec
        .data
        .iter()
        .map(Vec::len)
        .max()
        .unwrap_or(0)
        .saturating_sub(1) as u16;
    for (r, row) in spec.data.iter().enumerate() {
        let r = r as u32;
        let first = match row.first() {
            Some(Out::S(s)) => s.as_str(),
            _ => "",
        };
        let style = if first == "TOTAL" {
            Some(&total)
        } else if ["Category", "Date", "#"].contains(&first) {
            Some(&head)
        } else {
            None
        };
        if HEADINGS.contains(&first) {
            if last_col > 0 {
                ws.merge_range(r, 0, r, last_col, first, &title)?;
            } else {
                ws.write_string_with_format(r, 0, first, &title)?;
            }
            continue;
        }
        for (c, cell) in row.iter().enumerate() {
            let c = c as u16;
            match (cell, style) {
                (Out::S(s), Some(f)) => ws.write_string_with_format(r, c, s, f)?,
                (Out::S(s), None) => ws.write_string(r, c, s)?,
                (Out::N(v), Some(f)) => ws.write_number_with_format(r, c, *v, f)?,
                (Out::N(v), None) => ws.write_number(r, c, *v)?,
            };
        }
    }
    Ok(())
}

/// Builds an .xlsx file in memory.
pub fn workbook_bytes(sheets: &[SheetSpec]) -> Result<Vec<u8>, String> {
    let mut wb = Workbook::new();
    for spec in sheets {
        add_sheet(&mut wb, spec).map_err(|e| e.to_string())?;
    }
    wb.save_to_buffer().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::{Cell, read_workbook};

    #[test]
    fn written_file_reads_back() {
        let data: SheetData = vec![
            vec![Out::S("RATES".into())],
            vec![],
            vec![Out::S("Category".into()), Out::S("Rate (%)".into())],
            vec![Out::S("Icon".into()), Out::N(1.5)],
            vec![Out::S("TOTAL".into()), Out::S("".into())],
        ];
        let bytes = workbook_bytes(&[SheetSpec {
            name: "Commission Report",
            data: &data,
            widths: &[6.0, 12.0],
            theme: Theme::Single,
        }])
        .unwrap();
        let wb = read_workbook(&bytes).unwrap();
        assert_eq!(wb.sheets[0].name, "Commission Report");
        assert_eq!(
            wb.sheets[0].rows[3],
            vec![Cell::Str("Icon".into()), Cell::Num(1.5)]
        );
        assert_eq!(wb.sheets[0].rows[0][0], Cell::Str("RATES".into()));
    }
}
