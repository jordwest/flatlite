pub mod text;
mod actions;
mod events;

use std::collections::{HashMap, VecDeque};
use std::fmt::{Display, Formatter};
use std::fs;
use std::fs::File;
use rusqlite::Connection;
use rusqlite::types::{FromSql, ValueRef};
use eyre::{Context, Result};
use rusqlite::fallible_iterator::FallibleIterator;
use crate::color_scheme::ColorScheme;
use crate::db;
use crate::model::actions::Action;
use crate::model::text::TextInput;
use crate::schema::{FieldId, FieldType, Schema, SelectOption, TableId, TableSchema};
use crate::util::Vector2i;

pub struct SheetRow {
    pub rowid: i64,
    pub order: i64,
    pub cells: Vec<CellData>,
}

pub struct SheetColumn {
    pub width: u16,
    pub field_id: FieldId,
}

pub struct SheetCache {
    /// Selected cell (absolute coords of the complete dataset)
    pub selected_cell: Vector2i,
    /// The query offset at which the data starts (subtracted from the selected cell)
    pub start_offset: usize,
    /// Total number of records in the sheet
    pub total_count: usize,
    pub table_id: TableId,
    pub columns: Vec<SheetColumn>,
    /// Virtual rows of loaded data
    pub rows: Vec<SheetRow>,
    /// Group by enabled on field
    pub group_by_field: Option<FieldId>,
    pub group_tabs: Vec<String>,
    pub group_selected: usize,
}

impl SheetCache {
    pub fn end_offset(&self) -> usize {
        self.start_offset + self.rows.len()
    }

    /// Return the cursor coords relative to the currently viewed region of data
    pub fn view_cursor(&self) -> Vector2i {
        self.selected_cell - Vector2i::new(0, self.start_offset as i32)
    }
}

#[derive(Debug)]
pub struct RelatedRecord {
    pub key: String,
    pub label: String,
}

#[derive(Debug)]
pub enum Mode {
    Normal,
    EditingCell(TextInput),
    EditBelongsTo { search: TextInput, results: Vec<RelatedRecord>, selected_index: usize }
}

impl Mode {
    pub fn is_normal(&self) -> bool {
        match self {
            Mode::Normal => true,
            _ => false,
        }
    }

    pub fn is_editing(&self) -> bool {
        match self {
            Mode::EditingCell(_) => true,
            _ => false,
        }
    }
}

pub struct App {
    pub command_buffer: VecDeque<Action>,
    pub conn: Connection,
    pub schema: Schema,
    pub color_scheme: ColorScheme,
    pub sheets_cache: HashMap<TableId, SheetCache>,
    pub current_sheet: TableId,
    pub available_size: Vector2i,
    pub show_debug: bool,
    pub should_quit: bool,
    pub debug_text: String,
    pub mode: Mode,
}

impl App {
    pub fn new(conn: Connection, schema: Schema, initial_size: Vector2i) -> Self {
        let mut app = App {
            command_buffer: VecDeque::new(),
            current_sheet: TableId(0),
            show_debug: false,
            debug_text: "".to_string(),
            should_quit: false,
            available_size: initial_size,
            sheets_cache: HashMap::new(),
            mode: Mode::Normal,
            conn,
            schema,
            color_scheme: ColorScheme::default(),
        };

        app.populate_sheet(TableId(0));

        app
    }

    pub fn current_table_schema(&self) -> &TableSchema {
        self.schema.table(self.current_sheet)
    }

