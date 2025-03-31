use crate::model::{App, Mode};
use crate::model::text::TextInput;
use crate::schema::FieldType;
use crate::util::Vector2i;

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

impl App {
    pub fn process_action(&mut self, action: Action) {
        match action {
            Action::MoveCursor(rel) => {
                let sheet = self.active_sheet_mut().unwrap();
                sheet.selected_cell = (sheet.selected_cell + rel)
                    .clamp_wrapped(Vector2i::new(sheet.columns.len() as i32, sheet.total_count as i32));
            }
            Action::RefreshView => {
                match self.mode {
                    Mode::Normal => self.populate_sheet(self.current_sheet),
                    Mode::EditingCell(_) => {},
                    Mode::EditBelongsTo { .. } => self.refresh_related_autocomplete(),
                }
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
                    let cell = sheet.rows[cursor.row()].cells[cursor.col()].clone();
                    let col = &sheet.columns[cursor.col()];
                    let field = self.schema.field(col.field_id);

                    (cell, field)
                };

                match &cell_config.field_type {
                    FieldType::StringField => {
                        self.mode = Mode::EditingCell(TextInput::new(if clear { "" } else { cell.display.as_str() }))
                    }
                    FieldType::SelectField { options } => {
                        let mut select_next_option = false;
                        for opt in options {
                            if select_next_option {
                                self.push_action(Action::SaveCell { value: opt.key.clone() });
                                return;
                            }
                            if opt.key == cell.display.as_str() {
                                select_next_option = true;
                            }
                        }
                        self.push_action(Action::SaveCell { value: options[0].key.clone() });
                    }
                    FieldType::BelongsToField(_, _) => {
                        self.push_action(Action::SetMode(Mode::EditBelongsTo {
                            results: Vec::new(),
                            selected_index: 0,
                            search: TextInput::new(""),
                        }));
                        self.push_action(Action::RefreshView);
                    }
                }
            }
            Action::AddRow => {
                {
                    let sheet = self.active_sheet().unwrap();
                    let table = self.schema.table(sheet.table_id);
                    let cursor = sheet.view_cursor();
                    let row = &sheet.rows[cursor.row()];
                    let new_row_id = row.rowid + 1;

                    self.conn.execute(&format!("UPDATE {} SET __order = __order + 1 WHERE __order >= ?", table.name), [new_row_id]).unwrap();
                    self.conn.execute(&format!("INSERT INTO {} (__order) VALUES (?)", table.name), [new_row_id]).unwrap();
                }

                let sheet = self.active_sheet_mut().unwrap();
                sheet.selected_cell = sheet.selected_cell + Vector2i::new(0, 1);
                self.populate_sheet(self.current_sheet);
            }

            Action::FinishEdit => {
                let sheet = self.active_sheet().unwrap();
                let cursor = sheet.view_cursor();
                
                let existing_value = &sheet.rows[cursor.row()].cells[cursor.col()].display;
                
                let update_value = match self.mode {
                    Mode::EditingCell(ref text_input) if &text_input.input == existing_value => None,
                    Mode::EditingCell(ref text_input) => {
                        Some(text_input.input.clone())
                    },
                    Mode::EditBelongsTo { ref selected_index, ref results, .. } => {
                        Some(results[*selected_index].key.to_string())
                    },
                    Mode::Normal => {
                        None
                    }
                };

                match update_value {
                    None => self.push_action(Action::SetMode(Mode::Normal)),
                    Some(new_value) => self.push_action(Action::SaveCell { value: new_value }),
                }
            }
            Action::SaveCell { value } => {
                let sheet = self.active_sheet().unwrap();
                let table = self.schema.table(sheet.table_id);
                let cursor = sheet.view_cursor();
                let selected_field_name = self.schema.field(sheet.columns[cursor.col()].field_id).name.to_string();

                {
                    let mut stmt = self.conn.prepare(
                        &format!(
                            "UPDATE {} SET {} = ? WHERE __order = ?",
                            table.name,
                            selected_field_name,
                        )).unwrap();

                    stmt.execute((&value, sheet.rows[cursor.row()].rowid)).unwrap();
                }

                self.populate_sheet(self.current_sheet);
                self.mode = Mode::Normal;
                self.save_entity(self.current_sheet).unwrap();
            },
            Action::CancelEdit => self.mode = Mode::Normal,
            Action::SetMode(mode) => self.mode = mode
        }
    }

}
