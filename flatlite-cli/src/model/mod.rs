pub mod text;
mod actions;
mod events;

use std::collections::VecDeque;
use std::fs;
use std::fs::File;
use rusqlite::Connection;
use rusqlite::types::{FromSql, ValueRef};
use eyre::{Context, Result};
use crate::color_scheme::ColorScheme;
use crate::dbconfig::{DbConfig, DbTable, FieldType};
use crate::model::actions::Action;
use crate::model::text::TextInput;
use crate::util::Vector2i;

#[derive(Default)]
pub struct Entity {
    pub table: String,
    pub columns: Vec<String>,
    pub source_file: String,
}

impl Entity {
    pub fn query_columns(&self) -> String {
        self.columns.join(",")
    }
}

#[derive(Default)]
pub struct Schema {
    pub entities: Vec<Entity>,
}

pub struct SheetRow {
    pub rowid: i64,
    pub cells: Vec<CellData>,
}

pub struct SheetColumn {
    pub width: u16,
    pub table_column: String,
}

pub struct SheetCache {
    /// Selected cell (absolute coords of the complete dataset)
    pub selected_cell: Vector2i,
    /// The query offset at which the data starts (subtracted from the selected cell)
    pub start_offset: usize,
    /// Total number of records in the sheet
    pub total_count: usize,
    pub table_config: DbTable,
    pub table_name: String,
    pub columns: Vec<SheetColumn>,
    /// Virtual rows of loaded data
    pub rows: Vec<SheetRow>,
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
    pub config: DbConfig,
    pub color_scheme: ColorScheme,
    pub sheets_cache: Vec<Option<SheetCache>>,
    pub current_sheet: usize,
    pub available_size: Vector2i,
    pub show_debug: bool,
    pub should_quit: bool,
    pub debug_text: String,
    pub mode: Mode,
}

impl App {
    pub fn new(conn: Connection, schema: Schema, config: DbConfig, initial_size: Vector2i) -> Self {
        // Fill the sheets cache with blanks
        let sheets_cache: Vec<Option<SheetCache>> = schema.entities.iter().map(|_| None).collect();

        let mut app = App {
            command_buffer: VecDeque::new(),
            current_sheet: 0,
            show_debug: false,
            debug_text: "".to_string(),
            should_quit: false,
            available_size: initial_size,
            sheets_cache,
            mode: Mode::Normal,
            conn,
            schema,
            config,
            color_scheme: ColorScheme::default(),
        };

        app.populate_sheet(0);

        app
    }

    pub fn current_entity(&self) -> &Entity {
        self.schema.entities.get(self.current_sheet).unwrap()
    }

    pub fn populate_sheet(&mut self, index: usize) {
        let entity = &self.schema.entities[index];
        let limit = (self.available_size.y - 3) as usize;

        let existing_cache = self.sheets_cache.get(index).unwrap();
        let (selected_cell, offset) = match existing_cache {
            Some(c) => (c.selected_cell, c.start_offset),
            None => (Vector2i::new(0, 0), 0),
        };

        let mut count_stmt = self.conn.prepare(&format!("SELECT COUNT(rowid) from {}", entity.table)).unwrap();
        let count: i64 = count_stmt.query_row([], |r| {
            r.get(0)
        }).unwrap();

        let mut stmt = self.conn.prepare(&format!("SELECT * from {} ORDER BY __order LIMIT ? OFFSET ?", entity.table)).unwrap();
        let mut rows = stmt.query([limit, offset]).unwrap();

        let table_config = self.config.schema.tables.iter().find(|t| t.name == entity.table).unwrap().clone();

        let mut cache = SheetCache {
            selected_cell,
            start_offset: offset,
            rows: Vec::new(),
            table_config,
            total_count: count as usize,
            columns: entity.columns.iter().map(|s| SheetColumn { table_column: s.clone(), width: (s.len() + 2) as u16 }).collect(),
            table_name: entity.table.clone(),
        };

        while let Some(row) = rows.next().unwrap() {
            let mut sheet_row = SheetRow {
                rowid: row.get(0).unwrap(),
                cells: Vec::with_capacity(entity.columns.len()),
            };

            for (i, _column_name) in entity.columns.iter().enumerate() {
                let cell_data: CellData = row.get(i + 1).unwrap();

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

        self.sheets_cache[index] = Some(cache);
    }

    pub fn refresh_related_autocomplete(&mut self) {
        let related_table = {
            let sheet = self.active_sheet().unwrap();
            let col_name = &sheet.columns[sheet.selected_cell.col()].table_column;
            let relation = sheet.table_config.fields.iter().find(|f| &f.name == col_name).unwrap();
            let FieldType::BelongsTo(related_table, related_id) = &relation.field_type else { return };
            related_table
            // (sheet.table_name.clone(), col_name.clone())
        };

        let Mode::EditBelongsTo { search, results, selected_index } = &self.mode else { return };

        let results = {
            let mut stmt = self.conn.prepare(&format!("SELECT title FROM {}", related_table)).unwrap();
            let titles = stmt.query_map([], |r| r.get(0)).unwrap();
            let mut results = Vec::new();
            for title in titles {
                results.push(RelatedRecord { label: title.unwrap() });
            }
            results
        };

        self.push_action(Action::SetMode(Mode::EditBelongsTo { search: search.clone(), results, selected_index: *selected_index }));
    }

    pub fn active_sheet(&self) -> Option<&SheetCache> {
        self.sheets_cache.get(self.current_sheet).unwrap().into()
    }

    pub fn active_sheet_mut(&mut self) -> Option<&mut SheetCache> {
        self.sheets_cache.get_mut(self.current_sheet).unwrap().into()
    }

    pub fn save_entity(&self, entity_index: usize) -> Result<()> {
        let entity = self.schema.entities.get(entity_index).unwrap();

        let mut stmt = self.conn.prepare(
            &format!("SELECT {} FROM {} ORDER BY __order ASC", entity.query_columns(), entity.table)
        ).wrap_err("Statement prepare failed")?;


        let mut rows = stmt.query([])?;

        let temp_filename = format!("{}.new", &entity.source_file);
        {
            let file = File::create(&temp_filename).wrap_err_with(|| temp_filename.clone())?;
            let mut writer = csv::Writer::from_writer(file);

            // Write header
            writer.write_record(&entity.columns)?;

            let mut cells: Vec<String> = Vec::new();

            while let Some(row) = rows.next()? {
                cells.clear();

                for (i, _column_name) in entity.columns.iter().enumerate() {
                    let cell_data: CellData = row.get(i)?;
                    cells.push(cell_data.display);
                }
                writer.write_record(&cells)?;
            }
            writer.flush()?;
        }

        // Since everything went ok, delete the original file and rename the new one to the original
        fs::remove_file(&entity.source_file)?;
        fs::rename(&temp_filename, &entity.source_file)?;

        Ok(())
    }

    pub fn push_action(&mut self, action: Action) {
        self.command_buffer.push_back(action);
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
pub struct CellData {
    pub display: String,
}

impl FromSql for CellData {
    fn column_result(value: ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        let display = match value {
            ValueRef::Null => "[null]".to_string(),
            ValueRef::Integer(v) => format!("{}", v),
            ValueRef::Real(v) => format!("{}", v),
            ValueRef::Text(_) => value.as_str()?.to_string(),
            ValueRef::Blob(v) => format!("[{} bytes]", v.len()),
        };
        Ok(CellData { display })
    }
}
