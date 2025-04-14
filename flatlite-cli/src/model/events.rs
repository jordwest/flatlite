use ratatui::crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use crate::model::{App, Mode};
use crate::model::actions::Action;
use crate::schema::TableId;
use crate::util::Vector2i;

impl App {
    pub fn process_event(&mut self, event: Event) {
        match event {
            Event::FocusGained => {}
            Event::FocusLost => {}
            Event::Key(k) => {
                if k.kind != KeyEventKind::Press {
                    return;
                };

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
                    (Mode::EditBelongsTo { search, .. }, KeyCode::Char(c)) => {
                        search.insert_char_at_cursor(c);
                        self.refresh_related_autocomplete();
                    },
                    (Mode::EditBelongsTo { search, .. }, KeyCode::Backspace) => {
                        search.delete_char();
                        self.refresh_related_autocomplete();
                    },
                    (Mode::EditBelongsTo { .. }, KeyCode::Esc) => self.push_action(Action::SetMode(Mode::Normal)),
                    (Mode::EditBelongsTo { .. }, KeyCode::Enter) => self.push_action(Action::FinishEdit),

                    (Mode::EditBelongsTo { .. }, KeyCode::Down) => self.push_action(Action::AutocompleteNext),
                    (Mode::EditBelongsTo { .. }, KeyCode::Tab) if k.modifiers.intersects(KeyModifiers::SHIFT) => self.push_action(Action::AutocompletePrev),
                    (Mode::EditBelongsTo { .. }, KeyCode::Tab) => self.push_action(Action::AutocompleteNext),
                    (Mode::EditBelongsTo { .. }, KeyCode::Up) => self.push_action(Action::AutocompletePrev),

                    (Mode::EditBelongsTo { .. }, _) => {},
                    (Mode::Normal, code) => {
                        match code {
                            KeyCode::Char('[') => {
                                let next_sheet = if self.current_sheet.0 == 0 {
                                    self.schema.tables.len() - 1
                                } else {
                                    self.current_sheet.0 - 1
                                };
                                self.populate_sheet(TableId(next_sheet));
                                self.current_sheet = TableId(next_sheet);
                            },
                            KeyCode::Char(']') => {
                                let next_sheet = (self.current_sheet.0 + 1) % self.schema.tables.len();
                                self.populate_sheet(TableId(next_sheet));
                                self.current_sheet = TableId(next_sheet);
                            },
                            KeyCode::Right => self.push_action(Action::MoveCursor(Vector2i::new(1, 0))),
                            KeyCode::Left => self.push_action(Action::MoveCursor(Vector2i::new(-1, 0))),
                            KeyCode::Up => self.push_action(Action::MoveCursor(Vector2i::new(0, -1))),
                            KeyCode::Down => self.push_action(Action::MoveCursor(Vector2i::new(0, 1))),
                            KeyCode::PageUp => self.push_action(Action::Page(-1)),
                            KeyCode::PageDown => self.push_action(Action::Page(1)),
                            KeyCode::Tab => self.push_action(Action::NextCell),
                            KeyCode::Char(',') => self.push_action(Action::PrevGroup),
                            KeyCode::Char('.') => self.push_action(Action::NextGroup),
                            KeyCode::Char('a') => self.push_action(Action::AddRow),
                            KeyCode::Char('d') => self.push_action(Action::DeleteRow),
                            KeyCode::Char('h') => self.push_action(Action::MoveCursor(Vector2i::new(-1, 0))),
                            KeyCode::Char('j') => self.push_action(Action::MoveCursor(Vector2i::new(0, 1))),
                            KeyCode::Char('g') => self.push_action(Action::SetGroupBy),
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

        // self.debug_text = format!("{:#?} \n\n {:#?}", self.mode, self.schema);
    }
}
