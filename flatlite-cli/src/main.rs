mod color_scheme;

use std::fs::File;
use rusqlite::{params_from_iter, Connection};
use eyre::{Context, Result};
use ratatui::{DefaultTerminal, Frame};
use ratatui::crossterm::event;
use ratatui::crossterm::event::{Event, KeyCode};
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;
use rusqlite::fallible_streaming_iterator::FallibleStreamingIterator;
use rusqlite::types::{FromSql, ValueRef};
use crate::color_scheme::ColorScheme;

#[derive(Default)]
struct Entity {
    table: String,
    pub columns: Vec<String>,
}

#[derive(Default)]
struct Schema {
    entities: Vec<Entity>,
}

fn ingest_csv_table(conn: &Connection, schema: &mut Schema, table_name: &str, file: &str) -> Result<()> {
    let file = File::open(file).wrap_err("Failed to open file")?;
    let mut rdr = csv::Reader::from_reader(file);

    let headers = rdr.headers().wrap_err("Failed to read headers")?.iter().collect::<Vec<_>>();

    let columns = headers.iter().map(|s| s.to_string()).collect::<Vec<_>>().join(", ");
    let column_definitions = headers.iter().map(|h| format!("{} TEXT", h)).collect::<Vec<String>>().join(", ");

    let sql = format!("CREATE TABLE {} (__row INTEGER PRIMARY KEY, {})", table_name, column_definitions);
    conn.execute(&sql, [])?;

    let mut e = Entity::default();
    e.table = table_name.to_string();
    e.columns = headers.iter().map(|s| s.to_string()).collect();
    schema.entities.push(e);

    for row_result in rdr.records() {
        let row = row_result.wrap_err("Failed to read row")?;

        let placeholders = row.iter().map(|_| "?").collect::<Vec<_>>().join(",");

        let sql = format!("INSERT INTO {} ({}) VALUES ({})", table_name, columns, placeholders);
        // let values = row.iter().collect::<Vec<_>>();

        conn.execute(&sql, params_from_iter(row.iter())).wrap_err("Failed to INSERT row")?;
    }

    Ok(())
}

fn main() -> Result<()> {

    match std::fs::remove_file("test.sqlite") {
        Ok(_) => println!("Removed test file"),
        Err(e) => println!("{:?}", e),
    }

    let conn = Connection::open("test.sqlite")?;

    let mut schema = Schema::default();

    ingest_csv_table(&conn, &mut schema, "todo", "../docs/todo.csv")?;
    ingest_csv_table(&conn, &mut schema, "time_entry", "../docs/time_entries.csv")?;
    ingest_csv_table(&conn, &mut schema, "milestone", "../docs/milestone.csv")?;

    let app = App {
        conn,
        schema,
        color_scheme: ColorScheme::default(),
        current_sheet: 0,
        debug_text: "".to_string(),
        should_quit: false,
        selected_col: 0,
        selected_row: 0,
    };

    let terminal = ratatui::init();
    let result = run(terminal, app);
    ratatui::restore();
    result
}

struct TabBar<'a> {
    tabs: Vec<String>,
    selected_index: usize,
    color_scheme: &'a ColorScheme,
}

impl <'a> Widget for TabBar<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut x: u16 = 0;

        for (i, tab) in self.tabs.iter().enumerate() {
            let size = tab.len() as u16; // TODO: Unicode
            if x + size > area.width {
                // Can't fit any more tabs
                return
            }

            let style = match i == self.selected_index {
                true => self.color_scheme.active_tab,
                false => self.color_scheme.inactive_tab,
            };

            if i > 0 {
                // Spacer
                buf.set_string(area.x + x, area.y, " ", Style::default());
                x += 1;
            }
            buf.set_string(area.x + x, area.y, tab, Style::from(style));
            x += size;
        }
    }
}

struct CellData {
    display: String,
}

impl FromSql for CellData {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        let display = match value {
            ValueRef::Null => "[null]".to_string(),
            ValueRef::Integer(v) => format!("{}", v),
            ValueRef::Real(v) => format!("{}", v),
            ValueRef::Text(v) => value.as_str()?.to_string(),
            ValueRef::Blob(v) => format!("[{} bytes]", v.len()),
        };
        Ok(CellData { display })
    }
}

