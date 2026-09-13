use nu_plugin::{EngineInterface, EvaluatedCall, PluginCommand};
use nu_protocol::{Category, Config, LabeledError, PipelineData, Signature, Type, Value};
use rust_xlsxwriter::{Format, Table, Workbook, Worksheet, XlsxError};

use crate::XlsxPlugin;

pub struct ToXlsx;

#[allow(clippy::unnecessary_literal_bound)]
impl PluginCommand for ToXlsx {
    type Plugin = XlsxPlugin;

    fn name(&self) -> &str {
        "to xlsx"
    }

    fn description(&self) -> &str {
        "Convert table data to Excel (.xlsx) format"
    }

    fn extra_description(&self) -> &str {
        "Output is an Excel Table with auto-filter, banded rows, and autofit by default. \
         Use --raw for plain cells.\n\n\
         Note: `save` invokes `to xlsx` automatically based on the file extension, \
         so `ls | save files.xlsx` works directly. \
         `ls | to xlsx | save files.xlsx` also works: `save` writes binary as it is."
    }

    fn signature(&self) -> Signature {
        Signature::build("to xlsx")
            .input_output_type(Type::table(), Type::Binary)
            .input_output_type(Type::record(), Type::Binary)
            .switch(
                "raw",
                "Write plain cells instead of an Excel Table",
                Some('r'),
            )
            .category(Category::Formats)
    }

    fn search_terms(&self) -> Vec<&str> {
        vec!["excel", "spreadsheet", "xlsx", "workbook"]
    }

    fn examples(&self) -> Vec<nu_protocol::Example<'_>> {
        vec![
            nu_protocol::Example {
                description: "Save a table to an xlsx file",
                example: "ls | save files.xlsx",
                result: None,
            },
            nu_protocol::Example {
                description: "Explicit conversion to xlsx binary",
                example: "ls | to xlsx",
                result: None,
            },
            nu_protocol::Example {
                description: "Multi-sheet workbook",
                example: "{ Users: [[name age]; [Alice 30]], Orders: [[item qty]; [Widget 5]] } | save report.xlsx",
                result: None,
            },
            nu_protocol::Example {
                description: "Plain cells without Excel Table formatting",
                example: "ls | to xlsx --raw | save files.xlsx",
                result: None,
            },
        ]
    }

    fn run(
        &self,
        _plugin: &XlsxPlugin,
        _engine: &EngineInterface,
        call: &EvaluatedCall,
        input: PipelineData,
    ) -> Result<PipelineData, LabeledError> {
        let span = call.head;
        let raw = call.has_flag("raw")?;
        let input = input.into_value(span)?;

        let sheets = match &input {
            Value::List { .. } => {
                vec![("Sheet1".to_string(), input)]
            }
            Value::Record { val, .. } => val.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
            other => {
                return Err(LabeledError::new("Expected table or record of tables")
                    .with_label(format!("got {}", other.get_type()), other.span()));
            }
        };

        let bytes = write_workbook(&sheets, raw, span)?;
        Ok(PipelineData::Value(Value::binary(bytes, span), None))
    }
}

/// How a date cell is rendered. `autofit` cannot see this, hence [`date_column_width`].
const DATE_NUM_FORMAT: &str = "yyyy-mm-dd hh:mm:ss";

/// Width a date column needs, derived from the format so the two cannot drift.
fn date_column_width() -> f64 {
    #[allow(clippy::cast_precision_loss)]
    let chars = DATE_NUM_FORMAT.len() as f64;
    chars + 1.0
}

