mod text;

use std::collections::VecDeque;
use std::fs;
use std::fs::File;
use ratatui::crossterm::event::{Event, KeyCode, KeyEventKind};
use rusqlite::Connection;
use rusqlite::types::{FromSql, ValueRef};
use eyre::{Context, Result};
use crate::color_scheme::ColorScheme;
use crate::dbconfig::{DbConfig, DbTable, FieldType};
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
pub enum Mode {
    Normal,
    EditingCell(TextInput),
}

pub enum Action {
    MoveCursor(Vector2i),
    Scroll(i32),
    Page(i32),
    AddRow,
    NextCell,
    FinishEdit,
    CancelEdit,
    RefreshView,
    SetMode(Mode),
    EditCell { clear: bool },
    SaveCell { value: String },
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

    pub fn process_action(&mut self, action: Action) {
        match action {
            Action::MoveCursor(rel) => {
                let sheet = self.active_sheet_mut().unwrap();
                sheet.selected_cell = (sheet.selected_cell + rel)
                    .clamp_wrapped(Vector2i::new(sheet.columns.len() as i32, sheet.total_count as i32));
            }
            Action::RefreshView => {
                self.populate_sheet(self.current_sheet);
            }
            Action::Page(amount) => {
                let sheet = self.active_sheet().unwrap();
                self.process_action(Action::MoveCursor(Vector2i::new(0, amount * (sheet.rows.len() as i32))));
            }
            Action::Scroll(amount) => {
                let sheet = self.active_sheet_mut().unwrap();
                if amount < 0 {
                    sheet.start_offset = sheet.start_offset - amount.abs() as usize;
                } else {
                    sheet.start_offset = sheet.start_offset + amount as usize;
                }
                self.populate_sheet(self.current_sheet);
            },
            Action::NextCell => {
                let sheet = self.active_sheet_mut().unwrap();
                let bounds = Vector2i::new(sheet.columns.len() as i32, sheet.total_count as i32);
                
                let mut new_cell = (sheet.selected_cell + Vector2i::new(1, 0)).clamp_wrapped(bounds);
                
                if new_cell.x == 0 {
                    // Move to next row
                    new_cell.y = new_cell.y + 1;
                    new_cell = new_cell.clamp_wrapped(bounds);
                }
                sheet.selected_cell = new_cell;
            }
            Action::EditCell {clear} => {
                let (cell, cell_config) = {
                    let sheet = self.active_sheet().unwrap();
                    let cursor = sheet.view_cursor();
                    let cell = &sheet.rows[cursor.row()].cells[cursor.col()];
                    let col = &sheet.columns[cursor.col()];
                    let cell_config = sheet.table_config.fields.iter().find(|f| f.name == col.table_column).unwrap();

                    (cell.clone(), cell_config.clone())
                };

                match &cell_config.field_type {
                    FieldType::StringType => {
                        self.mode = Mode::EditingCell(TextInput::new(if clear { "" } else { cell.display.as_str() }))
                    }
                    FieldType::SelectType(opts) => {
                        let mut select_next_option = false;
                        for opt in opts {
                            if select_next_option {
                                self.push_action(Action::SaveCell { value: opt.key.clone() });
                                return;
                            }
                            if opt.key == cell.display.as_str() {
                                select_next_option = true;
                            }
                        }
                        self.push_action(Action::SaveCell { value: opts[0].key.clone() });
                    }
                    FieldType::BelongsTo(_, _) => {}
                }
            }
            Action::AddRow => {
                {
                    let sheet = self.active_sheet().unwrap();
                    let cursor = sheet.view_cursor();
                    let row = &sheet.rows[cursor.row()];
                    let new_row_id = row.rowid + 1;

                    self.conn.execute(&format!("UPDATE {} SET __order = __order + 1 WHERE __order >= ?", sheet.table_name), [new_row_id]).unwrap();
                    self.conn.execute(&format!("INSERT INTO {} (__order) VALUES (?)", sheet.table_name), [new_row_id]).unwrap();
                }

                let sheet = self.active_sheet_mut().unwrap();
                sheet.selected_cell = sheet.selected_cell + Vector2i::new(0, 1);
                self.populate_sheet(self.current_sheet);
            }

            Action::FinishEdit => {
                let sheet = self.active_sheet().unwrap();
                let Mode::EditingCell(ref text_input) = self.mode else { return };
                let cursor = sheet.view_cursor();

                if sheet.rows[cursor.row()].cells[cursor.col()].display == text_input.input {
                    self.push_action(Action::SetMode(Mode::Normal));
                    return;
                }

                self.push_action(Action::SaveCell { value: text_input.input.clone() });
            }
            Action::SaveCell { value } => {
                let sheet = self.active_sheet().unwrap();
                let cursor = sheet.view_cursor();

                {
                    let mut stmt = self.conn.prepare(
                        &format!(
                            "UPDATE {} SET {} = ? WHERE __order = ?",
                            sheet.table_name,
                            sheet.columns[cursor.col()].table_column,
                        )).unwrap();

                    stmt.execute((&value, sheet.rows[cursor.row()].rowid)).unwrap();
                }

                self.populate_sheet(self.current_sheet);
                self.mode = Mode::Normal;
                self.save_entity(self.current_sheet).unwrap();
            },
            Action::CancelEdit => {
                self.mode = Mode::Normal;
            }
            Action::SetMode(mode) => {
                self.mode = mode;
            }
        }
    }

