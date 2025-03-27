mod text;

use std::fs;
use std::fs::File;
use std::ops::{Add, Sub};
use ratatui::crossterm::event::{Event, KeyCode, KeyEventKind};
use rusqlite::Connection;
use rusqlite::types::{FromSql, ValueRef};
use eyre::{Context, Result};
use rusqlite::ffi::sqlite3_temp_directory;
use crate::color_scheme::ColorScheme;
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
    pub selected_cell: Vector2i,
    pub table_name: String,
    pub columns: Vec<SheetColumn>,
    pub rows: Vec<SheetRow>,
}

#[derive(Debug)]
pub enum Mode {
    Normal,
    EditingCell(TextInput),
}

pub enum Action {
    NavigateBy(Vector2i),
    NextCell,
    CancelEdit,
    EditCell,
    SaveCell,
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
    pub conn: Connection,
    pub schema: Schema,
    pub color_scheme: ColorScheme,
    pub current_sheet: usize,
    pub show_debug: bool,
    pub should_quit: bool,
    pub debug_text: String,
    pub mode: Mode,
    pub sheets_cache: Vec<Option<SheetCache>>,
}

impl App {
    pub fn new(conn: Connection, schema: Schema) -> Self {
        // Fill the sheets cache with blanks
        let sheets_cache: Vec<Option<SheetCache>> = schema.entities.iter().map(|_| None).collect();

        let mut app = App {
            conn,
            schema,
            color_scheme: ColorScheme::default(),
            current_sheet: 0,
            show_debug: false,
            debug_text: "".to_string(),
            should_quit: false,
            sheets_cache,
            mode: Mode::Normal,
        };

        app.populate_sheet(0);

        app
    }

    pub fn current_entity(&self) -> &Entity {
        self.schema.entities.get(self.current_sheet).unwrap()
    }

    pub fn populate_sheet(&mut self, index: usize) {
        let entity = &self.schema.entities[index];
        let limit = 5;

        let existing_cache = self.sheets_cache.get(index).unwrap();
        let selected_cell = match existing_cache {
            Some(c) => c.selected_cell,
            None => Vector2i::new(0, 0),
        };

        let mut stmt = self.conn.prepare(&format!("SELECT * from {} LIMIT {}", entity.table, limit)).unwrap();
        let mut rows = stmt.query([]).unwrap();

        let mut cache = SheetCache {
            selected_cell,
            rows: Vec::new(),
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
            &format!("SELECT {} FROM {} ORDER BY __row ASC", entity.query_columns(), entity.table)
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

    pub fn process_action(&mut self, action: Action) {
        match action {
            Action::NavigateBy(rel) => {
                let sheet = self.active_sheet_mut().unwrap();
                sheet.selected_cell = (sheet.selected_cell + rel)
                    .clamp_wrapped(Vector2i::new(sheet.columns.len() as i32, sheet.rows.len() as i32))
            }
            Action::NextCell => {
                let sheet = self.active_sheet_mut().unwrap();
                let bounds = Vector2i::new(sheet.columns.len() as i32, sheet.rows.len() as i32);
                let mut new_cell = (sheet.selected_cell + Vector2i::new(1, 0)).clamp_wrapped(bounds);
                if new_cell.x == 0 {
                    // Move to next row
                    new_cell.y = new_cell.y + 1;
                    new_cell = new_cell.clamp_wrapped(bounds);
                }
                sheet.selected_cell = new_cell;
            }
            Action::EditCell => {
                let sheet = self.active_sheet().unwrap();
                let cell = &sheet.rows[sheet.selected_cell.row()].cells[sheet.selected_cell.col()];
                self.mode = Mode::EditingCell(TextInput::new(cell.display.as_str()))
            }
            Action::SaveCell => {
                let sheet = self.active_sheet().unwrap();

                {
                    let Mode::EditingCell(ref text_input) = self.mode else { return };

                    if sheet.rows[sheet.selected_cell.row()].cells[sheet.selected_cell.col()].display == text_input.input {
                        self.mode = Mode::Normal;
                        return;
                    }

                    let mut stmt = self.conn.prepare(
                        &format!(
                            "UPDATE {} SET {} = ? WHERE __row = ?",
                            sheet.table_name,
                            sheet.columns[sheet.selected_cell.col()].table_column,
                        )).unwrap();

                    stmt.execute((&text_input.input, sheet.rows[sheet.selected_cell.row()].rowid)).unwrap();
                }

                self.populate_sheet(self.current_sheet);
                self.mode = Mode::Normal;
                self.save_entity(self.current_sheet).unwrap();
            },
            Action::CancelEdit => {
                self.mode = Mode::Normal;
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

                self.debug_text = format!("{:#?} \n\n {:#?}", self.mode, k);

                match (&mut self.mode, k.code) {
                    (Mode::EditingCell(_), KeyCode::Esc) => self.process_action(Action::CancelEdit),
                    (Mode::EditingCell(_), KeyCode::Enter) => self.process_action(Action::SaveCell),
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
                            KeyCode::Right => self.process_action(Action::NavigateBy(Vector2i::new(1, 0))),
                            KeyCode::Left => self.process_action(Action::NavigateBy(Vector2i::new(-1, 0))),
                            KeyCode::Up => self.process_action(Action::NavigateBy(Vector2i::new(0, -1))),
                            KeyCode::Down => self.process_action(Action::NavigateBy(Vector2i::new(0, 1))),
                            KeyCode::Tab => self.process_action(Action::NextCell),
                            KeyCode::Char('h') => self.process_action(Action::NavigateBy(Vector2i::new(-1, 0))),
                            KeyCode::Char('j') => self.process_action(Action::NavigateBy(Vector2i::new(0, 1))),
                            KeyCode::Char('k') => self.process_action(Action::NavigateBy(Vector2i::new(0, -1))),
                            KeyCode::Char('l') => self.process_action(Action::NavigateBy(Vector2i::new(1, 0))),
                            KeyCode::Char('a') => self.process_action(Action::EditCell),
                            KeyCode::Enter => self.process_action(Action::EditCell),
                            KeyCode::Char('q') => self.should_quit = true,
                            _ => (),
                        }
                    },
                }
            }
            Event::Mouse(_) => {}
            Event::Paste(_) => {}
            Event::Resize(_, _) => {}
        };
    }
}

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
