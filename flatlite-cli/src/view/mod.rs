mod widgets;

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;
use crate::model::{App, Mode};
use crate::view::widgets::TabBar;

pub fn table_view(app: &App, area: Rect, buf: &mut Buffer) {
    // let entity = app.current_entity();
    let sheet = app.active_sheet().unwrap();

    let column_constraints: Vec<Constraint> = sheet.columns.iter().map(|c| Constraint::Max(c.width)).collect();
    let column_layout = Layout::new(Direction::Horizontal, column_constraints).split(area);
    
    let view_cursor = sheet.view_cursor();

    for (i, col) in sheet.columns.iter().enumerate() {
        let col_area = column_layout[i];
        let heading_cell_area = Rect::new(col_area.x, 0, col_area.width, 1);
        buf.set_string(col_area.x, col_area.y, &col.table_column, app.color_scheme.sheet_heading_inactive);

        let style = if i == view_cursor.col() { app.color_scheme.sheet_heading_active } else { app.color_scheme.sheet_heading_inactive };
        buf.set_style(heading_cell_area, style)
    }

    let mut y = 1;
    let mut row_idx = 0;

    for row in &sheet.rows {
        for (i, col_area) in column_layout.iter().enumerate() {

            if y > area.height {
                return;
            }

            let is_selected_col = i == view_cursor.col();
            let is_selected_row = row_idx == view_cursor.row();
            let is_selected_cell = is_selected_col && is_selected_row;

            let style = match () {
                _ if is_selected_cell && app.mode.is_editing() => app.color_scheme.cell_editing,
                _ if is_selected_cell => app.color_scheme.cell_selected,
                _ if (is_selected_row || is_selected_col) && app.mode.is_normal() => app.color_scheme.cell_aligned,
                _ => app.color_scheme.cell,
            };

            let mut cell_display_text = row.cells[i].display.as_str();

            if let Mode::EditingCell(ref text_input) = app.mode {
                if is_selected_cell {
                    cell_display_text = &text_input.input;
                }
            }

            let para = Paragraph::new(cell_display_text).style(style);
            para.render(Rect::new(col_area.x, y, col_area.width, 1), buf);

            if let Mode::EditingCell(ref text_input) = app.mode {
                if is_selected_cell {
                    buf.set_style(Rect::new(col_area.x + (text_input.char_index as u16), y, 1, 1), app.color_scheme.cell_editing_cursor);
                }
            }
        }
        y += 1;
        row_idx += 1;
    }
}

pub fn statusbar_view(app: &App, area: Rect, buf: &mut Buffer) {
    buf.set_style(area, app.color_scheme.status_bar);

    let Some(sheet) = app.active_sheet() else { return };

    let status_string = format!("{}-{} / {}     Cursor {}, {}", sheet.start_offset, sheet.end_offset(), sheet.total_count, sheet.selected_cell.x, sheet.selected_cell.y);
    buf.set_stringn(area.x, area.y, status_string, area.width as usize, Style::default());
}

pub fn sheet_view(app: &App, area: Rect, buf: &mut Buffer) {
    let layout = Layout::new(Direction::Vertical, vec![
        Constraint::Ratio(1, 1),
        Constraint::Min(1),
        Constraint::Min(1)
    ]).split(area);

    table_view(app, layout[0], buf);

    statusbar_view(app, layout[1], buf);

    let tab = TabBar {
        tabs: app.schema.entities.iter().map(|e| e.table.to_string()).collect(),
        color_scheme: &app.color_scheme,
        selected_index: app.current_sheet,
    };
    tab.render(layout[2], buf);
}