pub(crate) fn write_workbook(
    sheets: &[(String, Value)],
    raw: bool,
    span: nu_protocol::Span,
) -> Result<Vec<u8>, LabeledError> {
    reject_duplicate_sheet_names(sheets, span)?;

    let date_format = Format::new().set_num_format(DATE_NUM_FORMAT);
    let mut workbook = Workbook::new();

    for (sheet_name, table_value) in sheets {
        let records = match table_value {
            Value::List { vals, .. } => vals,
            other => {
                return Err(LabeledError::new("Expected a table (list of records)")
                    .with_label(format!("got {}", other.get_type()), other.span()));
            }
        };

        if records.is_empty() {
            name_sheet(workbook.add_worksheet(), sheet_name, span)?;
            continue;
        }

        let columns: Vec<String> = match &records[0] {
            Value::Record { val, .. } => val.columns().map(String::from).collect(),
            other => {
                return Err(LabeledError::new("Expected records in table")
                    .with_label(format!("got {}", other.get_type()), other.span()));
            }
        };

        let num_cols = u16::try_from(columns.len()).map_err(|_| {
            LabeledError::new("Too many columns").with_label("exceeds u16 range", span)
        })?;

        let worksheet = workbook.add_worksheet();
        name_sheet(worksheet, sheet_name, span)?;

        // Column count validated above via num_cols: u16
        #[allow(clippy::cast_possible_truncation)]
        for (col, header) in columns.iter().enumerate() {
            worksheet.write_string(0, col as u16, header).map_err(|e| {
                LabeledError::new("Failed to write header").with_label(e.to_string(), span)
            })?;
        }

        let mut has_date = vec![false; columns.len()];
        for (row_idx, record) in records.iter().enumerate() {
            let row = u32::try_from(row_idx + 1).map_err(|_| {
                LabeledError::new("Too many rows").with_label("exceeds u32 range", span)
            })?;
            if let Value::Record { val, .. } = record {
                #[allow(clippy::cast_possible_truncation)]
                for (col, col_name) in columns.iter().enumerate() {
                    if let Some(cell_value) = val.get(col_name) {
                        if matches!(cell_value, Value::Date { .. }) {
                            has_date[col] = true;
                        }
                        write_cell(worksheet, row, col as u16, cell_value, &date_format)?;
                    }
                }
            }
        }

        if !raw {
            let last_row = u32::try_from(records.len()).map_err(|_| {
                LabeledError::new("Too many rows").with_label("exceeds u32 range", span)
            })?;
            worksheet
                .add_table(0, 0, last_row, num_cols - 1, &Table::new())
                .map_err(|e| {
                    LabeledError::new("Failed to add table").with_label(e.to_string(), span)
                })?;
        }

        // `autofit` sizes a datetime cell from a fixed 68-pixel guess, blind to the
        // format actually applied to it, so `yyyy-mm-dd hh:mm:ss` overflows and Excel
        // prints ### instead of the value. Claim the width the format needs first:
        // autofit only widens a user-set column, never narrows one.
        #[allow(clippy::cast_possible_truncation)]
        for (col, is_date) in has_date.iter().enumerate() {
            if !is_date {
                continue;
            }
            worksheet
                .set_column_width(col as u16, date_column_width())
                .map_err(|e| {
                    LabeledError::new("Failed to set column width").with_label(e.to_string(), span)
                })?;
        }

        worksheet.autofit();
    }

    workbook
        .save_to_buffer()
        .map_err(|e| LabeledError::new("Failed to write workbook").with_label(e.to_string(), span))
}

fn write_cell(
    worksheet: &mut Worksheet,
    row: u32,
    col: u16,
    value: &Value,
    date_format: &Format,
) -> Result<(), LabeledError> {
    let map_err = |e: XlsxError| {
        LabeledError::new("Failed to write cell").with_label(e.to_string(), value.span())
    };

    match value {
        Value::String { val, .. } => {
            worksheet.write_string(row, col, val).map_err(map_err)?;
        }
        Value::Int { val, .. } => {
            #[allow(clippy::cast_precision_loss)]
            worksheet
                .write_number(row, col, *val as f64)
                .map_err(map_err)?;
        }
        Value::Float { val, .. } => {
            worksheet.write_number(row, col, *val).map_err(map_err)?;
        }
        Value::Bool { val, .. } => {
            worksheet.write_boolean(row, col, *val).map_err(map_err)?;
        }
        Value::Date { val, .. } => {
            worksheet
                .write_datetime_with_format(row, col, val.naive_utc(), date_format)
                .map_err(map_err)?;
        }
        Value::Duration { val, .. } => {
            #[allow(clippy::cast_precision_loss)]
            worksheet
                .write_number(row, col, *val as f64 / 1_000_000_000.0)
                .map_err(map_err)?;
        }
        Value::Filesize { val, .. } => {
            #[allow(clippy::cast_precision_loss)]
            worksheet
                .write_number(row, col, val.get() as f64)
                .map_err(map_err)?;
        }
        Value::Nothing { .. } => {}
        _ => {
            let text = value.to_expanded_string(", ", &Config::default());
            worksheet.write_string(row, col, text).map_err(map_err)?;
        }
    }

    Ok(())
}