    pub fn populate_sheet(&mut self, table_id: TableId) {
        let table = self.schema.table(table_id);
        let limit = (self.available_size.y - 3) as usize;

        let existing_cache = self.sheets_cache.get(&table_id);
        let (selected_cell, offset, group_by_field, group_selected) = match existing_cache {
            Some(c) => (c.selected_cell, c.start_offset, c.group_by_field, c.group_selected),
            None => (Vector2i::new(0, 0), 0, None, 0),
        };

        let count = db::count_rows(&mut self.conn, &table.name).unwrap();

        let group_tabs = match group_by_field {
            Some(field_id) => {
                let field = self.schema.field(field_id);
                db::groups(&mut self.conn, &table.name, &field.name).unwrap()
            },
            None => Vec::new(),
        };

        let current_group = match (group_by_field) {
            Some(id) => {
                let field = self.schema.field(id);
                Some(db::groups(&mut self.conn, &table.name, &field.name).unwrap()[group_selected].clone())
            },
            None => None,
        };

        let mut stmt = match group_by_field {
            Some(fid) => {
                let group_field = self.schema.field(fid);
                self.conn.prepare(&format!("SELECT rowid, * from {} WHERE {} = ? ORDER BY __order LIMIT ? OFFSET ?", table.name, group_field.name)).unwrap()
            },
            _ => {
                self.conn.prepare(&format!("SELECT rowid, * from {} ORDER BY __order LIMIT ? OFFSET ?", table.name)).unwrap()
            }
        };

        let mut rows = match current_group {
            Some(group_field_value, ) => {
                stmt.query((group_field_value, limit, offset)).unwrap()
            },
            None => {
                stmt.query([limit, offset]).unwrap()
            }
        };
        //
        // let mut stmt = self.conn.prepare(&format!("SELECT rowid, * from {} ORDER BY __order LIMIT ? OFFSET ?", table.name)).unwrap();
        // let mut rows = stmt.query([limit, offset]).unwrap();

        let columns = self.schema.fields_for_table(table_id);

        let mut cache = SheetCache {
            selected_cell,
            start_offset: offset,
            rows: Vec::new(),
            group_by_field,
            total_count: count as usize,
            columns: columns.iter().map(|s| SheetColumn { field_id: s.id, width: (s.name.len() + 2) as u16 }).collect(),
            table_id,
            group_tabs,
            group_selected,
        };

        while let Some(row) = rows.next().unwrap() {
            let mut sheet_row = SheetRow {
                rowid: row.get(0).unwrap(),
                order: row.get(1).unwrap(),
                cells: Vec::with_capacity(columns.len()),
            };

            for (i, column) in columns.iter().enumerate() {
                let cell_value: CellValue = row.get(i + 2).unwrap();

                let cell_data = match column.field_type {
                    FieldType::StringField => { CellData {
                        display: cell_value.to_string(),
                        value: cell_value,
                    } }
                    FieldType::SelectField { ref options } => {
                        let display = match options.iter().find(|o| &o.key == &cell_value.to_string()) {
                            Some(SelectOption { label: Some(label), .. }) => label.clone(),
                            _ => cell_value.to_string(),
                        };
                        CellData {
                            display,
                            value: cell_value,
                        }
                    }
                    FieldType::BelongsToField(related_table_id, related_key_id) => {
                        let related_table = self.schema.table(related_table_id);
                        let related_field = self.schema.field(related_key_id);

                        let mut stmt = self.conn.prepare(&format!("SELECT title FROM {} WHERE {} = ?", related_table.name, related_field.name)).unwrap();
                        let result = stmt.query_row([&cell_value.to_string()], |r| {
                            r.get(0)
                        });
                        let display = match result {
                            Ok(str) => str,
                            Err(_) => cell_value.to_string(),
                        };

                        CellData {
                            display,
                            value: cell_value,
                        }
                    }
                };

                // Technically should be using char len for accuracy with unicode, but that would
                // be more expensive on long fields. This is only used for estimating column
                // widths anyway
                let len = cell_data.display.len() + 2;

                if len > 50 {
                    cache.columns[i].width = 50;
                } else if (cache.columns[i].width as usize) < len {
                    cache.columns[i].width = len as u16;
                };

                sheet_row.cells.push(cell_data);
            }

            cache.rows.push(sheet_row);
        }

        self.sheets_cache.insert(table_id, cache);
    }

    pub fn refresh_related_autocomplete(&mut self) {
        let related_table = {
            let sheet = self.active_sheet().unwrap();
            let col_field_id = sheet.columns[sheet.selected_cell.col()].field_id;
            let selected_field = self.schema.field(col_field_id);

            let FieldType::BelongsToField(related_table, related_id) = selected_field.field_type else { return };
            self.schema.table(related_table)
            // (sheet.table_name.clone(), col_name.clone())
        };

        let Mode::EditBelongsTo { search, results, selected_index } = &self.mode else { return };

        let results: Vec<RelatedRecord> = {
            let search_term = format!("%{}%", search.input);
            let mut stmt = self.conn.prepare(&format!("SELECT id, title FROM {} WHERE title LIKE ? OR id LIKE ?", related_table.name)).unwrap();
            stmt.query_map([&search_term, &search_term], |r| Ok(RelatedRecord {
                key: r.get(0)?,
                label: r.get(1)?,
            }))
                .unwrap()
                .collect::<rusqlite::Result<Vec<RelatedRecord>>>()
                .unwrap()
        };

        self.push_action(Action::SetMode(Mode::EditBelongsTo { search: search.clone(), results, selected_index: *selected_index }));
    }