    pub fn process_event(&mut self, event: Event) {
        match event {
            Event::FocusGained => {}
            Event::FocusLost => {}
            Event::Key(k) => {
                if k.kind != KeyEventKind::Press {
                    return;
                };

                self.debug_text = format!("{:#?} \n\n {:#?}", self.mode, self.config);

                match (&mut self.mode, k.code) {
                    (Mode::EditingCell(_), KeyCode::Esc) => self.push_action(Action::CancelEdit),
                    (Mode::EditingCell(_), KeyCode::Tab) => {
                        self.push_action(Action::FinishEdit);
                        self.push_action(Action::NextCell);
                    },
                    (Mode::EditingCell(_), KeyCode::Enter) => self.push_action(Action::FinishEdit),
                    (Mode::EditingCell(ref mut input), KeyCode::Char(c)) => input.insert_char_at_cursor(c),
                    (Mode::EditingCell(ref mut input), KeyCode::Backspace) => input.delete_char(),
                    (Mode::EditingCell(_), _) => {},
                    (Mode::Normal, code) => {
                        match code {
                            KeyCode::Char('[') => {
                                let next_sheet = if self.current_sheet == 0 {
                                    self.schema.entities.len() - 1
                                } else {
                                    self.current_sheet - 1
                                };
                                self.populate_sheet(next_sheet);
                                self.current_sheet = next_sheet;
                            },
                            KeyCode::Char(']') => {
                                let next_sheet = (self.current_sheet + 1) % self.schema.entities.len();
                                self.populate_sheet(next_sheet);
                                self.current_sheet = next_sheet;
                            },
                            KeyCode::Right => self.push_action(Action::MoveCursor(Vector2i::new(1, 0))),
                            KeyCode::Left => self.push_action(Action::MoveCursor(Vector2i::new(-1, 0))),
                            KeyCode::Up => self.push_action(Action::MoveCursor(Vector2i::new(0, -1))),
                            KeyCode::Down => self.push_action(Action::MoveCursor(Vector2i::new(0, 1))),
                            KeyCode::PageUp => self.push_action(Action::Page(-1)),
                            KeyCode::PageDown => self.push_action(Action::Page(1)),
                            KeyCode::Tab => self.push_action(Action::NextCell),
                            KeyCode::Char('a') => self.push_action(Action::AddRow),
                            KeyCode::Char('h') => self.push_action(Action::MoveCursor(Vector2i::new(-1, 0))),
                            KeyCode::Char('j') => self.push_action(Action::MoveCursor(Vector2i::new(0, 1))),
                            KeyCode::Char('k') => self.push_action(Action::MoveCursor(Vector2i::new(0, -1))),
                            KeyCode::Char('l') => self.push_action(Action::MoveCursor(Vector2i::new(1, 0))),
                            KeyCode::Char('e') => self.push_action(Action::EditCell { clear: false }),
                            KeyCode::Char('E') => self.push_action(Action::EditCell { clear: true }),
                            KeyCode::Enter => self.push_action(Action::EditCell { clear: false }),
                            KeyCode::Char('q') => self.should_quit = true,
                            _ => (),
                        }
                    },
                }
            }
            Event::Mouse(_) => {}
            Event::Paste(_) => {}
            Event::Resize(_, _) => {
                self.push_action(Action::RefreshView)
            }
        };

        self.process_actions();
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