const SHEET_NAME_RULES: &str = "Excel sheet names are 1 to 31 characters, cannot contain \
                                [ ] : * ? / \\, cannot start or end with an apostrophe, and \
                                must be unique regardless of case.";

/// Name a worksheet, or explain which of Excel's rules the key broke.
fn name_sheet(
    sheet: &mut Worksheet,
    name: &str,
    span: nu_protocol::Span,
) -> Result<(), LabeledError> {
    sheet.set_name(name).map(|_| ()).map_err(|e| {
        LabeledError::new(format!("Invalid sheet name \"{name}\""))
            .with_label(sheet_name_problem(name, &e), span)
            .with_help(SHEET_NAME_RULES)
    })
}

/// Excel matches sheet names without regard to case, and `rust_xlsxwriter` only
/// notices at save time — by which point the error can name neither key. Caught
/// here instead, where both are still in hand.
fn reject_duplicate_sheet_names(
    sheets: &[(String, Value)],
    span: nu_protocol::Span,
) -> Result<(), LabeledError> {
    let mut seen: Vec<(String, &str)> = Vec::with_capacity(sheets.len());
    for (name, _) in sheets {
        let folded = name.to_lowercase();
        if let Some((_, first)) = seen.iter().find(|(f, _)| *f == folded) {
            let detail = if *first == name.as_str() {
                format!("\"{name}\" appears twice")
            } else {
                format!("\"{first}\" and \"{name}\" differ only in case")
            };
            return Err(LabeledError::new("Duplicate sheet name")
                .with_label(detail, span)
                .with_help(SHEET_NAME_RULES));
        }
        seen.push((folded, name));
    }
    Ok(())
}

