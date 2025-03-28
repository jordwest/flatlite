mod color_scheme;
mod view;
mod model;
mod db;
mod util;
mod dbconfig;

use std::fs::{read_to_string};
use rusqlite::{Connection};
use eyre::{Context, Result};
use ratatui::{DefaultTerminal};
use ratatui::crossterm::event;
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;
use crate::db::ingest_csv_table;
use crate::dbconfig::DbConfig;
use crate::model::{App, Schema};
use crate::view::sheet_view;

fn main() -> Result<()> {
    match std::fs::remove_file("test.sqlite") {
        Ok(_) => println!("Removed test file"),
        Err(e) => println!("{:?}", e),
    }

    // let conn = Connection::open("test.sqlite")?;
    let mut conn = Connection::open_in_memory()?;

    let mut schema = Schema::default();

    let config_content = read_to_string("../docs/db.kdl")?;
    let config = DbConfig::parse_from_str(&config_content)?;

    for table in &config.schema.tables {
        for file in &table.files {
            ingest_csv_table(&mut conn, &mut schema, &table.name, &format!("../docs/{}", file))?;
        }
    }
    // ingest_csv_table(&conn, &mut schema, "todo", "../docs/todo.csv")?;
    // ingest_csv_table(&conn, &mut schema, "time_entry", "../docs/time_entries.csv")?;
    // ingest_csv_table(&conn, &mut schema, "milestone", "../docs/milestone.csv")?;

    let app = App::new(conn, schema, config);

    let terminal = ratatui::init();
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
    }
}

fn run(mut terminal: DefaultTerminal, mut app: App) -> Result<()> {
    while !app.should_quit {
        terminal.draw(|frame| {
            frame.render_widget(&app, frame.area())
        }).wrap_err("Failed to render app")?;

        app.process_event(event::read()?);
    }

    Ok(())
}