fn table_view(app: &App, area: Rect, buf: &mut Buffer) {
    let entity = app.current_entity();
    let column_constraints: Vec<Constraint> = entity.columns.iter().map(|_| Constraint::Fill(1)).collect();
    let column_layout = Layout::new(Direction::Horizontal, column_constraints).split(area);

    for (i, col) in entity.columns.iter().enumerate() {
        let col_area = column_layout[i];
        let heading_cell_area = Rect::new(col_area.x, 0, col_area.width, 1);
        buf.set_string(col_area.x, col_area.y, col, app.color_scheme.sheet_heading_inactive);

        let style = if i == app.selected_col { app.color_scheme.sheet_heading_active } else { app.color_scheme.sheet_heading_inactive };
        buf.set_style(heading_cell_area, style)
    }

    let limit = area.height - 1;
    let mut stmt = app.conn.prepare(&format!("SELECT * from {} LIMIT {}", entity.table, limit)).unwrap();
    let mut rows = stmt.query([]).unwrap();

    let mut y = 1;
    let mut row_idx = 0;

    while let Some(row) = rows.next().unwrap() {
        for (i, col_area) in column_layout.iter().enumerate() {
            let s: CellData = row.get(i + 1).unwrap();

            let style = match () {
                _ if i == app.selected_col && row_idx == app.selected_row => app.color_scheme.cell_selected,
                _ if i == app.selected_col || row_idx == app.selected_row => app.color_scheme.cell_aligned,
                _ => app.color_scheme.cell,
            };

            let para = Paragraph::new(s.display).style(style);

            para.render(Rect::new(col_area.x, y, col_area.width, 1), buf);
        }
        y += 1;
        row_idx += 1;
    }
}

fn sheet_view(app: &App, area: Rect, buf: &mut Buffer) {
    let layout = Layout::new(Direction::Vertical, vec![Constraint::Ratio(1, 1), Constraint::Min(1)]).split(area);

    table_view(app, layout[0], buf);

    let tab = TabBar {
        tabs: app.schema.entities.iter().map(|e| e.table.to_string()).collect(),
        color_scheme: &app.color_scheme,
        selected_index: app.current_sheet,
    };
    tab.render(layout[1], buf);
}

struct App {
    conn: Connection,
    schema: Schema,
    color_scheme: ColorScheme,
    current_sheet: usize,
    should_quit: bool,
    debug_text: String,
    selected_col: usize,
    selected_row: usize,
}

impl App {
    fn current_entity(&self) -> &Entity {
        self.schema.entities.get(self.current_sheet).unwrap()
    }
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![
                Constraint::Ratio(1, 1),
                Constraint::Min(40),
            ]).split(area);

        sheet_view(&self, layout[0], buf);

        let debug = Paragraph::new(self.debug_text.clone()).style(self.color_scheme.debug_panel);
        debug.render(layout[1], buf);

        // buf.set_string(0, 0, "Hello world", Style::default());
        // MyHeaderWidget::new("Header text")
        //     .render(Rect::new(0, 0, area.width, 1), buf);
    }
}

fn run(mut terminal: DefaultTerminal, mut app: App) -> Result<()> {
    while !app.should_quit {
        terminal.draw(|frame| {
            frame.render_widget(&app, frame.area())
        }).wrap_err("Failed to render app")?;

        match event::read()? {
            Event::FocusGained => {}
            Event::FocusLost => {}
            Event::Key(k) => {
                app.debug_text = format!("{:#?}", k);
                match k.code {
                    KeyCode::Char('[') => {
                        if app.current_sheet == 0 {
                            app.current_sheet = app.schema.entities.len() - 1
                        } else {
                            app.current_sheet = app.current_sheet - 1;
                        }
                    },
                    KeyCode::Right => { app.selected_col += 1; }
                    KeyCode::Left => { if app.selected_col > 0 { app.selected_col -= 1 }; }
                    KeyCode::Up => { if app.selected_row > 0 { app.selected_row -= 1 }; }
                    KeyCode::Down => { app.selected_row += 1 }
                    KeyCode::Char(']') => {
                        app.current_sheet = (app.current_sheet + 1) % app.schema.entities.len();
                    },
                    KeyCode::Char('q') => break,
                    _ => (),
                }
            }
            Event::Mouse(_) => {}
            Event::Paste(_) => {}
            Event::Resize(_, _) => {}
        };
        // if matches!(event::read()?, Event::Key(_)) {
        //     break;
        // }
    }

    Ok(())

    // loop {
    //     terminal.draw(render)?;
    // }
}

// fn render(frame: &mut Frame) {
//     frame.render_widget("hello world", frame.area());
// }