/// Say which of Excel's sheet-name rules a key broke.
///
/// `rust_xlsxwriter` reports these as distinct error variants, but its messages
/// name the constraint in library terms. A user who typed `Q1/Q2 2024` wants to
/// read about the slash, not about a `SheetnameContainsInvalidCharacter`.
/// Falls back to the library's own words for anything not enumerated here, so a
/// new rule upstream degrades to a vaguer message rather than a wrong one.
fn sheet_name_problem(name: &str, err: &XlsxError) -> String {
    const FORBIDDEN: [char; 7] = ['[', ']', ':', '*', '?', '/', '\\'];
    match err {
        XlsxError::SheetnameCannotBeBlank(_) => "the name is empty".to_string(),
        XlsxError::SheetnameLengthExceeded(_) => {
            format!("{} characters, and Excel allows 31", name.chars().count())
        }
        XlsxError::SheetnameContainsInvalidCharacter(_) => {
            let bad: Vec<String> = name
                .chars()
                .filter(|c| FORBIDDEN.contains(c))
                .map(|c| format!("'{c}'"))
                .collect();
            if bad.is_empty() {
                "contains a character Excel forbids".to_string()
            } else {
                format!("contains {}", bad.join(", "))
            }
        }
        XlsxError::SheetnameStartsOrEndsWithApostrophe(_) => {
            "starts or ends with an apostrophe".to_string()
        }
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use calamine::{open_workbook_from_rs, Reader, Xlsx};
    use nu_protocol::{record, Span, Value};
    use std::io::Cursor;

    fn span() -> Span {
        Span::test_data()
    }

    fn read_xlsx(bytes: &[u8]) -> Xlsx<Cursor<Vec<u8>>> {
        open_workbook_from_rs(Cursor::new(bytes.to_vec())).expect("Failed to open xlsx from buffer")
    }

    #[test]
    fn single_table_creates_sheet1() {
        let table = Value::list(
            vec![Value::test_record(record! {
                "name" => Value::test_string("Alice"),
                "age" => Value::test_int(30),
            })],
            span(),
        );

        let sheets = vec![("Sheet1".to_string(), table)];
        let bytes = write_workbook(&sheets, false, span()).expect("write_workbook failed");
        let mut wb = read_xlsx(&bytes);

        assert_eq!(wb.sheet_names(), &["Sheet1"]);

        let range = wb.worksheet_range("Sheet1").expect("Failed to read sheet");
        assert_eq!(range.get_size().0, 2); // header + 1 data row
    }

    #[test]
    fn multi_sheet_from_record() {
        let users = Value::list(
            vec![Value::test_record(record! {
                "name" => Value::test_string("Bob"),
            })],
            span(),
        );
        let orders = Value::list(
            vec![Value::test_record(record! {
                "item" => Value::test_string("Widget"),
                "qty" => Value::test_int(5),
            })],
            span(),
        );

        let sheets = vec![("Users".to_string(), users), ("Orders".to_string(), orders)];
        let bytes = write_workbook(&sheets, false, span()).expect("write_workbook failed");
        let wb = read_xlsx(&bytes);

        assert_eq!(wb.sheet_names(), &["Users", "Orders"]);
    }

    #[test]
    fn empty_table_creates_empty_sheet() {
        let table = Value::list(vec![], span());
        let sheets = vec![("Empty".to_string(), table)];
        let bytes = write_workbook(&sheets, false, span()).expect("write_workbook failed");
        let wb = read_xlsx(&bytes);

        assert_eq!(wb.sheet_names(), &["Empty"]);
    }

    #[test]
    fn raw_mode_no_table() {
        let table = Value::list(
            vec![Value::test_record(record! {
                "x" => Value::test_int(1),
            })],
            span(),
        );

        let sheets = vec![("Sheet1".to_string(), table)];
        let bytes = write_workbook(&sheets, true, span()).expect("write_workbook failed");
        assert!(!bytes.is_empty());
    }

    #[test]
    fn type_mapping_values() {
        let table = Value::list(
            vec![Value::test_record(record! {
                "str" => Value::test_string("hello"),
                "int" => Value::test_int(42),
                "float" => Value::test_float(2.72),
                "bool" => Value::test_bool(true),
                "empty" => Value::nothing(span()),
            })],
            span(),
        );

        let sheets = vec![("Sheet1".to_string(), table)];
        let bytes = write_workbook(&sheets, true, span()).expect("write_workbook failed");
        let mut wb = read_xlsx(&bytes);

        let range = wb.worksheet_range("Sheet1").expect("Failed to read sheet");
        let rows: Vec<_> = range.rows().collect();
        assert_eq!(rows.len(), 2);

        let data = &rows[1];
        assert_eq!(data[0], calamine::Data::String("hello".to_string()));
        assert_eq!(data[1], calamine::Data::Float(42.0));
        assert_eq!(data[2], calamine::Data::Float(2.72));
        assert_eq!(data[3], calamine::Data::Bool(true));
        assert_eq!(data[4], calamine::Data::Empty);
    }

    #[test]
    fn rejects_non_table_input() {
        let string_val = Value::test_string("not a table");
        let sheets = vec![("Sheet1".to_string(), string_val)];
        assert!(write_workbook(&sheets, false, span()).is_err());
    }

    /// One sheet holding one row, under whatever name the test wants to try.
    fn one_sheet(name: &str) -> Vec<(String, Value)> {
        let row = Value::test_record(record! { "a" => Value::test_int(1) });
        vec![(name.to_string(), Value::test_list(vec![row]))]
    }

    /// The text a user actually reads: title, labels and help, joined.
    ///
    /// Not `{err:?}` — `Debug` re-escapes, so a backslash comes back doubled
    /// and a test asserting the opposite would pass against the wrong string.
    fn failure_label(sheets: &[(String, Value)]) -> String {
        let err = write_workbook(sheets, false, span()).expect_err("expected a rejection");
        let labels: Vec<&str> = err.labels.iter().map(|l| l.text.as_str()).collect();
        format!(
            "{} | {} | {}",
            err.msg,
            labels.join(" "),
            err.help.unwrap_or_default()
        )
    }

    #[test]
    fn sheet_name_error_names_the_forbidden_character() {
        let msg = failure_label(&one_sheet("Q1/Q2 2024"));
        assert!(msg.contains("Q1/Q2 2024"), "names the key: {msg}");
        assert!(msg.contains("contains '/'"), "names the character: {msg}");
    }

    #[test]
    fn sheet_name_error_names_every_forbidden_character() {
        let msg = failure_label(&one_sheet("a[b]"));
        assert!(msg.contains("'['"), "{msg}");
        assert!(msg.contains("']'"), "{msg}");
    }

    #[test]
    fn sheet_name_error_gives_the_length() {
        let msg = failure_label(&one_sheet(&"x".repeat(32)));
        assert!(msg.contains("32 characters"), "counts them: {msg}");
        assert!(msg.contains("31"), "states the limit: {msg}");
    }

    /// A backslash is both a forbidden character and the one Rust's `Debug`
    /// would double, so it proves the message echoes what the user typed.
    #[test]
    fn sheet_name_error_shows_the_name_as_typed() {
        let msg = failure_label(&one_sheet("a\\b"));
        assert!(msg.contains(r#""a\b""#), "no doubled backslash: {msg}");
        assert!(msg.contains("contains '\\'"), "names it once: {msg}");
    }

    /// The help lists the rules, so it has to list the one just reported.
    #[test]
    fn sheet_name_help_covers_the_apostrophe_rule() {
        let msg = failure_label(&one_sheet("'Q1'"));
        assert!(msg.contains("apostrophe"), "label names it: {msg}");
        assert!(
            SHEET_NAME_RULES.contains("apostrophe"),
            "and the help lists it"
        );
    }

    #[test]
    fn sheet_name_error_says_when_it_is_empty() {
        let msg = failure_label(&one_sheet(""));
        assert!(msg.contains("empty"), "{msg}");
    }

    /// Excel folds case, and the library only notices at save time, where the
    /// two keys are no longer available to name.
    #[test]
    fn duplicate_sheet_names_differing_only_in_case_are_rejected() {
        let row = Value::test_record(record! { "a" => Value::test_int(1) });
        let sheets = vec![
            ("Ventes".to_string(), Value::test_list(vec![row.clone()])),
            ("ventes".to_string(), Value::test_list(vec![row])),
        ];
        let msg = failure_label(&sheets);
        assert!(msg.contains("Ventes"), "{msg}");
        assert!(msg.contains("ventes"), "{msg}");
        assert!(msg.contains("case"), "{msg}");
    }

    /// The empty-table path creates its worksheet somewhere else, so it needs
    /// its own proof that the name is checked the same way.
    #[test]
    fn an_empty_sheet_still_validates_its_name() {
        let sheets = vec![("x/y".to_string(), Value::test_list(vec![]))];
        let msg = failure_label(&sheets);
        assert!(msg.contains("contains '/'"), "{msg}");
    }

    #[test]
    fn date_column_is_wide_enough_for_its_format() {
        use chrono::{FixedOffset, TimeZone};
        use std::io::Read;

        let dt = chrono::NaiveDate::from_ymd_opt(2026, 4, 6)
            .expect("valid date")
            .and_hms_opt(9, 1, 34)
            .expect("valid time");
        let dt_fixed = FixedOffset::east_opt(0)
            .expect("valid offset")
            .from_utc_datetime(&dt);

        let table = Value::list(
            vec![Value::test_record(record! {
                "d" => Value::date(dt_fixed, span()),
            })],
            span(),
        );
        let sheets = vec![("Sheet1".to_string(), table)];
        let bytes = write_workbook(&sheets, true, span()).expect("write_workbook failed");

        let mut archive =
            zip::ZipArchive::new(Cursor::new(bytes)).expect("output is not a zip archive");
        let mut xml = String::new();
        archive
            .by_name("xl/worksheets/sheet1.xml")
            .expect("no sheet1.xml")
            .read_to_string(&mut xml)
            .expect("sheet1.xml is not utf-8");

        let width: f64 = xml
            .split("width=\"")
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .expect("no column width recorded")
            .parse()
            .expect("width is not a number");

        // autofit sizes a datetime from a fixed 68-pixel guess, blind to the format
        // applied to it. Anything narrower than the format renders as ### in Excel.
        #[allow(clippy::cast_precision_loss)]
        let needed = DATE_NUM_FORMAT.len() as f64;
        assert!(
            width >= needed,
            "date column is {width} wide, too narrow for {DATE_NUM_FORMAT}",
        );
    }

    #[test]
    fn date_written_as_excel_date() {
        use chrono::{FixedOffset, TimeZone};
        let dt = chrono::NaiveDate::from_ymd_opt(2026, 4, 6)
            .expect("valid date")
            .and_hms_opt(9, 1, 34)
            .expect("valid time");
        let dt_fixed = FixedOffset::east_opt(0)
            .expect("valid offset")
            .from_utc_datetime(&dt);

        let table = Value::list(
            vec![Value::test_record(record! {
                "modified" => Value::date(dt_fixed, span()),
            })],
            span(),
        );

        let sheets = vec![("Sheet1".to_string(), table)];
        let bytes = write_workbook(&sheets, true, span()).expect("write_workbook failed");
        let mut wb = read_xlsx(&bytes);

        let range = wb.worksheet_range("Sheet1").expect("Failed to read sheet");
        let rows: Vec<_> = range.rows().collect();
        let cell = &rows[1][0];

        assert!(
            matches!(cell, calamine::Data::DateTime(_) | calamine::Data::Float(_)),
            "Expected DateTime or Float (Excel serial date), got {cell:?}",
        );
    }

    #[test]
    fn sparse_records_with_missing_columns() {
        // Row 1 has "a" and "b", row 2 only has "a" — "b" should be empty
        let table = Value::list(
            vec![
                Value::test_record(record! {
                    "a" => Value::test_int(1),
                    "b" => Value::test_string("present"),
                }),
                Value::test_record(record! {
                    "a" => Value::test_int(2),
                }),
            ],
            span(),
        );

        let sheets = vec![("Sheet1".to_string(), table)];
        let bytes = write_workbook(&sheets, true, span()).expect("write_workbook failed");
        let mut wb = read_xlsx(&bytes);

        let range = wb.worksheet_range("Sheet1").expect("Failed to read sheet");
        let rows: Vec<_> = range.rows().collect();
        assert_eq!(rows.len(), 3); // header + 2 data rows

        // Row 2, column "b" should be empty since the record didn't have it
        assert_eq!(rows[2][1], calamine::Data::Empty);
    }

    #[test]
    fn mixed_types_in_same_column() {
        // Column "val" has an int in row 1, a string in row 2, nothing in row 3
        let table = Value::list(
            vec![
                Value::test_record(record! {
                    "val" => Value::test_int(42),
                }),
                Value::test_record(record! {
                    "val" => Value::test_string("hello"),
                }),
                Value::test_record(record! {
                    "val" => Value::nothing(span()),
                }),
            ],
            span(),
        );

        let sheets = vec![("Sheet1".to_string(), table)];
        let bytes = write_workbook(&sheets, true, span()).expect("write_workbook failed");
        let mut wb = read_xlsx(&bytes);

        let range = wb.worksheet_range("Sheet1").expect("Failed to read sheet");
        let rows: Vec<_> = range.rows().collect();

        assert_eq!(rows[1][0], calamine::Data::Float(42.0));
        assert_eq!(rows[2][0], calamine::Data::String("hello".to_string()));
        // Row 3 (nothing) — calamine may trim trailing empty rows,
        // so just verify the first two data rows are correct
    }
}
