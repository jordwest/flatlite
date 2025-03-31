mod color_scheme;
mod view;
mod model;
mod db;
mod util;
pub mod dbconfig;
pub mod schema;

use std::fs::{read_to_string};
use std::path::PathBuf;
use rusqlite::{Connection};
use eyre::{Context, Result};
use ratatui::{DefaultTerminal};
use ratatui::crossterm::event;
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;
use crate::db::ingest_csv_table;
use crate::dbconfig::Config;
use crate::model::{App, Mode};
use crate::schema::{Schema, TableId};
use crate::util::Vector2i;
use crate::view::sheet_view;
use crate::view::widgets::autocomplete::Autocomplete;

fn main() -> Result<()> {
    match std::fs::remove_file("test.sqlite") {
        Ok(_) => println!("Removed test file"),
        Err(e) => println!("{:?}", e),
    }

    // let conn = Connection::open("test.sqlite")?;
    let mut conn = Connection::open_in_memory()?;

    let path_to_config = PathBuf::from("../docs/db.kdl");
    let config_content = read_to_string(&path_to_config)?;
    let config = Config::parse_from_str(&config_content, &path_to_config)?;

    let mut schema: Schema = (&config).try_into()?;
    let tables: Vec<(TableId, PathBuf)> = schema.tables.iter().map(|t| (t.id, t.source_file.clone())).collect();

    for (table_id, source_file) in tables {
        ingest_csv_table(&mut conn, &mut schema, table_id, &source_file)?;
    }
    // ingest_csv_table(&conn, &mut schema, "todo", "../docs/todo.csv")?;
    // ingest_csv_table(&conn, &mut schema, "time_entry", "../docs/time_entries.csv")?;
    // ingest_csv_table(&conn, &mut schema, "milestone", "../docs/milestone.csv")?;

    let mut terminal = ratatui::init();
    let area = terminal.get_frame().area();
    let initial_size = Vector2i::new(area.width as i32, area.height as i32);

    let app = App::new(conn, schema, initial_size);

    let result = run(terminal, app);
    ratatui::restore();
    result
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![
                Constraint::Ratio(1, 1),
                Constraint::Min(80),
            ]).split(area);

        sheet_view(&self, layout[0], buf);

        let debug = Paragraph::new(self.debug_text.clone()).style(self.color_scheme.debug_panel);
        debug.render(layout[1], buf);

        if let Mode::EditBelongsTo { search, selected_index, results } = &self.mode {
            let items = results.iter().map(|r| r.label.to_string()).collect();
            let ac = Autocomplete {
                search: &search,
                placeholder: "Search milestone",
                selected_index: *selected_index,
                color_scheme: &self.color_scheme,
                items: &items,
            };

            ac.render(layout[0].inner(Margin::new(20, 10)), buf);
        }
    }
}

fn run(mut terminal: DefaultTerminal, mut app: App) -> Result<()> {
    while !app.should_quit {
        terminal.draw(|frame| {
            frame.render_widget(&app, frame.area())
        }).wrap_err("Failed to render app")?;

        let area = terminal.get_frame().area();
        app.available_size = Vector2i::new(area.width as i32, area.height as i32);
        app.process_event(event::read()?);
    }

    Ok(())
}