    pub fn active_sheet(&self) -> Option<&SheetCache> {
        self.sheets_cache.get(&self.current_sheet).unwrap().into()
    }

    pub fn active_sheet_mut(&mut self) -> Option<&mut SheetCache> {
        self.sheets_cache.get_mut(&self.current_sheet).unwrap().into()
    }

    pub fn save_entity(&self, table_id: TableId) -> Result<()> {
        let table = self.schema.table(table_id);
        let columns = self.schema.fields_for_table(table_id);

        let column_names: Vec<String> = columns.iter().map(|c| c.name.to_string()).collect();

        let mut stmt = self.conn.prepare(
            &format!("SELECT {} FROM {} ORDER BY __order ASC", column_names.join(","), table.name)
        ).wrap_err("Statement prepare failed")?;

        let mut rows = stmt.query([])?;

        let temp_filename = table.source_file.with_extension(".new");

        {
            let file = File::create(&temp_filename).wrap_err_with(|| format!("Failed to create temp file {}", temp_filename.to_str().unwrap_or_default()))?;
            let mut writer = csv::Writer::from_writer(file);

            // Write header
            writer.write_record(&column_names)?;

            let mut cells: Vec<String> = Vec::new();

            while let Some(row) = rows.next()? {
                cells.clear();

                for (i, _column_name) in columns.iter().enumerate() {
                    let cell_value: CellValue = row.get(i)?;
                    cells.push(cell_value.to_string());
                }
                writer.write_record(&cells)?;
            }
            writer.flush()?;
        }

        // Since everything went ok, delete the original file and rename the new one to the original
        fs::remove_file(&table.source_file)?;
        fs::rename(&temp_filename, &table.source_file)?;

        Ok(())
    }

    pub fn push_action(&mut self, action: Action) {
        self.command_buffer.push_back(action);
    }

    pub fn push_action_front(&mut self, action: Action) {
        self.command_buffer.push_front(action)
    }

    pub fn process_actions(&mut self) {
        while self.command_buffer.len() > 0 {
            let cmd = self.command_buffer.pop_front().unwrap();
            self.process_action(cmd);
        }

        // Check if the data view needs to be scrolled because the cursor is out of range
        let (selected_cell, start_offset, end_offset) = {
            let sheet = self.active_sheet().unwrap();
            (sheet.selected_cell, sheet.start_offset as i32, sheet.end_offset() as i32)
        };
        if selected_cell.y >= end_offset {
            self.process_action(Action::Scroll(selected_cell.y - end_offset + 1));
        } else if selected_cell.y < start_offset {
            self.process_action(Action::Scroll(selected_cell.y - start_offset));
        }
    }
}

#[derive(Clone)]
pub enum CellValue {
    StringValue(String),
    IntValue(i64),
    FloatValue(f64),
    NullValue,
    BlobValue(Vec<u8>),
}

#[derive(Clone)]
pub struct CellData {
    pub value: CellValue,
    pub display: String,
}

impl Display for CellValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            CellValue::StringValue(s) => {write!(f, "{}", s)},
            CellValue::IntValue(i) => {write!(f, "{}", i)},
            CellValue::FloatValue(v) => {write!(f, "{}", v)},
            CellValue::NullValue => {write!(f, "<null>")}
            CellValue::BlobValue(b) => {write!(f, "<{} bytes>", b.len())},
        }
    }
}

impl FromSql for CellValue {
    fn column_result(value: ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        Ok(match value {
            ValueRef::Null => CellValue::NullValue,
            ValueRef::Integer(v) => CellValue::IntValue(v),
            ValueRef::Real(v) => CellValue::FloatValue(v),
            ValueRef::Text(_) => CellValue::StringValue(value.as_str()?.to_string()),
            ValueRef::Blob(v) => CellValue::BlobValue(v.to_vec()),
        })
    }
}
