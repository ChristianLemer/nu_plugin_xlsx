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
         Using `ls | to xlsx | save files.xlsx` also works (binary input is passed through)."
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

        // Binary passthrough: `to xlsx | save foo.xlsx` causes save to re-invoke
        // `to xlsx` on the already-converted binary.
        if let Value::Binary { .. } = &input {
            return Ok(PipelineData::Value(input, None));
        }

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
            workbook.add_worksheet().set_name(sheet_name).map_err(|e| {
                LabeledError::new("Failed to create worksheet").with_label(e.to_string(), span)
            })?;
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
        worksheet.set_name(sheet_name).map_err(|e| {
            LabeledError::new("Failed to set sheet name").with_label(e.to_string(), span)
        })?;

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
                    LabeledError::new("Failed to set column width")
                        .with_label(e.to_string(), span)
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
